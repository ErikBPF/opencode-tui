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
