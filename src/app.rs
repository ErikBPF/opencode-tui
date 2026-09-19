use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event as TerminalEvent, KeyCode, KeyModifiers};
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

/// Tracer-bullet M1: connect, load the session list, hold the event stream and
/// render the home screen. Mirrors upstream `app.tsx`'s provider wiring in a
/// deliberately reduced form.
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
    let mut route = Route::default();
    let selected = 0usize;
    let theme = Theme::default();
    let mut prompt = Prompt::default();
    let keybinds = crate::config::load_keybinds();

    let mut terminal = ratatui::init();
    let view = View {
        client: &client,
        theme: &theme,
        location: &location,
        keybinds: &keybinds,
    };
    let outcome = draw_loop(
        &mut terminal,
        &mut receiver,
        &mut store,
        &mut route,
        selected,
        &view,
        &mut prompt,
    )
    .await;
    ratatui::restore();
    outcome
}

/// Read-only view state shared by the draw loop: the client plus the footer
/// values. Bundled so the loop stays under the argument-count lint.
struct View<'a> {
    client: &'a OpencodeClient,
    theme: &'a Theme,
    location: &'a str,
    keybinds: &'a Keybinds,
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
    route: &mut Route,
    selected: usize,
    view: &View<'_>,
    prompt: &mut Prompt,
) -> Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, store, route, selected, view, prompt))?;

        while let Ok(message) = receiver.try_recv() {
            match message {
                Ok(envelope) => store.apply(&envelope),
                Err(_) => store.degraded(),
            }
        }

        if event::poll(Duration::from_millis(100))? {
            if let TerminalEvent::Key(key) = event::read()? {
                match view.keybinds.command_for(key) {
                    Some(Command::Quit) => return Ok(()),
                    Some(Command::Enter) => match route {
                        Route::Home => open_session(store, route, selected, view).await,
                        Route::Session(_) => submit_prompt(route, prompt, view).await,
                    },
                    Some(Command::Back) => *route = Route::Home,
                    None => {
                        // `q` quits only on the home screen; in a session it is
                        // prompt text.
                        if matches!(route, Route::Home)
                            && key.code == KeyCode::Char('q')
                            && key.modifiers.is_empty()
                        {
                            return Ok(());
                        }
                        edit_prompt(key, route, prompt);
                    }
                }
            }
        }
    }
}

/// Load the selected session's transcript and open it.
async fn open_session(store: &mut Store, route: &mut Route, selected: usize, view: &View<'_>) {
    let Some(session) = store.sessions.get(selected).cloned() else {
        return;
    };
    match view.client.list_messages(&session.id).await {
        Ok(messages) => {
            store.open_session(session.id.clone(), messages);
            *route = Route::Session(session.id);
        }
        Err(err) => tracing::warn!(%err, "failed to load transcript"),
    }
}

/// Submit the buffered prompt to the open session. The reply arrives on the
/// event stream, so only a successful send clears the buffer.
async fn submit_prompt(route: &Route, prompt: &mut Prompt, view: &View<'_>) {
    let Route::Session(id) = route else {
        return;
    };
    if prompt.is_empty() {
        return;
    }
    match view.client.send_prompt(id, prompt.text()).await {
        Ok(()) => {
            prompt.take();
        }
        Err(err) => tracing::warn!(%err, "failed to send prompt"),
    }
}

/// Apply a plain-text key to the prompt buffer while a session is open.
fn edit_prompt(key: crossterm::event::KeyEvent, route: &Route, prompt: &mut Prompt) {
    if !matches!(route, Route::Session(_)) {
        return;
    }
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char(character) if !control => prompt.push(character),
        KeyCode::Backspace => prompt.pop(),
        _ => {}
    }
}

fn draw(
    frame: &mut Frame,
    store: &Store,
    route: &Route,
    selected: usize,
    view: &View<'_>,
    prompt: &Prompt,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(frame.area());

    match route {
        Route::Home => crate::routes::home::render(frame, chunks[0], store, selected),
        Route::Session(id) => {
            let session = store.sessions.iter().find(|session| &session.id == id);
            crate::routes::session::render(
                frame,
                chunks[0],
                session,
                &store.messages,
                prompt.text(),
                store.pending_permission.as_ref(),
            );
        }
    }

    let status = format!(
        " {} | {} | {:?} ",
        view.theme.name, view.location, store.status
    );
    frame.render_widget(
        Paragraph::new(status).block(Block::default().borders(Borders::TOP)),
        chunks[1],
    );
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;
    use crate::context::event::Event;

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
}
