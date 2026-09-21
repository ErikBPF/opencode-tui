use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem};
use ratatui::Frame;

use crate::keymap::{Command, Keybinds};

/// Command palette overlay, mirroring upstream `component/command-palette.tsx`
/// in read-only form: it lists the bound commands and their current keys.
pub fn render(frame: &mut Frame, area: Rect, keybinds: &Keybinds) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(rows[1]);

    let items: Vec<ListItem> = Command::ALL
        .iter()
        .map(|command| {
            let keys = keybinds.describe(*command);
            let line = Line::from(vec![
                Span::raw(format!("{:<18}", command.name())),
                Span::raw(format!("{:<24}", keys)),
                Span::raw(command.description()),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Commands (any key closes)"),
    );

    frame.render_widget(Clear, columns[1]);
    frame.render_widget(list, columns[1]);
}
