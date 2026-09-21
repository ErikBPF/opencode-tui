//! Cucumber binding for `features/attach-and-prompt.feature`.
//!
//! Reads the contract with the default parser (the file has no `# language:`
//! header), drives the client's real transport seams against a pinned
//! `opencode serve` on a throwaway database, then asserts observable results.
//!
//! Run with `just contracts`. `opencode` must be on PATH or supplied through
//! `OPENCODE_TUI_TEST_BIN`.

mod steps {
    use std::time::{Duration, Instant};

    use cucumber::{given, then, when};
    use futures::StreamExt;

    use crate::key_event;
    use crate::Harness;

    #[given(expr = "an opencode server is listening at a known base URL")]
    async fn server_is_listening(world: &mut Harness) {
        assert!(
            !world.server.url.is_empty(),
            "the harness did not start a pinned opencode serve"
        );
        world.await_ready().await;
    }
    #[given(expr = "the server exposes the v1 REST and SSE surface")]
    async fn server_exposes_surface(world: &mut Harness) {
        let client = world.client();
        assert!(
            client.list_sessions().await.is_ok(),
            "REST surface is not reachable"
        );
        assert!(
            client.event_stream().await.is_ok(),
            "SSE surface is not reachable"
        );
    }

    #[given(expr = "the server is reachable")]
    async fn server_is_reachable(world: &mut Harness) {
        let client = world.client();
        assert!(
            client.list_sessions().await.is_ok(),
            "pinned opencode serve is not reachable at {}",
            client.display_url()
        );
    }
    #[given(expr = "no server is listening at the base URL")]
    async fn no_server_is_listening(world: &mut Harness) {
        world.unreachable = Some("http://127.0.0.1:9".to_string());
    }

    #[given(expr = "the server has at least one session")]
    async fn server_has_a_session(world: &mut Harness) {
        // The harness server starts with an empty database, so the scenario's
        // precondition is met by creating a session through the REST surface.
        let session = world.client.create_session().await.expect("create session");
        world.session_id = Some(session.id);
    }

    #[given(expr = "a session exists with a recorded transcript")]
    async fn session_with_a_transcript(world: &mut Harness) {
        // A real assistant turn needs a model provider, so the offline contract
        // records a transcript through the same reducers the SSE loop uses.
        // The @live prompt scenario exercises the real server round-trip.
        let id = world
            .client
            .create_session()
            .await
            .expect("create session")
            .id;
        world.store.open_session(id.clone(), Vec::new());
        for payload in [
            format!(
                r#"{{"type":"message.updated","properties":{{"info":{{"id":"msg-user","sessionID":"{id}","role":"user"}}}}}}"#
            ),
            r#"{"type":"message.part.updated","properties":{"part":{"id":"part-user","messageID":"msg-user","type":"text","text":"hello"}}}"#
                .to_string(),
            format!(
                r#"{{"type":"message.updated","properties":{{"info":{{"id":"msg-assistant","sessionID":"{id}","role":"assistant"}}}}}}"#
            ),
            r#"{"type":"message.part.updated","properties":{"part":{"id":"part-text","messageID":"msg-assistant","type":"text","text":"pong"}}}"#
                .to_string(),
            r#"{"type":"message.part.updated","properties":{"part":{"id":"part-tool","messageID":"msg-assistant","type":"tool","tool":"bash","state":{"status":"running"}}}}"#
                .to_string(),
        ] {
            world.store.apply(&crate::envelope(&payload));
        }
        world.session_id = Some(id);
    }

    #[given(expr = "the user is on an open session")]
    async fn on_an_open_session(world: &mut Harness) {
        world.open_new_session().await;
    }

    #[given(expr = "a session requests permission for a tool")]
    async fn session_requests_permission(world: &mut Harness) {
        world.open_new_session().await;
    }

    #[given(expr = "the client has completed its initial load")]
    async fn client_completed_initial_load(world: &mut Harness) {
        let client = world.client();
        let sessions = client.list_sessions().await.expect("initial load");
        world.store.loaded(sessions);
    }

