use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event as TerminalEvent, KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::context::event::Envelope;
use crate::context::prompt::Prompt;
use crate::context::route::Route;
use crate::context::sdk::OpencodeClient;
use crate::context::sync::Store;
use crate::context::theme::Theme;
use crate::keymap::{Command, Keybinds};
use crate::runtime::Args;

/// Tracer-bullet M1: connect, list sessions, hold the event stream and render
/// the screens. Mirrors upstream `app.tsx`'s provider wiring in a deliberately
/// reduced form.
pub async fn run(args: Args) -> Result<()> {
    let client = OpencodeClient::new(args.url.clone(), args.directory.clone())?;

    // The initial REST load happens before raw mode is enabled, so an
    // unreachable server is a clean error that never touches the terminal.
    let sessions = client
        .list_sessions()
        .await
        .map_err(|err| anyhow::anyhow!("cannot reach server at {}: {err}", client.display_url()))?;

    let location = args
        .directory
        .as_deref()
        .map(crate::util::abbreviate_home)
        .unwrap_or_else(|| client.display_url());

    // Headless readiness probe: report the connection and exit before any
    // terminal setup, so scripts and the behavior contract can assert it.
    if args.check {
        println!("opencode-tui: connected to {}", client.display_url());
        return Ok(());
    }

    // Bounded so a burst of events from a hostile or buggy server applies
    // backpressure instead of growing the queue without limit.
    let (sender, mut receiver) = mpsc::channel(1024);
    spawn_event_stream(client.clone(), sender);

    let mut store = Store::default();
    store.loaded(sessions);
    let keybinds = crate::config::load_keybinds();
    let mut app = App::default();

    let mut terminal = ratatui::init();
    let view = View {
        client: &client,
        theme: &Theme::default(),
        location: &location,
        keybinds: &keybinds,
    };
    let outcome = draw_loop(&mut terminal, &mut receiver, &mut store, &view, &mut app).await;
    ratatui::restore();
    outcome
}

/// Read-only view state shared by the draw loop: the client, footer values and
/// the resolved keybindings. Bundled so the loop stays under the argument-count
/// lint.
struct View<'a> {
    client: &'a OpencodeClient,
    theme: &'a Theme,
    location: &'a str,
    keybinds: &'a Keybinds,
}

/// Mutable UI state owned by the draw loop: the route, the home-screen
/// selection, the transcript scroll offset, the prompt buffer, whether the
/// leader key is pending, and whether the command palette is open.
#[derive(Default)]
struct App {
    route: Route,
    selected: usize,
    scroll: u16,
    prompt: Prompt,
    leader_pending: bool,
    palette: bool,
    /// Last failed action, surfaced in the footer instead of only logged.
    last_error: Option<String>,
}

/// Subscribe to `/global/event` and forward decoded envelopes, reconnecting
/// with bounded exponential backoff until the channel closes. Mirrors the
/// upstream retry loop (1s..30s): the attempt count is never reset on connect,
/// so a server that accepts and immediately drops still backs off.
fn spawn_event_stream(client: OpencodeClient, sender: mpsc::Sender<Result<Envelope>>) {
    tokio::spawn(async move {
        let mut attempt: u32 = 0;
        loop {
            match client.event_stream().await {
                Ok(mut stream) => {
                    while let Some(item) = stream.next().await {
                        if sender.send(item).await.is_err() {
                            return;
                        }
                    }
                    let _ = sender
                        .send(Err(anyhow::anyhow!("event stream ended")))
                        .await;
                }
                Err(err) => {
                    if sender.send(Err(err)).await.is_err() {
                        return;
                    }
                }
            }

            attempt += 1;
            tokio::time::sleep(crate::context::event::backoff(attempt)).await;
        }
    });
}

