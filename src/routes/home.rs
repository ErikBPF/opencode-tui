use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::context::sync::Store;

/// The startup logo, mirroring upstream `logo.ts`: the left half is muted and
/// the right half is normal. `_` is a shadowed space, `^` and `~` are shadow
/// halves of a block, `,` a lower half.
const LOGO_LEFT: [&str; 4] = [
    "                   ",
    "█▀▀█ █▀▀█ █▀▀█ █▀▀▄",
    "█__█ █__█ █^^^ █__█",
    "▀▀▀▀ █▀▀▀ ▀▀▀▀ ▀~~▀",
];
const LOGO_RIGHT: [&str; 4] = [
    "             ▄     ",
    "█▀▀▀ █▀▀█ █▀▀█ █▀▀█",
    "█___ █__█ █__█ █^^^",
    "▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀",
];

/// Placeholder examples shown in the empty prompt, mirroring upstream's
/// `placeholder.normal`.
const PLACEHOLDERS: [&str; 3] = [
    "Fix a TODO in the codebase",
    "What is the tech stack of this project?",
    "Fix broken tests",
];

pub const PLACEHOLDER: &str = "Ask anything…";

/// The home screen's live inputs. Bundled to stay under the argument-count
/// lint; mirrors upstream `routes/home.tsx`.
pub struct HomeView<'a> {
    pub store: &'a Store,
    pub home: &'a str,
    pub selected: usize,
    pub prompt: &'a str,
    /// The model the server routes to when none is chosen, e.g.
    /// `litellm/deepseek-v4.1-flash`.
    pub model: Option<&'a str>,
}

/// Home screen: the logo, an input line, the footer, and the session list when
/// the user asks for it. Mirrors upstream `routes/home.tsx`.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    view: &HomeView<'_>,
    show_sessions: bool,
) -> (usize, usize) {
    let Some(model) = view.model else {
        // No server-reported model yet. The session list is the only thing we
        // can show, so it fills the area.
        render_sessions(frame, area, view);
        return (0, area.height as usize);
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(9), Constraint::Length(1)])
        .split(area);
    let body = chunks[0];

    if show_sessions {
        render_sessions(frame, body, view);
    } else {
        render_welcome(frame, body, view, model);
    }

    let footer = if show_sessions {
        " sessions · enter opens · ctrl+x n new · esc back ".to_string()
    } else {
        format!(
            " {} | {:?} | enter to run · ctrl+x l sessions · ctrl+x n new | {model} ",
            view.home, view.store.status
        )
    };
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(Color::DarkGray)),
        chunks[1],
    );
    (0, body.height as usize)
}

/// The startup screen: logo, model line, prompt, and the session hint.
fn render_welcome(frame: &mut Frame, area: Rect, view: &HomeView<'_>, model: &str) {
    let mut lines: Vec<Line> = Vec::new();
    for (left, right) in LOGO_LEFT.iter().zip(LOGO_RIGHT.iter()) {
        lines.push(Line::from(vec![
            Span::styled(*left, Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(*right, Style::default().add_modifier(Modifier::BOLD)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  model  ", Style::default().fg(Color::DarkGray)),
        Span::styled(model.to_string(), Style::default().fg(Color::Cyan)),
    ]));
    lines.push(Line::from(""));

    let prompt = if view.prompt.is_empty() {
        Line::from(Span::styled(
            format!("  {PLACEHOLDER}  \"{}\"", PLACEHOLDERS[0]),
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::from(format!("  > {}", view.prompt))
    };
    lines.push(prompt);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("  {}", session_hint(view.store)),
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title("opencode")),
        area,
    );
}

/// What the session hint says, so an empty server does not silently look broken.
fn session_hint(store: &Store) -> String {
    let count = store.sessions.len();
    match count {
        0 => "No sessions yet — ctrl+x n starts one".to_string(),
        1 => "1 session — ctrl+x l to browse, enter to start typing".to_string(),
        other => format!("{other} sessions — ctrl+x l to browse, enter to start typing"),
    }
}

/// The session list, shown on demand or when the server reports no model.
fn render_sessions(frame: &mut Frame, area: Rect, view: &HomeView<'_>) {
    let selected = if view.store.sessions.is_empty() {
        0
    } else {
        view.selected.min(view.store.sessions.len() - 1)
    };
    let mut lines: Vec<Line> = Vec::new();
    for (index, session) in view.store.sessions.iter().enumerate() {
        let label = session.title.clone().unwrap_or_else(|| session.id.clone());
        let marker = if index == selected { "> " } else { "  " };
        let style = if index == selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(format!("{marker}{label}"), style)));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No sessions yet — ctrl+x n starts one",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Sessions")
                .title_bottom(" enter open · ctrl+x n new · esc back "),
        ),
        area,
    );
}
