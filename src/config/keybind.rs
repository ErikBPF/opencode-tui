use crossterm::event::{KeyCode, KeyEvent};

/// Commands M1 exposes. Upstream's `CommandMap` is much larger; this is the
/// subset the read + prompt milestone binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Quit,
    Refresh,
    OpenSession,
    Back,
}

/// Resolve a key event to a command. Defaults mirror the upstream bindings for
/// the commands M1 supports; overrides from `~/.config/opencode/tui.json` are
/// a later slice.
pub fn command_for(key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Some(Command::Quit),
        KeyCode::Char('r') => Some(Command::Refresh),
        KeyCode::Enter => Some(Command::OpenSession),
        KeyCode::Backspace => Some(Command::Back),
        _ => None,
    }
}
