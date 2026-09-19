use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::context::sync::Store;

/// Home screen: the server's session list. Mirrors upstream `routes/home.tsx`.
pub fn render(frame: &mut Frame, area: Rect, store: &Store, selected: usize) {
    let items: Vec<ListItem> = store
        .sessions
        .iter()
        .map(|session| {
            let label = session.title.clone().unwrap_or_else(|| session.id.clone());
            ListItem::new(Line::from(Span::raw(label)))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Sessions"))
        .highlight_symbol("> ");

    let mut state = ListState::default();
    if !store.sessions.is_empty() {
        state.select(Some(selected.min(store.sessions.len() - 1)));
    }
    frame.render_stateful_widget(list, area, &mut state);
}
