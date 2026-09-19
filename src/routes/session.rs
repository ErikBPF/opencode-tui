use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::context::sdk::{MessageWithParts, Session};

/// Session screen. Mirrors upstream `routes/session/index.tsx`: the loaded
/// transcript, grouped by message and rendered by part type.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    session: Option<&Session>,
    messages: &[MessageWithParts],
) {
    let title = session
        .map(|session| session.title.clone().unwrap_or_else(|| session.id.clone()))
        .unwrap_or_else(|| "Session".to_string());

    let mut lines: Vec<Line> = Vec::new();
    for message in messages {
        lines.push(
            Line::from(format!("{}:", message.info.role))
                .style(Style::default().add_modifier(Modifier::BOLD)),
        );
        for part in &message.parts {
            let display = part.part.display();
            if !display.is_empty() {
                lines.push(Line::from(display));
            }
        }
        lines.push(Line::from(""));
    }
    if lines.is_empty() {
        lines.push(Line::from("No messages yet."));
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
