use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::context::sdk::{MessageWithParts, Permission, Session};

/// Placeholder shown while the prompt buffer is empty, mirroring upstream's
/// `Ask anything…` hint.
const PROMPT_PLACEHOLDER: &str = "Ask anything…";

/// Session screen. Mirrors upstream `routes/session/index.tsx`: the loaded
/// transcript in a bordered pane, a read-only permission banner, and a prompt
/// input box at the bottom.
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

    // The transcript and the prompt box are separate panes so the input is
    // always visible with its own border and placeholder, as upstream does.
    let prompt_height = 3u16;
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Min(3),
            ratatui::layout::Constraint::Length(prompt_height),
        ])
        .split(area);
    let transcript_area = chunks[0];
    let prompt_area = chunks[1];

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

    // Wrapped rows, not source lines: a long text part occupies several rows,
    // and the caller needs the real height to clamp and bottom-anchor scroll.
    let width = transcript_area.width.saturating_sub(2).max(1) as usize;
    let rows: usize = lines
        .iter()
        .map(|line| wrapped_height(line.width(), width))
        .sum();

    let transcript = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(transcript, transcript_area);

    render_prompt(frame, prompt_area, prompt);

    rows
}

/// The prompt input box: a bordered pane that always shows where to type and
/// displays a muted placeholder while the buffer is empty.
fn render_prompt(frame: &mut Frame, area: Rect, prompt: &str) {
    let body = if prompt.is_empty() {
        Line::from(Span::styled(
            PROMPT_PLACEHOLDER,
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::from(format!("> {prompt}"))
    };
    let widget = Paragraph::new(Text::from(body)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Prompt")
            .title_bottom(" enter to send "),
    );
    frame.render_widget(widget, area);
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
