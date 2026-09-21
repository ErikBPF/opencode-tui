//! Keybinding engine. Mirrors upstream `config/keybind.ts` (definition defaults
//! and override parsing) plus `keymap.tsx` (resolving a key event to a command).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Commands M1 exposes, named exactly as upstream so `tui.json` overrides and
/// the upstream docs line up. The rest of upstream's `CommandMap` arrives with
/// the screens that use it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// `app_exit`: leave the client.
    AppExit,
    /// `command_list`: show the available commands.
    CommandList,
    /// `session_submit` / `input_submit`: open the selected session on the home
    /// screen, or submit the prompt in a session.
    InputSubmit,
    /// `session_back`: return to the home screen.
    SessionBack,
    /// `session_new`: create a session and open it.
    SessionNew,
    /// `session_interrupt`: abort the running turn.
    SessionInterrupt,
    /// `session_next` / `session_previous`: move the home-screen selection.
    SessionNext,
    SessionPrevious,
    /// `messages_page_up` / `messages_page_down`: scroll the transcript.
    MessagesPageUp,
    MessagesPageDown,
    /// `messages_first` / `messages_last`: jump to the transcript bounds.
    MessagesFirst,
    MessagesLast,
}

impl Command {
    /// Every command the client knows, in the order the palette lists them.
    pub const ALL: [Command; 12] = [
        Command::AppExit,
        Command::CommandList,
        Command::InputSubmit,
        Command::SessionBack,
        Command::SessionNew,
        Command::SessionInterrupt,
        Command::SessionNext,
        Command::SessionPrevious,
        Command::MessagesPageUp,
        Command::MessagesPageDown,
        Command::MessagesFirst,
        Command::MessagesLast,
    ];

    /// The upstream `tui.json` `keybinds` name.
    pub fn name(self) -> &'static str {
        match self {
            Command::AppExit => "app_exit",
            Command::CommandList => "command_list",
            Command::InputSubmit => "input_submit",
            Command::SessionBack => "session_back",
            Command::SessionNew => "session_new",
            Command::SessionInterrupt => "session_interrupt",
            Command::SessionNext => "session_next",
            Command::SessionPrevious => "session_previous",
            Command::MessagesPageUp => "messages_page_up",
            Command::MessagesPageDown => "messages_page_down",
            Command::MessagesFirst => "messages_first",
            Command::MessagesLast => "messages_last",
        }
    }

    /// Human description, mirroring upstream's `Definitions` text.
    pub fn description(self) -> &'static str {
        match self {
            Command::AppExit => "Exit the application",
            Command::CommandList => "List available commands",
            Command::InputSubmit => "Open the selected session or submit the prompt",
            Command::SessionBack => "Return to the session list",
            Command::SessionNew => "Create a new session",
            Command::SessionInterrupt => "Interrupt the current session",
            Command::SessionNext => "Select the next session",
            Command::SessionPrevious => "Select the previous session",
            Command::MessagesPageUp => "Scroll messages up by one page",
            Command::MessagesPageDown => "Scroll messages down by one page",
            Command::MessagesFirst => "Navigate to the first message",
            Command::MessagesLast => "Navigate to the last message",
        }
    }
}

/// A binding: one or more key strokes that map to a command. `none`/`false`
/// disables the command, which is how upstream lets a user unbind a default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Binding {
    Keys(Vec<Stroke>),
    Disabled,
}

/// A single key stroke parsed from the upstream string form (`ctrl+c`,
/// `escape`, `enter`). `leader` is resolved against the configured leader key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stroke {
    pub code: KeyCode,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub leader: bool,
}

impl Stroke {
    /// Does this stroke match a live key event? Shift is only compared for
    /// character keys, since terminals report shifted characters inconsistently.
    pub fn matches(&self, key: KeyEvent) -> bool {
        if self.leader || self.code != key.code {
            return false;
        }
        if self.ctrl != key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        if self.alt != key.modifiers.contains(KeyModifiers::ALT) {
            return false;
        }
        if matches!(self.code, KeyCode::Char(_)) {
            self.shift == key.modifiers.contains(KeyModifiers::SHIFT)
        } else {
            true
        }
    }
}

