use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::context::sdk::{MessageWithParts, Permission, Session};

/// Session screen. Mirrors upstream `routes/session/index.tsx`: the loaded
/// transcript, grouped by message and rendered by part type, plus a read-only
/// permission banner when the server is waiting for an answer.
///
/// Returns the total number of *wrapped* rows so the caller can clamp its scroll
/// offset; `scroll` is the top row to display.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    session: Option<&Session>,
    messages: &[MessageWithParts],
    prompt: &str,
    permission: Option<&Permission>,
    scroll: u16,
) -> usize {
    let title = session
        .map(|session| session.title.clone().unwrap_or_else(|| session.id.clone()))
        .unwrap_or_else(|| "Session".to_string());

    let mut lines: Vec<Line> = Vec::new();
    if let Some(permission) = permission {
        lines.push(
            Line::from(permission.display()).style(Style::default().add_modifier(Modifier::BOLD)),
        );
        lines.push(Line::from("(read-only: answering is not supported yet)"));
        lines.push(Line::from(""));
    }
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
    // The prompt line is always present so an empty session still shows where to
    // type; it is the last row and the caller bottom-anchors the viewport on it.
    lines.push(Line::from(""));
    lines.push(Line::from(format!("> {prompt}")));

    // Wrapped rows, not source lines: a long text part occupies several rows,
    // and the caller needs the real height to clamp and bottom-anchor scroll.
    let width = area.width.saturating_sub(2).max(1) as usize;
    let rows: usize = lines
        .iter()
        .map(|line| wrapped_height(line.width(), width))
        .sum();

    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, area);
    rows
}

/// Rows a line of `line_width` cell-widths occupies when wrapped at `width`.
fn wrapped_height(line_width: usize, width: usize) -> usize {
    line_width.div_ceil(width).max(1)
}

#[cfg(test)]
mod tests {
    use super::wrapped_height;

    #[test]
    fn wrapped_height_counts_rows_and_never_zero() {
        assert_eq!(wrapped_height(0, 10), 1);
        assert_eq!(wrapped_height(10, 10), 1);
        assert_eq!(wrapped_height(11, 10), 2);
        assert_eq!(wrapped_height(25, 10), 3);
    }
}