async fn draw_loop(
    terminal: &mut ratatui::DefaultTerminal,
    receiver: &mut mpsc::Receiver<Result<Envelope>>,
    store: &mut Store,
    view: &View<'_>,
    app: &mut App,
) -> Result<()> {
    // Redraw only when something changed: a key, a stream event, or a resize.
    // Redrawing every tick re-laid-out the whole transcript and burned CPU for
    // no visible change, which made a large session feel slow.
    let mut dirty = true;
    loop {
        if dirty {
            // The transcript row count comes back through a cell so the scroll
            // clamp sees it without threading a return value through the frame.
            let total = std::cell::Cell::new(0usize);
            let height = std::cell::Cell::new(0usize);
            terminal.draw(|frame| {
                let (rows, viewport) = draw(frame, store, view, app);
                total.set(rows);
                height.set(viewport);
            })?;
            // Clamp to the last row that still fills the viewport, so
            // `scroll = u16::MAX` (scroll to bottom) lands on the real end.
            let last_top = last_scroll(total.get(), height.get());
            app.scroll = app.scroll.min(last_top);
            dirty = false;
        }

        while let Ok(message) = receiver.try_recv() {
            match message {
                Ok(envelope) => store.apply(&envelope),
                Err(_) => store.degraded(),
            }
            dirty = true;
        }

        // A short poll keeps keys and stream events responsive; when idle we
        // block until the next event instead of spinning.
        let poll = if dirty {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(100)
        };
        if event::poll(poll)? {
            match event::read()? {
                TerminalEvent::Key(key) => {
                    if dispatch(key, store, view, app).await {
                        return Ok(());
                    }
                    dirty = true;
                }
                TerminalEvent::Resize(_, _) => dirty = true,
                _ => {}
            }
        }
    }
}

/// Handle one key event. Returns `true` when the app should exit.
async fn dispatch(key: KeyEvent, store: &mut Store, view: &View<'_>, app: &mut App) -> bool {
    // A pending leader prefix resolves the next key through the leader bindings
    // and always clears the prefix, matching upstream's leader state machine.
    if app.leader_pending {
        app.leader_pending = false;
        if let Some(command) = view.keybinds.leader_command_for(key) {
            return run_command(command, store, view, app).await;
        }
        return false;
    }
    if view.keybinds.is_leader(key) {
        app.leader_pending = true;
        return false;
    }

    if app.palette {
        // The palette closes on any key; `Esc` also cancels.
        app.palette = false;
        return false;
    }

    // Any key dismisses a surfaced error, so it never lingers over the footer.
    app.last_error = None;

    if let Some(command) = view.keybinds.command_for(key) {
        return run_command(command, store, view, app).await;
    }

    // Contextual fallbacks not bound by default.
    if matches!(app.route, Route::Home)
        && key.code == KeyCode::Char('q')
        && key.modifiers.is_empty()
    {
        return true;
    }
    edit_prompt(key, app);
    false
}

/// Apply a command. Returns `true` when it should exit the app.
async fn run_command(command: Command, store: &mut Store, view: &View<'_>, app: &mut App) -> bool {
    match command {
        Command::AppExit => return true,
        Command::CommandList => app.palette = true,
        Command::InputSubmit => match app.route {
            Route::Home => open_selected_session(store, view, app).await,
            Route::Session(_) => submit_prompt(view, app).await,
        },
        Command::SessionBack => {
            app.route = Route::Home;
            app.scroll = 0;
        }
        Command::SessionNew => new_session(store, view, app).await,
        // Escape is the interrupt key upstream; the pinned v1 API has no
        // interrupt endpoint, so in a session it returns to the list.
        Command::SessionInterrupt => {
            app.route = Route::Home;
            app.scroll = 0;
        }
        Command::SessionNext => {
            if !store.sessions.is_empty() {
                app.selected = (app.selected + 1).min(store.sessions.len() - 1);
            }
        }
        Command::SessionPrevious => app.selected = app.selected.saturating_sub(1),
        Command::MessagesPageUp => app.scroll = app.scroll.saturating_sub(10),
        Command::MessagesPageDown => app.scroll = app.scroll.saturating_add(10),
        Command::MessagesFirst => app.scroll = 0,
        Command::MessagesLast => app.scroll = u16::MAX,
    }
    false
}