/// The leader key default, mirroring upstream `LeaderDefault`.
pub const LEADER_DEFAULT: &str = "ctrl+x";

/// The built-in bindings, mirroring the upstream `Definitions` defaults for the
/// commands M1 exposes. Plain `q` is intentionally absent so it stays typable
/// in the prompt; the home screen handles it explicitly.
pub fn defaults() -> Vec<(Command, Binding)> {
    let stroke = |code, ctrl, shift, alt| {
        Binding::Keys(vec![Stroke {
            code,
            ctrl,
            shift,
            alt,
            leader: false,
        }])
    };
    let leader = |code| {
        Binding::Keys(vec![Stroke {
            code,
            ctrl: false,
            shift: false,
            alt: false,
            leader: true,
        }])
    };
    vec![
        (
            Command::AppExit,
            Binding::Keys(vec![
                Stroke {
                    code: KeyCode::Char('c'),
                    ctrl: true,
                    shift: false,
                    alt: false,
                    leader: false,
                },
                Stroke {
                    code: KeyCode::Char('d'),
                    ctrl: true,
                    shift: false,
                    alt: false,
                    leader: false,
                },
                Stroke {
                    code: KeyCode::Char('q'),
                    ctrl: false,
                    shift: false,
                    alt: false,
                    leader: true,
                },
            ]),
        ),
        (
            Command::CommandList,
            stroke(KeyCode::Char('p'), true, false, false),
        ),
        (
            Command::InputSubmit,
            stroke(KeyCode::Enter, false, false, false),
        ),
        (Command::SessionNew, leader(KeyCode::Char('n'))),
        (
            Command::SessionInterrupt,
            stroke(KeyCode::Esc, false, false, false),
        ),
        (
            Command::SessionNext,
            stroke(KeyCode::Down, false, false, false),
        ),
        (
            Command::SessionPrevious,
            stroke(KeyCode::Up, false, false, false),
        ),
        (
            Command::MessagesPageUp,
            stroke(KeyCode::PageUp, false, false, false),
        ),
        (
            Command::MessagesPageDown,
            stroke(KeyCode::PageDown, false, false, false),
        ),
        (
            Command::MessagesFirst,
            stroke(KeyCode::Home, false, false, false),
        ),
        (
            Command::MessagesLast,
            stroke(KeyCode::End, false, false, false),
        ),
    ]
}

/// Parsed bindings ready for lookup: the resolved table plus any unknown keys
/// the user set, which are reported rather than silently ignored (upstream
/// `parse` throws on them).
#[derive(Clone, Debug)]
pub struct Keybinds {
    bindings: Vec<(Command, Binding)>,
    leader: Binding,
    pub unknown: Vec<String>,
}

impl Keybinds {
    /// Merge user overrides over the defaults. Values use the upstream string
    /// form: `"ctrl+c"`, `"escape,q"`, or `"none"`/`false` to unbind.
    pub fn resolve(overrides: &serde_json::Map<String, serde_json::Value>) -> Self {
        let mut bindings = defaults();
        let mut leader = Binding::Keys(vec![Stroke {
            code: KeyCode::Char('x'),
            ctrl: true,
            shift: false,
            alt: false,
            leader: false,
        }]);
        let mut unknown = Vec::new();
        for (name, value) in overrides {
            if name == "leader" {
                match parse_binding(value) {
                    Some(binding) => leader = binding,
                    None => unknown.push(name.clone()),
                }
                continue;
            }
            let Some(command) = Command::ALL.into_iter().find(|c| c.name() == name) else {
                unknown.push(name.clone());
                continue;
            };
            match parse_binding(value) {
                Some(binding) => {
                    if let Some(slot) = bindings.iter_mut().find(|(c, _)| *c == command) {
                        slot.1 = binding;
                    }
                }
                None => unknown.push(name.clone()),
            }
        }
        unknown.sort();
        Self {
            bindings,
            leader,
            unknown,
        }
    }

