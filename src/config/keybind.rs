//! Keybinding engine. Mirrors upstream `config/keybind.ts` (definition defaults
//! and override parsing) plus `keymap.tsx` (resolving a key event to a command).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Commands M1 exposes. Upstream's `CommandMap` is much larger; this is the
/// subset the read + prompt milestone binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Quit,
    /// Enter: opens a session on the home screen, submits the prompt in a
    /// session. The caller decides by route.
    Enter,
    Back,
}

impl Command {
    /// The upstream command name used as the `tui.json` `keybinds` key.
    pub fn name(self) -> &'static str {
        match self {
            Command::Quit => "app_exit",
            Command::Enter => "session_submit",
            Command::Back => "session_back",
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
/// `escape`, `enter`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stroke {
    pub code: KeyCode,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Stroke {
    /// Does this stroke match a live key event? Shift is only compared for
    /// character keys, since terminals report shifted characters inconsistently.
    pub fn matches(&self, key: KeyEvent) -> bool {
        if self.code != key.code {
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

/// The built-in bindings, mirroring the upstream `Definitions` defaults for the
/// commands M1 exposes. Plain `q` is intentionally absent so it stays typable
/// in the prompt; the home screen handles it explicitly.
pub fn defaults() -> Vec<(Command, Binding)> {
    vec![
        (
            Command::Quit,
            Binding::Keys(vec![Stroke {
                code: KeyCode::Char('c'),
                ctrl: true,
                shift: false,
                alt: false,
            }]),
        ),
        (
            Command::Enter,
            Binding::Keys(vec![Stroke {
                code: KeyCode::Enter,
                ctrl: false,
                shift: false,
                alt: false,
            }]),
        ),
        (
            Command::Back,
            Binding::Keys(vec![Stroke {
                code: KeyCode::Esc,
                ctrl: false,
                shift: false,
                alt: false,
            }]),
        ),
    ]
}

/// Parsed bindings ready for lookup: the resolved table plus any unknown keys
/// the user set, which are reported rather than silently ignored (upstream
/// `parse` throws on them).
#[derive(Clone, Debug)]
pub struct Keybinds {
    bindings: Vec<(Command, Binding)>,
    pub unknown: Vec<String>,
}

impl Keybinds {
    /// Merge user overrides over the defaults. Values use the upstream string
    /// form: `"ctrl+c"`, `"escape,q"`, or `"none"`/`false` to unbind.
    pub fn resolve(overrides: &serde_json::Map<String, serde_json::Value>) -> Self {
        let mut bindings = defaults();
        let mut unknown = Vec::new();
        for (name, value) in overrides {
            let Some(command) = command_by_name(name) else {
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
        Self { bindings, unknown }
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
}

fn command_by_name(name: &str) -> Option<Command> {
    [Command::Quit, Command::Enter, Command::Back]
        .into_iter()
        .find(|command| command.name() == name)
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

/// Parse one stroke such as `ctrl+c`, `escape`, or `enter`.
fn parse_stroke(input: &str) -> Option<Stroke> {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut code = None;
    for part in input.split('+') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.to_ascii_lowercase().as_str() {
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
    })
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

    fn overrides(json: &str) -> serde_json::Map<String, serde_json::Value> {
        serde_json::from_str(json).expect("json object")
    }

    #[test]
    fn defaults_bind_enter_escape_and_control_c_only() {
        let bindings = Keybinds::resolve(&overrides("{}"));
        assert_eq!(
            bindings.command_for(key(KeyCode::Enter)),
            Some(Command::Enter)
        );
        assert_eq!(bindings.command_for(key(KeyCode::Esc)), Some(Command::Back));
        assert_eq!(
            bindings.command_for(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Command::Quit)
        );
        assert_eq!(bindings.command_for(key(KeyCode::Char('q'))), None);
    }

    #[test]
    fn overrides_replace_defaults_and_accept_lists() {
        let bindings = Keybinds::resolve(&overrides(
            r#"{"app_exit":"ctrl+d,q","session_submit":"ctrl+j"}"#,
        ));
        assert_eq!(
            bindings.command_for(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
            Some(Command::Quit)
        );
        assert_eq!(
            bindings.command_for(key(KeyCode::Char('q'))),
            Some(Command::Quit)
        );
        assert_eq!(
            bindings.command_for(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL)),
            Some(Command::Enter)
        );
    }

    #[test]
    fn none_or_false_disables_a_binding() {
        let bindings = Keybinds::resolve(&overrides(r#"{"session_back":"none"}"#));
        assert_eq!(bindings.command_for(key(KeyCode::Esc)), None);
        let bindings = Keybinds::resolve(&overrides(r#"{"app_exit":false}"#));
        assert_eq!(
            bindings.command_for(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            None
        );
    }

    #[test]
    fn unknown_or_malformed_keys_are_reported_not_applied() {
        let bindings = Keybinds::resolve(&overrides(
            r#"{"not_a_command":"ctrl+x","app_exit":"ctrl++"}"#,
        ));
        assert_eq!(bindings.unknown, vec!["app_exit", "not_a_command"]);
        assert_eq!(
            bindings.command_for(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Command::Quit)
        );
    }
}
