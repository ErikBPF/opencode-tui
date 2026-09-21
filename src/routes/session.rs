use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::context::sdk::{Command, MessageWithParts, Permission, Session};

/// Placeholder shown while the prompt buffer is empty, mirroring upstream's
/// `Ask anything…` hint.
const PROMPT_PLACEHOLDER: &str = "Ask anything…";

/// How many command suggestions to list while typing a slash command.
const MAX_SUGGESTIONS: usize = 8;

/// Everything the session screen reads for one frame, bundled so the render
/// signature stays under the argument-count lint.
pub struct SessionView<'a> {
    pub session: Option<&'a Session>,
    pub messages: &'a [MessageWithParts],
    pub prompt: &'a str,
    pub permission: Option<&'a Permission>,
    pub scroll: u16,
    pub commands: &'a [Command],
}

/// Session screen. Mirrors upstream `routes/session/index.tsx`: the loaded
/// transcript in a bordered pane, a read-only permission banner, and a prompt
/// input box at the bottom.
///
/// Returns the total number of *wrapped* rows so the caller can clamp its scroll
/// offset; `scroll` is the top row to display.
pub fn render(frame: &mut Frame, area: Rect, view: &SessionView<'_>) -> usize {
    let SessionView {
        session,
        messages,
        prompt,
        permission,
        scroll,
        commands,
    } = *view;
    let title = session
        .map(|session| session.title.clone().unwrap_or_else(|| session.id.clone()))
        .unwrap_or_else(|| "Session".to_string());

    // The transcript and the prompt box are separate panes so the input is
    // always visible with its own border and placeholder, as upstream does.
    // While a slash command is being typed, a suggestion list sits above it,
    // mirroring the upstream autocomplete popup.
    let suggestions = slash_suggestions(prompt, commands);
    let prompt_height = 3u16;
    let suggestion_height = suggestions.len() as u16;
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Min(3),
            ratatui::layout::Constraint::Length(suggestion_height),
            ratatui::layout::Constraint::Length(prompt_height),
        ])
        .split(area);
    let transcript_area = chunks[0];
    let prompt_area = chunks[2];

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
    if !suggestions.is_empty() {
        render_suggestions(frame, chunks[1], &suggestions);
    }

    rows
}

/// One row of the slash-command autocomplete, from either the client-native
/// table or the server's `/command` list.
struct Suggestion {
    name: String,
    description: String,
}

/// Slash commands matching the current prefix: client-native entries first
/// (mirroring upstream, where `useCommandSlashes` leads the list), then the
/// server's commands. Empty unless the buffer begins with `/` and has no
/// newline yet, since the name is the first line.
fn slash_suggestions(prompt: &str, commands: &[Command]) -> Vec<Suggestion> {
    let Some(rest) = prompt.strip_prefix('/') else {
        return Vec::new();
    };
    if rest.contains('\n') {
        return Vec::new();
    }
    let prefix = rest.split_whitespace().next().unwrap_or("");

    let native = crate::config::keybind::NATIVE_SLASHES
        .iter()
        .filter(|entry| entry.name.starts_with(prefix))
        .map(|entry| Suggestion {
            name: entry.name.to_string(),
            description: entry.command.description().to_string(),
        });
    let server = commands
        .iter()
        .filter(|command| command.name.starts_with(prefix))
        .map(|command| Suggestion {
            name: command.name.clone(),
            description: command.description.clone().unwrap_or_default(),
        });

    native.chain(server).take(MAX_SUGGESTIONS).collect()
}

/// The autocomplete popup: name on the left, description on the right.
fn render_suggestions(frame: &mut Frame, area: Rect, suggestions: &[Suggestion]) {
    let lines: Vec<Line> = suggestions
        .iter()
        .map(|suggestion| {
            Line::from(vec![
                Span::styled(
                    format!("/{:<16}", suggestion.name),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(suggestion.description.clone()),
            ])
        })
        .collect();
    let widget = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::TOP).title("Commands"));
    frame.render_widget(widget, area);
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
    use super::{slash_suggestions, wrapped_height};
    use crate::context::sdk::Command;

    #[test]
    fn wrapped_height_counts_rows_and_never_zero() {
        assert_eq!(wrapped_height(0, 10), 1);
        assert_eq!(wrapped_height(10, 10), 1);
        assert_eq!(wrapped_height(11, 10), 2);
        assert_eq!(wrapped_height(25, 10), 3);
    }

    #[test]
    fn slash_prefix_suggests_matching_commands() {
        let commands = vec![
            Command {
                name: "init".to_string(),
                description: Some("setup".to_string()),
            },
            Command {
                name: "review".to_string(),
                description: None,
            },
        ];
        assert!(slash_suggestions("hello", &commands).is_empty());
        assert!(slash_suggestions("/", &commands).len() >= 2);
        let matched = slash_suggestions("/re", &commands);
        assert!(matched.iter().any(|s| s.name == "review"));
        // Once the name is followed by a newline the popup closes.
        assert!(slash_suggestions("/init\nmore", &commands).is_empty());
    }

    #[test]
    fn slash_suggestions_lead_with_native_commands() {
        let suggestions = slash_suggestions("/", &[]);
        let names: Vec<&str> = suggestions.iter().map(|s| s.name.as_str()).collect();
        // Client-native entries come first, mirroring upstream's ordering.
        assert_eq!(&names[..4], &["exit", "new", "sessions", "help"]);
    }
}
