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

/// Resolve a key event to a command. Plain `q` is intentionally left unbound so
/// it stays typable in the prompt; the home screen handles it explicitly.
/// Overrides from `~/.config/opencode/tui.json` are a later slice.
pub fn command_for(key: KeyEvent) -> Option<Command> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('c') if control => Some(Command::Quit),
        KeyCode::Enter => Some(Command::Enter),
        KeyCode::Esc => Some(Command::Back),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn escape_returns_and_control_c_quits() {
        assert_eq!(command_for(key(KeyCode::Esc)), Some(Command::Back));
        assert_eq!(command_for(key(KeyCode::Enter)), Some(Command::Enter));
        assert_eq!(
            command_for(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Command::Quit)
        );
        assert_eq!(command_for(key(KeyCode::Char('q'))), None);
    }
}
