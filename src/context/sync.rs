use crate::context::event::{Envelope, Event};
use crate::context::sdk::Session;

/// Local state lifecycle. Mirrors upstream `context/sync.tsx`'s
/// `loading | partial | complete` status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Loading,
    Partial,
    Complete,
}

/// Server state projected into the client. Mirrors the upstream sync store,
/// minus the reducers M1 does not yet need.
#[derive(Default)]
pub struct Store {
    pub status: Status,
    pub sessions: Vec<Session>,
}

impl Store {
    /// Initial REST load completed.
    pub fn loaded(&mut self, sessions: Vec<Session>) {
        self.sessions = sessions;
        self.status = Status::Complete;
    }

    /// The event stream dropped; keep what we have but stop claiming fresh.
    pub fn degraded(&mut self) {
        if self.status != Status::Loading {
            self.status = Status::Partial;
        }
    }

    /// Feed one server event through the reducers.
    pub fn apply(&mut self, envelope: &Envelope) {
        match &envelope.payload {
            Event::ServerConnected { .. } => self.status = Status::Complete,
            Event::SessionUpdated { properties } => {
                let info = properties.get("info").cloned().unwrap_or_default();
                if let Ok(session) = serde_json::from_value::<Session>(info) {
                    self.upsert(session);
                }
            }
            Event::SessionDeleted { properties } => {
                if let Some(id) = properties
                    .get("info")
                    .and_then(|info| info.get("id"))
                    .and_then(|id| id.as_str())
                {
                    self.sessions.retain(|session| session.id != id);
                }
            }
            _ => {}
        }
    }

    fn upsert(&mut self, session: Session) {
        match self
            .sessions
            .iter_mut()
            .find(|existing| existing.id == session.id)
        {
            Some(existing) => *existing = session,
            None => self.sessions.push(session),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(json: &str) -> Envelope {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn connect_marks_the_store_complete() {
        let mut store = Store::default();
        assert_eq!(store.status, Status::Loading);
        store.apply(&envelope(
            r#"{"payload":{"type":"server.connected"},"directory":"/d"}"#,
        ));
        assert_eq!(store.status, Status::Complete);
    }

    #[test]
    fn session_updates_upsert_and_session_deletes_prune() {
        let mut store = Store::default();
        store.apply(&envelope(
            r#"{"payload":{"type":"session.updated","properties":{"info":{"id":"s1","title":"One"}}},"directory":"/d"}"#,
        ));
        store.apply(&envelope(
            r#"{"payload":{"type":"session.updated","properties":{"info":{"id":"s1","title":"One (renamed)"}}},"directory":"/d"}"#,
        ));
        assert_eq!(store.sessions.len(), 1);
        assert_eq!(store.sessions[0].title.as_deref(), Some("One (renamed)"));

        store.apply(&envelope(
            r#"{"payload":{"type":"session.deleted","properties":{"info":{"id":"s1"}}},"directory":"/d"}"#,
        ));
        assert!(store.sessions.is_empty());
    }

    #[test]
    fn degraded_does_not_overwrite_a_pending_initial_load() {
        let mut store = Store::default();
        store.degraded();
        assert_eq!(store.status, Status::Loading);

        store.apply(&envelope(
            r#"{"payload":{"type":"server.connected"},"directory":"/d"}"#,
        ));
        store.degraded();
        assert_eq!(store.status, Status::Partial);
    }
}
