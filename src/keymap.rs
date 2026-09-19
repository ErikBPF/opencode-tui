use crossterm::event::KeyEvent;

pub use crate::config::keybind::Command;

/// Thin wrapper over the keybinding definitions, matching the upstream split
/// between declarative definitions and the live keymap.
pub fn command_for(key: KeyEvent) -> Option<Command> {
    crate::config::keybind::command_for(key)
}