    /// The human form of a command's binding, for the palette. Leader strokes
    /// render as `<leader>q`; a disabled binding renders as `none`.
    pub fn describe(&self, command: Command) -> String {
        let binding = self
            .bindings
            .iter()
            .find(|(candidate, _)| *candidate == command)
            .map(|(_, binding)| binding);
        match binding {
            Some(Binding::Disabled) | None => "none".to_string(),
            Some(Binding::Keys(strokes)) => strokes
                .iter()
                .map(describe_stroke)
                .collect::<Vec<_>>()
                .join(","),
        }
    }

    /// Resolve a key event to a command, or `None` when unbound.
    pub fn command_for(&self, key: KeyEvent) -> Option<Command> {
        self.bindings
            .iter()
            .find_map(|(command, binding)| match binding {
                Binding::Keys(strokes) if strokes.iter().any(|stroke| stroke.matches(key)) => {
                    Some(*command)
                }
                _ => None,
            })
    }

    /// Is this key the configured leader? Resolved separately because a leader
    /// prefix is a state transition, not a command.
    pub fn is_leader(&self, key: KeyEvent) -> bool {
        matches!(&self.leader, Binding::Keys(strokes) if strokes.iter().any(|stroke| stroke.matches(key)))
    }

    /// The command a leader-prefixed key resolves to.
    pub fn leader_command_for(&self, key: KeyEvent) -> Option<Command> {
        self.bindings
            .iter()
            .find_map(|(command, binding)| match binding {
                Binding::Keys(strokes)
                    if strokes
                        .iter()
                        .any(|stroke| stroke.leader && stroke.code == key.code) =>
                {
                    Some(*command)
                }
                _ => None,
            })
    }
}

/// Parse one override value. Returns `None` for a malformed value so the caller
/// can surface it as unknown rather than guessing.
fn parse_binding(value: &serde_json::Value) -> Option<Binding> {
    match value {
        serde_json::Value::Bool(false) => Some(Binding::Disabled),
        serde_json::Value::String(text) => {
            if text == "none" {
                return Some(Binding::Disabled);
            }
            let strokes = text
                .split(',')
                .map(str::trim)
                .map(parse_stroke)
                .collect::<Option<Vec<_>>>()?;
            if strokes.is_empty() {
                None
            } else {
                Some(Binding::Keys(strokes))
            }
        }
        _ => None,
    }
}

/// Parse one stroke such as `ctrl+c`, `escape`, or `<leader>q`.
fn parse_stroke(input: &str) -> Option<Stroke> {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut leader = false;
    let mut code = None;
    for part in input.split('+') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.to_ascii_lowercase().as_str() {
            "<leader>" | "leader" => leader = true,
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "meta" | "alt" | "option" => alt = true,
            _ if code.is_some() => return None,
            other => code = Some(parse_code(other)?),
        }
    }
    Some(Stroke {
        code: code?,
        ctrl,
        shift,
        alt,
        leader,
    })
}

fn describe_stroke(stroke: &Stroke) -> String {
    let mut parts = Vec::new();
    if stroke.leader {
        parts.push("leader".to_string());
    }
    if stroke.ctrl {
        parts.push("ctrl".to_string());
    }
    if stroke.shift {
        parts.push("shift".to_string());
    }
    if stroke.alt {
        parts.push("alt".to_string());
    }
    parts.push(match stroke.code {
        KeyCode::Char(character) => character.to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Esc => "escape".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        other => format!("{other:?}").to_ascii_lowercase(),
    });
    parts.join("+")
}

