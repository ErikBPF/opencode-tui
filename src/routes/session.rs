use ratatui::layout::Rect;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::context::sdk::Session;

/// Session screen. Mirrors upstream `routes/session/index.tsx`; the transcript
/// and part renderers land in the next slice.
pub fn render(frame: &mut Frame, area: Rect, session: Option<&Session>) {
    let title = session
        .map(|session| session.title.clone().unwrap_or_else(|| session.id.clone()))
        .unwrap_or_else(|| "Session".to_string());

    let body = Text::raw("Transcript streaming is the next slice.");
    let paragraph = Paragraph::new(body)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
