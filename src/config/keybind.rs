use crossterm::event::{KeyCode, KeyEvent};

/// Commands M1 exposes. Upstream's `CommandMap` is much larger; this is the
/// subset the read + prompt milestone binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Quit,
    OpenSession,
    Back,
}

/// Resolve a key event to a command. Defaults mirror the upstream bindings for
/// the commands M1 supports (Esc returns, it never quits); overrides from
/// `~/.config/opencode/tui.json` are a later slice.
pub fn command_for(key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Char('q') => Some(Command::Quit),
        KeyCode::Enter => Some(Command::OpenSession),
        KeyCode::Esc | KeyCode::Backspace => Some(Command::Back),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn escape_returns_and_does_not_quit() {
        assert_eq!(command_for(key(KeyCode::Esc)), Some(Command::Back));
        assert_eq!(command_for(key(KeyCode::Char('q'))), Some(Command::Quit));
        assert_eq!(command_for(key(KeyCode::Enter)), Some(Command::OpenSession));
    }
}