/// Load the selected session's transcript and open it.
async fn open_selected_session(store: &mut Store, view: &View<'_>, app: &mut App) {
    let Some(session) = store.sessions.get(app.selected).cloned() else {
        return;
    };
    open_session(store, view, app, session.id).await;
}

/// Create a session on the server and open it.
async fn new_session(store: &mut Store, view: &View<'_>, app: &mut App) {
    match view.client.create_session().await {
        Ok(session) => open_session(store, view, app, session.id).await,
        Err(err) => {
            tracing::warn!(%err, "failed to create session");
            app.last_error = Some(format!("create session failed: {err}"));
        }
    }
}

/// Load a session's transcript, open it, and select it in the home list.
async fn open_session(store: &mut Store, view: &View<'_>, app: &mut App, id: String) {
    match view.client.list_messages(&id).await {
        Ok(messages) => {
            store.open_session(id.clone(), messages);
            app.route = Route::Session(id.clone());
            app.scroll = 0;
            app.last_error = None;
            if let Some(index) = store.sessions.iter().position(|session| session.id == id) {
                app.selected = index;
            }
        }
        Err(err) => {
            tracing::warn!(%err, "failed to load transcript");
            app.last_error = Some(format!("load transcript failed: {err}"));
        }
    }
}

/// Submit the buffered prompt to the open session. The reply arrives on the
/// event stream, so only a successful send clears the buffer.
async fn submit_prompt(view: &View<'_>, app: &mut App) {
    let Route::Session(id) = &app.route else {
        return;
    };
    if app.prompt.is_empty() {
        return;
    }
    match view.client.send_prompt(id, app.prompt.text()).await {
        Ok(()) => {
            app.prompt.take();
            app.scroll = u16::MAX;
            app.last_error = None;
        }
        Err(err) => {
            tracing::warn!(%err, "failed to send prompt");
            app.last_error = Some(format!("send prompt failed: {err}"));
        }
    }
}

/// Apply a plain-text key to the prompt buffer while a session is open.
fn edit_prompt(key: KeyEvent, app: &mut App) {
    if !matches!(app.route, Route::Session(_)) {
        return;
    }
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char(character) if !control => app.prompt.push(character),
        KeyCode::Backspace => app.prompt.pop(),
        _ => {}
    }
}

/// Draw one frame and return the transcript row count plus the viewport height,
/// so the scroll offset can be clamped to the real end.
/// The largest scroll offset that still shows content: the last row that fills
/// the viewport. Keeps `scroll = u16::MAX` (scroll to bottom) on the real end.
fn last_scroll(total: usize, height: usize) -> u16 {
    total.saturating_sub(height.max(1)).min(u16::MAX as usize) as u16
}