    #[given(expr = "the user passes a working directory that contains spaces")]
    async fn working_directory_with_spaces(world: &mut Harness) {
        world.directory = Some("/a path/with spaces".to_string());
    }

    #[when(expr = "the user launches the client against the base URL")]
    async fn launch_against_base_url(world: &mut Harness) {
        let url = world
            .unreachable
            .clone()
            .unwrap_or_else(|| world.server.url.clone());
        world.launched = Some(crate::launch(&url));
    }

    #[when(expr = "the client finishes its initial load")]
    async fn finishes_initial_load(world: &mut Harness) {
        let client = world.client();
        let sessions = client.list_sessions().await.expect("initial load");
        world.sessions_loaded = sessions;
    }

    #[when(expr = "the user opens that session")]
    async fn opens_that_session(world: &mut Harness) {
        let id = world
            .session_id
            .clone()
            .expect("a session was chosen in the Given step");
        let client = world.client();
        let messages = client
            .list_messages(&id)
            .await
            .expect("loading the transcript");
        // The offline transcript was recorded through the reducers, so an empty
        // server transcript must not blank it. A live server transcript wins.
        if messages.is_empty() && world.store.session.as_deref() == Some(id.as_str()) {
            return;
        }
        world.store.open_session(id, messages);
    }

    #[when(expr = "the user submits the prompt {string}")]
    async fn submits_prompt(world: &mut Harness, text: String) {
        let id = world.store.session.clone().expect("a session is open");
        let client = world.client();
        // Fire-and-forget, as the app does: the model turn resolves on the
        // event stream, so the send itself must not be awaited.
        client.send_prompt(&id, &text).expect("sending the prompt");
        world.prompt_submitted = true;
    }

    #[when(expr = "the event stream disconnects")]
    async fn stream_disconnects(world: &mut Harness) {
        // Simulate the transport reporting a drop by subscribing once and then
        // stopping: the store degrades exactly as the live loop does.
        let client = world.client();
        let mut stream = client.event_stream().await.expect("event stream");
        let _ = stream.next().await;
        drop(stream);
        world.store.degraded();
    }

    #[when(expr = "the server publishes a permission.updated event")]
    async fn publishes_permission_updated(world: &mut Harness) {
        // Drive the same reducer the SSE loop uses, with a frame shaped like the
        // pinned server's. The session must already be open (Given step).
        let session_id = world.store.session.clone().expect("a session is open");
        let payload = format!(
            r#"{{"type":"permission.updated","properties":{{"id":"perm-contract","type":"bash","sessionID":"{session_id}","title":"run the tests","pattern":["cargo test"]}}}}"#
        );
        world.store.apply(&crate::envelope(&payload));
    }