fn parse_code(name: &str) -> Option<KeyCode> {
    Some(match name {
        "enter" | "return" => KeyCode::Enter,
        "escape" | "esc" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        other => {
            let mut chars = other.chars();
            let first = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            KeyCode::Char(first)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(character: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(character), KeyModifiers::CONTROL)
    }

    fn overrides(json: &str) -> serde_json::Map<String, serde_json::Value> {
        serde_json::from_str(json).expect("json object")
    }

    #[test]
    fn defaults_bind_the_core_commands_only() {
        let bindings = Keybinds::resolve(&overrides("{}"));
        assert!(bindings.is_leader(ctrl('x')));
        assert_eq!(
            bindings.leader_command_for(key(KeyCode::Char('q'))),
            Some(Command::AppExit)
        );
        assert_eq!(
            bindings.leader_command_for(key(KeyCode::Char('n'))),
            Some(Command::SessionNew)
        );
        assert_eq!(bindings.command_for(ctrl('c')), Some(Command::AppExit));
        assert_eq!(bindings.command_for(ctrl('d')), Some(Command::AppExit));
        assert_eq!(bindings.command_for(ctrl('p')), Some(Command::CommandList));
        assert_eq!(
            bindings.command_for(key(KeyCode::Enter)),
            Some(Command::InputSubmit)
        );
        assert_eq!(
            bindings.command_for(key(KeyCode::Esc)),
            Some(Command::SessionInterrupt)
        );
        assert_eq!(
            bindings.command_for(key(KeyCode::Down)),
            Some(Command::SessionNext)
        );
        assert_eq!(
            bindings.command_for(key(KeyCode::Up)),
            Some(Command::SessionPrevious)
        );
        assert_eq!(
            bindings.command_for(key(KeyCode::PageUp)),
            Some(Command::MessagesPageUp)
        );
        assert_eq!(
            bindings.command_for(key(KeyCode::PageDown)),
            Some(Command::MessagesPageDown)
        );
        assert_eq!(
            bindings.command_for(key(KeyCode::Home)),
            Some(Command::MessagesFirst)
        );
        assert_eq!(
            bindings.command_for(key(KeyCode::End)),
            Some(Command::MessagesLast)
        );
        assert_eq!(bindings.command_for(key(KeyCode::Char('q'))), None);
    }

    #[test]
    fn overrides_replace_defaults_and_accept_lists() {
        let bindings = Keybinds::resolve(&overrides(
            r#"{"app_exit":"ctrl+d,q","input_submit":"ctrl+j","leader":"ctrl+a"}"#,
        ));
        assert_eq!(bindings.command_for(ctrl('d')), Some(Command::AppExit));
        assert_eq!(
            bindings.command_for(key(KeyCode::Char('q'))),
            Some(Command::AppExit)
        );
        assert_eq!(bindings.command_for(ctrl('j')), Some(Command::InputSubmit));
        assert!(bindings.is_leader(ctrl('a')));
        assert!(!bindings.is_leader(ctrl('x')));
    }

    #[test]
    fn none_or_false_disables_a_binding() {
        // Escape is bound to `session_interrupt`, not `session_back`, so
        // disabling `session_back` leaves Escape working.
        let bindings = Keybinds::resolve(&overrides(r#"{"session_interrupt":"none"}"#));
        assert_eq!(bindings.command_for(key(KeyCode::Esc)), None);
        let bindings = Keybinds::resolve(&overrides(r#"{"app_exit":false}"#));
        assert_eq!(bindings.command_for(ctrl('c')), None);
    }

    #[test]
    fn unknown_or_malformed_keys_are_reported_not_applied() {
        let bindings = Keybinds::resolve(&overrides(
            r#"{"not_a_command":"ctrl+x","app_exit":"ctrl++"}"#,
        ));
        assert_eq!(bindings.unknown, vec!["app_exit", "not_a_command"]);
        assert_eq!(bindings.command_for(ctrl('c')), Some(Command::AppExit));
    }

    #[test]
    fn every_command_has_a_distinct_name() {
        let mut names: Vec<&str> = Command::ALL.iter().map(|c| c.name()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total);
    }
}