fn draw(frame: &mut Frame, store: &Store, view: &View<'_>, app: &App) -> (usize, usize) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(frame.area());

    let total = match &app.route {
        Route::Home => {
            crate::routes::home::render(frame, chunks[0], store, app.selected);
            0
        }
        Route::Session(id) => {
            let session = store.sessions.iter().find(|session| &session.id == id);
            crate::routes::session::render(
                frame,
                chunks[0],
                session,
                &store.messages,
                app.prompt.text(),
                store.pending_permission.as_ref(),
                app.scroll,
            )
        }
    };

    if app.palette {
        crate::component::command_palette::render(frame, chunks[0], view.keybinds);
    }

    let leader = if app.leader_pending { " LEADER " } else { "" };
    let status = match &app.last_error {
        Some(error) => format!(" {error} | esc to dismiss{leader} "),
        None => format!(
            " {} | {} | {:?}{leader} ",
            view.theme.name, view.location, store.status
        ),
    };
    frame.render_widget(
        Paragraph::new(status).block(Block::default().borders(Borders::TOP)),
        chunks[1],
    );
    (total, chunks[0].height as usize)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;
    use crate::context::event::Event;

    #[test]
    fn last_scroll_keeps_the_viewport_filled() {
        assert_eq!(last_scroll(0, 10), 0);
        assert_eq!(last_scroll(10, 10), 0);
        assert_eq!(last_scroll(11, 10), 1);
        assert_eq!(last_scroll(25, 10), 15);
        assert_eq!(last_scroll(5, 0), 4);
    }

    #[test]
    fn typing_edits_the_prompt_only_inside_a_session() {
        let mut app = App::default();
        // On the home screen printable keys are not prompt text.
        edit_prompt(
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            &mut app,
        );
        assert!(app.prompt.is_empty());

        app.route = Route::Session("ses".to_string());
        for character in ['h', 'i'] {
            edit_prompt(
                KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
                &mut app,
            );
        }
        assert_eq!(app.prompt.text(), "hi");
        edit_prompt(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            &mut app,
        );
        assert_eq!(app.prompt.text(), "h");
        // A control chord is not text.
        edit_prompt(
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
            &mut app,
        );
        assert_eq!(app.prompt.text(), "h");
    }

    /// One SSE response carrying a single `server.connected` frame, then close.
    fn serve_connected(listener: &TcpListener) {
        let (mut socket, _) = listener.accept().expect("accept");
        let mut request = [0u8; 1024];
        let _ = socket.read(&mut request);
        let body = "data: {\"payload\":{\"type\":\"server.connected\"},\"directory\":\"/d\"}\n\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n{body}"
        );
        socket.write_all(response.as_bytes()).expect("write");
        socket.flush().expect("flush");
    }

    #[tokio::test]
    async fn reconnects_after_the_stream_drops() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            serve_connected(&listener);
            serve_connected(&listener);
        });

        let client = OpencodeClient::new(format!("http://{addr}"), None).expect("client");
        let (sender, mut receiver) = mpsc::channel(16);
        spawn_event_stream(client, sender);

        let mut connected = 0;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while connected < 2 && tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(10), receiver.recv()).await {
                Ok(Some(Ok(envelope))) => {
                    if matches!(envelope.payload, Event::ServerConnected { .. }) {
                        connected += 1;
                    }
                }
                Ok(Some(Err(_))) => {}
                Ok(None) | Err(_) => break,
            }
        }

        assert_eq!(connected, 2, "expected a second connection after the drop");
        server.join().expect("stub server");
    }

    #[test]
    fn leader_prefix_resolves_and_clears() {
        let keybinds = crate::config::load_keybinds();
        assert!(keybinds.is_leader(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)));
        assert_eq!(
            keybinds.leader_command_for(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(Command::AppExit)
        );
    }

    #[tokio::test]
    async fn selection_moves_within_bounds() {
        let client = OpencodeClient::new("http://127.0.0.1:1".to_string(), None).expect("client");
        let theme = Theme::default();
        let keybinds = crate::config::load_keybinds();
        let view = View {
            client: &client,
            theme: &theme,
            location: "",
            keybinds: &keybinds,
        };
        let mut store = Store::default();
        store.loaded(vec![
            crate::context::sdk::Session {
                id: "a".into(),
                title: None,
            },
            crate::context::sdk::Session {
                id: "b".into(),
                title: None,
            },
        ]);
        let mut app = App::default();

        run_command(Command::SessionNext, &mut store, &view, &mut app).await;
        assert_eq!(app.selected, 1);
        run_command(Command::SessionNext, &mut store, &view, &mut app).await;
        assert_eq!(app.selected, 1, "selection must not run past the list");
        run_command(Command::SessionPrevious, &mut store, &view, &mut app).await;
        assert_eq!(app.selected, 0);
    }
}