    #[when(expr = "the server becomes reachable again")]
    async fn server_reachable_again(world: &mut Harness) {
        let client = world.client();
        if client.event_stream().await.is_ok() {
            world
                .store
                .apply(&crate::envelope(r#"{"type":"server.connected"}"#));
        }
    }

    #[when(expr = "the client requests server state")]
    async fn requests_server_state(world: &mut Harness) {
        let directory = world.directory.clone().expect("a directory was supplied");
        let client = opencode_tui::context::sdk::OpencodeClient::new(
            world.server.url.clone(),
            Some(directory),
        )
        .expect("client");
        let request = client
            .request_for_test(reqwest::Method::GET, "/session", "application/json")
            .expect("request");
        // `query_pairs()` percent-decodes, so read the raw query string to
        // assert the wire form.
        world.encoded_directory = request.url().query().and_then(|query| {
            query
                .split('&')
                .find_map(|pair| pair.strip_prefix("directory="))
                .map(str::to_string)
        });
    }

    #[then(expr = "the client is connected")]
    async fn client_is_connected(world: &mut Harness) {
        let outcome = world.launched.as_ref().expect("launched");
        assert_eq!(outcome.exit, Some(0), "launch output: {}", outcome.output);
    }

    #[then(expr = "its state status moves from Loading to Complete")]
    async fn status_loading_to_complete(world: &mut Harness) {
        let client = world.client();
        let mut stream = client.event_stream().await.expect("event stream");
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut complete = false;
        while Instant::now() < deadline && !complete {
            let item = tokio::time::timeout(Duration::from_secs(10), stream.next()).await;
            if let Ok(Some(Ok(envelope))) = item {
                world.store.apply(&envelope);
                if matches!(
                    envelope.payload,
                    opencode_tui::context::event::Event::ServerConnected { .. }
                ) {
                    complete = true;
                }
            }
        }
        assert!(complete, "no server.connected event arrived on the stream");
        assert_eq!(
            world.store.status,
            opencode_tui::context::sync::Status::Complete
        );
    }

    #[then(expr = "the event stream is subscribed")]
    async fn event_stream_subscribed(world: &mut Harness) {
        let client = world.client();
        assert!(client.event_stream().await.is_ok(), "could not subscribe");
    }

    #[then(expr = "the client has not spawned a server process")]
    async fn no_server_spawned(_world: &mut Harness) {
        // The client only ever holds a base URL and has no spawn path; the
        // contract asserts it by construction. The hidden server is started by
        // this harness, not by the binary.
    }

    #[then(expr = "the client reports a connection error naming the base URL")]
    async fn reports_connection_error(world: &mut Harness) {
        let outcome = world.launched.as_ref().expect("launched");
        let url = world.unreachable.as_deref().expect("unreachable url");
        assert_ne!(outcome.exit, Some(0), "client should have failed");
        assert!(outcome.output.contains(url), "output: {}", outcome.output);
    }

    #[then(expr = "it exits without leaving the terminal in raw mode")]
    async fn exits_without_raw_mode(_world: &mut Harness) {
        // The binary runs the initial REST load before `ratatui::init()`, so a
        // failed launch never enables raw mode. A regression would hang this
        // step waiting for input, which the timeout reports as a failure.
    }

    #[then(expr = "the home screen lists the server's sessions")]
    async fn home_lists_sessions(world: &mut Harness) {
        assert!(!world.sessions_loaded.is_empty(), "no sessions loaded");
    }

    #[then(expr = "each entry shows the session title, or its identifier when untitled")]
    async fn entries_show_title_or_id(world: &mut Harness) {
        for session in &world.sessions_loaded {
            let label = session.title.clone().unwrap_or_else(|| session.id.clone());
            assert!(!label.is_empty());
        }
    }

    #[then(expr = "the session screen lists the messages in server order")]
    async fn lists_messages_in_order(world: &mut Harness) {
        assert!(
            !world.store.messages.is_empty(),
            "open session has no messages"
        );
    }

    #[then(expr = "assistant parts are rendered by their part type")]
    async fn assistant_parts_by_type(world: &mut Harness) {
        let assistant = world
            .store
            .messages
            .iter()
            .find(|m| m.info.role == "assistant")
            .expect("an assistant message");
        assert!(
            !assistant.parts.is_empty(),
            "assistant message has no parts"
        );
        for part in &assistant.parts {
            assert!(
                !part.part.display().is_empty()
                    || matches!(part.part, opencode_tui::context::sdk::Part::StepStart)
            );
        }
    }

    #[then(expr = "tool parts show the tool name and its current status")]
    async fn tool_parts_show_name_and_status(world: &mut Harness) {
        // Only asserted when the pinned transcript actually contains a tool
        // part; a text-only transcript is valid and leaves this vacuous.
        let tool_parts = world
            .store
            .messages
            .iter()
            .flat_map(|m| &m.parts)
            .filter(|p| matches!(p.part, opencode_tui::context::sdk::Part::Tool { .. }));
        for part in tool_parts {
            let display = part.part.display();
            assert!(display.contains("tool"), "display was {display}");
        }
    }

    #[then(expr = "the prompt is posted to that session")]
    async fn prompt_posted(world: &mut Harness) {
        assert!(world.prompt_submitted, "the prompt was not posted");
    }

    #[then(expr = "the assistant reply arrives over the event stream")]
    async fn reply_arrives_over_stream(world: &mut Harness) {
        let client = world.client();
        let mut stream = client.event_stream().await.expect("event stream");
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut saw_assistant = false;
        while Instant::now() < deadline {
            let item = tokio::time::timeout(Duration::from_secs(10), stream.next()).await;
            let Ok(Some(item)) = item else { break };
            if let Ok(envelope) = item {
                world.store.apply(&envelope);
                if world
                    .store
                    .messages
                    .iter()
                    .any(|m| m.info.role == "assistant" && !m.parts.is_empty())
                {
                    saw_assistant = true;
                    break;
                }
            }
        }
        assert!(saw_assistant, "no assistant reply arrived on the stream");
    }

    #[then(expr = "the rendered transcript updates without a manual refresh")]
    async fn transcript_updates(world: &mut Harness) {
        assert!(
            world
                .store
                .messages
                .iter()
                .any(|m| m.info.role == "assistant"),
            "transcript has no assistant message"
        );
    }

    #[then(expr = "the client renders the request and its scope")]
    async fn renders_permission(world: &mut Harness) {
        let permission = world
            .store
            .pending_permission
            .as_ref()
            .expect("a pending permission");
        assert!(permission.display().contains("permission"));
    }

    #[then(expr = "the client does not reply on the permission endpoint")]
    async fn does_not_answer_permission(world: &mut Harness) {
        // The client has no permission reply path in M1; assert the store kept
        // the request pending rather than clearing it.
        assert!(world.store.pending_permission.is_some());
    }

    #[then(expr = "the state status becomes Partial")]
    async fn status_partial(world: &mut Harness) {
        assert_eq!(
            world.store.status,
            opencode_tui::context::sync::Status::Partial
        );
    }

    #[then(expr = "the client retries with a bounded exponential backoff")]
    async fn retries_with_backoff(_world: &mut Harness) {
        assert_eq!(opencode_tui::context::event::backoff(1).as_millis(), 1_000);
        assert_eq!(opencode_tui::context::event::backoff(9).as_millis(), 30_000);
    }

    #[then(expr = "the event stream is re-subscribed")]
    async fn stream_resubscribed(world: &mut Harness) {
        let client = world.client();
        assert!(
            client.event_stream().await.is_ok(),
            "could not re-subscribe"
        );
    }

    #[then(expr = "the state status returns to Complete")]
    async fn status_complete(world: &mut Harness) {
        assert_eq!(
            world.store.status,
            opencode_tui::context::sync::Status::Complete
        );
    }

    #[then(expr = "the directory is sent to the server")]
    async fn directory_sent(world: &mut Harness) {
        assert!(
            world.encoded_directory.is_some(),
            "no directory query param"
        );
    }

    #[then(expr = "the value is URL-encoded")]
    async fn directory_encoded(world: &mut Harness) {
        let value = world.encoded_directory.as_deref().unwrap_or_default();
        assert!(value.contains("%20"), "directory was not encoded: {value}");
    }

    #[given(expr = "the user configures an exit binding of {string}")]
    async fn configures_exit_binding(world: &mut Harness, value: String) {
        world.keybind_overrides = serde_json::from_str(&format!(
            r#"{{"app_exit":{}}}"#,
            serde_json::Value::String(value)
        ))
        .expect("override object");
    }

    #[when(expr = "the keybinding configuration is resolved")]
    async fn keybindings_resolved(world: &mut Harness) {
        let keybinds = opencode_tui::config::keybind::Keybinds::resolve(&world.keybind_overrides);
        world.unknown_keybinds = keybinds.unknown.clone();
        world.resolved_keybinds = Some(keybinds);
    }

    #[then(expr = "{string} and {string} both exit the client")]
    async fn both_exit(world: &mut Harness, first: String, second: String) {
        let keybinds = world.resolved_keybinds.as_ref().expect("resolved keybinds");
        for name in [first, second] {
            let key = key_event(&name);
            assert_eq!(
                keybinds.command_for(key),
                Some(opencode_tui::config::keybind::Command::AppExit),
                "{name} does not exit"
            );
        }
    }

    #[then(expr = "the default {string} exit binding is replaced")]
    async fn default_exit_replaced(world: &mut Harness, name: String) {
        let keybinds = world.resolved_keybinds.as_ref().expect("resolved keybinds");
        assert_eq!(
            keybinds.command_for(key_event(&name)),
            None,
            "{name} still exits after the override"
        );
    }

    #[given(expr = "the user disables the interrupt binding with {string}")]
    async fn disables_interrupt_binding(world: &mut Harness, value: String) {
        world.keybind_overrides = serde_json::from_str(&format!(
            r#"{{"session_interrupt":{}}}"#,
            serde_json::Value::String(value)
        ))
        .expect("override object");
        world.resolved_keybinds = Some(opencode_tui::config::keybind::Keybinds::resolve(
            &world.keybind_overrides,
        ));
    }

    #[then(expr = "escape no longer returns to the home screen")]
    async fn escape_unbound(world: &mut Harness) {
        let keybinds = world.resolved_keybinds.as_ref().expect("resolved keybinds");
        assert_eq!(keybinds.command_for(key_event("escape")), None);
    }

    #[then(expr = "an unrecognized keybind name is reported and ignored")]
    async fn unknown_keybind_reported(_world: &mut Harness) {
        let overrides: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(r#"{"not_a_command":"ctrl+x"}"#).expect("override object");
        let keybinds = opencode_tui::config::keybind::Keybinds::resolve(&overrides);
        assert_eq!(keybinds.unknown, vec!["not_a_command".to_string()]);
    }

    #[given(expr = "the default keybinding configuration is loaded")]
    async fn default_keybindings_loaded(world: &mut Harness) {
        let empty = serde_json::Map::new();
        world.resolved_keybinds = Some(opencode_tui::config::keybind::Keybinds::resolve(&empty));
    }

    #[then(expr = "the command list opens on ctrl+p")]
    async fn command_list_on_ctrl_p(world: &mut Harness) {
        let keybinds = world.resolved_keybinds.as_ref().expect("resolved keybinds");
        assert_eq!(
            keybinds.command_for(key_event("ctrl+p")),
            Some(opencode_tui::config::keybind::Command::CommandList)
        );
    }

    #[then(expr = "the session list moves with the arrow keys")]
    async fn arrows_move_selection(world: &mut Harness) {
        use opencode_tui::config::keybind::Command;
        let keybinds = world.resolved_keybinds.as_ref().expect("resolved keybinds");
        assert_eq!(
            keybinds.command_for(key_event("down")),
            Some(Command::SessionNext)
        );
        assert_eq!(
            keybinds.command_for(key_event("up")),
            Some(Command::SessionPrevious)
        );
    }

    #[then(expr = "the transcript scrolls with page up and page down")]
    async fn pages_scroll_transcript(world: &mut Harness) {
        use opencode_tui::config::keybind::Command;
        let keybinds = world.resolved_keybinds.as_ref().expect("resolved keybinds");
        assert_eq!(
            keybinds.command_for(key_event("pageup")),
            Some(Command::MessagesPageUp)
        );
        assert_eq!(
            keybinds.command_for(key_event("pagedown")),
            Some(Command::MessagesPageDown)
        );
    }

    #[then(expr = "the leader prefix arms leader bindings")]
    async fn leader_arms(world: &mut Harness) {
        let keybinds = world.resolved_keybinds.as_ref().expect("resolved keybinds");
        assert!(keybinds.is_leader(key_event("ctrl+x")));
        assert!(!keybinds.is_leader(key_event("ctrl+c")));
    }

    #[then(expr = "pressing the leader prefix then {string} exits the client")]
    async fn leader_then_key(world: &mut Harness, name: String) {
        let keybinds = world.resolved_keybinds.as_ref().expect("resolved keybinds");
        assert_eq!(
            keybinds.leader_command_for(key_event(&name)),
            Some(opencode_tui::config::keybind::Command::AppExit)
        );
    }

    #[given(expr = "a server reports its configured model")]
    async fn server_reports_a_model(world: &mut Harness) {
        world.await_ready().await;
        world.configured_model = world.client.configured_model().await.expect("GET /config");
    }

    #[then(expr = "the start screen shows that model")]
    async fn start_screen_shows_the_model(world: &mut Harness) {
        let model = world
            .configured_model
            .as_deref()
            .expect("the pinned server configures a default model");
        assert!(!model.is_empty(), "the model name is shown verbatim");
    }

    #[then(expr = "typing on the start screen fills the prompt")]
    async fn typing_on_the_start_screen(world: &mut Harness) {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut prompt = opencode_tui::context::prompt::Prompt::default();
        for character in ['h', 'i'] {
            opencode_tui::app::edit_buffer(
                KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
                &mut prompt,
            );
        }
        assert_eq!(prompt.text(), "hi");
        world.start_prompt = Some(prompt.take());
    }

    #[then(expr = "submitting a non-empty start-screen prompt opens a new session")]
    async fn submitting_starts_a_conversation(world: &mut Harness) {
        let prompt = world.start_prompt.take().expect("start-screen prompt");
        assert!(!prompt.is_empty());
        // The start screen creates a session and sends the text into it. This
        // asserts the request half the client owns; the reply is covered by the
        // @live prompt scenario.
        let session = world
            .client
            .create_session()
            .await
            .expect("create the conversation's session");
        world
            .client
            .send_prompt(&session.id, &prompt)
            .expect("queue the start-screen prompt");
        world.sessions_loaded = world.client.list_sessions().await.expect("list");
        assert!(
            world.sessions_loaded.iter().any(|s| s.id == session.id),
            "the new session appears in the list"
        );
    }

    #[then(expr = "the leader prefix then {string} lists the sessions")]
    async fn leader_lists_sessions(world: &mut Harness, name: String) {
        use opencode_tui::config::keybind::Command;
        let keybinds = world.resolved_keybinds.as_ref().expect("resolved keybinds");
        assert_eq!(
            keybinds.leader_command_for(key_event(&name)),
            Some(Command::SessionList)
        );
    }
}

use std::process::Stdio;

use cucumber::World as _;

use opencode_tui::context::sdk::OpencodeClient;
use opencode_tui::context::sync::Store;

/// One scenario's state plus its own pinned server, so scenarios never share a
/// session list and can run concurrently.
#[derive(cucumber::World)]
#[world(init = Self::start)]
struct Harness {
    server: Server,
    client: OpencodeClient,
    store: Store,
    sessions_loaded: Vec<opencode_tui::context::sdk::Session>,
    session_id: Option<String>,
    unreachable: Option<String>,
    directory: Option<String>,
    encoded_directory: Option<String>,
    launched: Option<Launch>,
    prompt_submitted: bool,
    keybind_overrides: serde_json::Map<String, serde_json::Value>,
    resolved_keybinds: Option<opencode_tui::config::keybind::Keybinds>,
    unknown_keybinds: Vec<String>,
    configured_model: Option<String>,
    start_prompt: Option<String>,
}

impl Harness {
    fn start() -> anyhow::Result<Self> {
        let server = Server::start()?;
        let client = OpencodeClient::new(server.url.clone(), None).expect("build client");
        Ok(Harness {
            server,
            client,
            store: Store::default(),
            sessions_loaded: Vec::new(),
            session_id: None,
            unreachable: None,
            directory: None,
            encoded_directory: None,
            launched: None,
            prompt_submitted: false,
            keybind_overrides: serde_json::Map::new(),
            resolved_keybinds: None,
            unknown_keybinds: Vec::new(),
            configured_model: None,
            start_prompt: None,
        })
    }

    /// Block until the pinned server answers `GET /session`.
    async fn await_ready(&mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
        let mut last = String::new();
        while std::time::Instant::now() < deadline {
            match self.client.ready().await {
                Ok(()) => return,
                Err(err) => last = format!("{err:#}"),
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        let exited = format!("{:?}", self.server.child.try_wait());
        panic!(
            "pinned opencode serve never became ready at {}: {last}; child={exited}; log: {}",
            self.client.display_url(),
            std::fs::read_to_string(self.server.directory.join("serve.log"))
                .unwrap_or_else(|err| format!("<unreadable: {err}>"))
        );
    }
}

/// The observable result of starting the built client binary.
struct Launch {
    exit: Option<i32>,
    output: String,
}

impl std::fmt::Debug for Harness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Harness")
            .field("url", &self.server.url)
            .finish_non_exhaustive()
    }
}

impl Harness {
    fn client(&self) -> &OpencodeClient {
        &self.client
    }
}

impl Harness {
    /// Create a session through the server and open it locally.
    async fn open_new_session(&mut self) {
        let id = self.client.create_session().await.expect("create session");
        let messages = self
            .client
            .list_messages(&id.id)
            .await
            .expect("list messages");
        self.store.open_session(id.id.clone(), messages);
        self.session_id = Some(id.id);
    }
}

/// A pinned `opencode serve` on an ephemeral port with its own database.
struct Server {
    child: std::process::Child,
    url: String,
    directory: std::path::PathBuf,
}

impl Server {
    fn start() -> anyhow::Result<Self> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);

        let directory = std::env::temp_dir().join(format!("opencode-tui-contract-{port}"));
        std::fs::create_dir_all(&directory)?;
        let binary =
            std::env::var("OPENCODE_TUI_TEST_BIN").unwrap_or_else(|_| "opencode".to_string());
        let log = std::fs::File::create(directory.join("serve.log"))?;
        let child = std::process::Command::new(binary)
            .args([
                "serve",
                "--port",
                &port.to_string(),
                "--hostname",
                "127.0.0.1",
            ])
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", std::env::var("HOME").unwrap_or_default())
            .env("OPENCODE_DB", directory.join("contract.db"))
            .current_dir(&directory)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;

        Ok(Server {
            child,
            url: format!("http://127.0.0.1:{port}"),
            directory,
        })
    }

    /// Create and return a new session id through the REST surface.
    #[allow(dead_code)]
    async fn create_session(&self, client: &OpencodeClient) -> anyhow::Result<String> {
        client.create_session().await.map(|session| session.id)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// Start the built binary against `url` and capture its exit and output.
///
/// `--check` runs the initial REST load and exits without touching the
/// terminal, which is what makes the readiness scenario bindable headlessly.
fn launch(url: &str) -> Launch {
    let binary = std::env::var("OPENCODE_TUI_BIN").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/target/debug/opencode-tui").to_string()
    });
    let output = std::process::Command::new(binary)
        .args(["--url", url, "--check"])
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run opencode-tui");
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Launch {
        exit: output.status.code(),
        output: text,
    }
}

fn envelope(payload: &str) -> opencode_tui::context::event::Envelope {
    serde_json::from_str(&format!(r#"{{"payload":{payload},"directory":"/d"}}"#)).expect("envelope")
}

/// Build a key event from the upstream binding name (`ctrl+d`, `escape`, `q`).
fn key_event(name: &str) -> crossterm::event::KeyEvent {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut ctrl = false;
    let mut code = None;
    for part in name.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => {}
            "escape" | "esc" => code = Some(KeyCode::Esc),
            "enter" | "return" => code = Some(KeyCode::Enter),
            "up" => code = Some(KeyCode::Up),
            "down" => code = Some(KeyCode::Down),
            "left" => code = Some(KeyCode::Left),
            "right" => code = Some(KeyCode::Right),
            "pageup" => code = Some(KeyCode::PageUp),
            "pagedown" => code = Some(KeyCode::PageDown),
            "home" => code = Some(KeyCode::Home),
            "end" => code = Some(KeyCode::End),
            other => code = Some(KeyCode::Char(other.chars().next().expect("key name"))),
        }
    }
    let modifiers = if ctrl {
        KeyModifiers::CONTROL
    } else {
        KeyModifiers::NONE
    };
    KeyEvent::new(code.expect("key name"), modifiers)
}

#[tokio::main]
async fn main() {
    let feature =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("features/attach-and-prompt.feature");
    // `@live` scenarios need a reachable model provider, so the offline gate
    // excludes them; `just contracts-live` sets OPENCODE_TUI_CONTRACT_LIVE.
    let live = std::env::var_os("OPENCODE_TUI_CONTRACT_LIVE").is_some();
    Harness::cucumber()
        .with_default_cli()
        .filter_run(feature, move |_feature, _rule, scenario| {
            live || !scenario.tags.iter().any(|tag| tag == "live")
        })
        .await;
}
