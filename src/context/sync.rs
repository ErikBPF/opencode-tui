use crate::context::event::{Envelope, Event};
use crate::context::sdk::{Command, Message, MessageWithParts, PartEntry, Permission, Session};

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
    /// The open session's transcript. M1 tracks one session at a time.
    pub session: Option<String>,
    pub messages: Vec<MessageWithParts>,
    /// A permission request the server is waiting on. M1 shows it read-only.
    pub pending_permission: Option<Permission>,
    /// Server slash commands, used to route `/name` input.
    pub commands: Vec<Command>,
    /// The model the server routes to by default (`GET /config`), shown on the
    /// start screen so the user knows what will answer.
    pub model: Option<String>,
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

    /// Install the transcript returned by the initial `GET .../message` load.
    pub fn open_session(&mut self, session_id: String, messages: Vec<MessageWithParts>) {
        self.session = Some(session_id);
        self.messages = messages;
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
            Event::MessageUpdated { properties } => {
                let info = properties.get("info").cloned().unwrap_or_default();
                if let Ok(message) = serde_json::from_value::<Message>(info) {
                    if self.session.as_deref() == Some(message.session_id.as_str()) {
                        self.upsert_message(message);
                    }
                }
            }
            Event::MessagePartUpdated { properties } => {
                let part = properties.get("part").cloned().unwrap_or_default();
                if let Ok(entry) = serde_json::from_value::<PartEntry>(part) {
                    self.upsert_part(entry);
                }
            }
            Event::PermissionUpdated { properties } => {
                if let Ok(permission) = serde_json::from_value::<Permission>(properties.clone()) {
                    if self.session.as_deref() == Some(permission.session_id.as_str()) {
                        self.pending_permission = Some(permission);
                    }
                }
            }
            Event::PermissionReplied { properties } => {
                if let Some(id) = properties.get("permissionID").and_then(|id| id.as_str()) {
                    if self
                        .pending_permission
                        .as_ref()
                        .is_some_and(|permission| permission.id == id)
                    {
                        self.pending_permission = None;
                    }
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

    fn upsert_message(&mut self, message: Message) {
        match self
            .messages
            .iter_mut()
            .find(|existing| existing.info.id == message.id)
        {
            Some(existing) => existing.info = message,
            None => self.messages.push(MessageWithParts {
                info: message,
                parts: Vec::new(),
            }),
        }
    }

    fn upsert_part(&mut self, entry: PartEntry) {
        let Some(message) = self
            .messages
            .iter_mut()
            .find(|existing| existing.info.id == entry.message_id)
        else {
            return;
        };
        match message.parts.iter_mut().find(|part| part.id == entry.id) {
            Some(existing) => *existing = entry,
            None => message.parts.push(entry),
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

    #[test]
    fn transcript_reducers_upsert_messages_and_parts() {
        let mut store = Store::default();
        store.open_session("s1".to_string(), Vec::new());

        store.apply(&envelope(
            r#"{"payload":{"type":"message.updated","properties":{"info":{"id":"m1","sessionID":"s1","role":"assistant"}}}}"#,
        ));
        // A message for a different session is not part of the open transcript.
        store.apply(&envelope(
            r#"{"payload":{"type":"message.updated","properties":{"info":{"id":"m9","sessionID":"other","role":"user"}}}}"#,
        ));
        assert_eq!(store.messages.len(), 1);

        store.apply(&envelope(
            r#"{"payload":{"type":"message.part.updated","properties":{"part":{"id":"p1","messageID":"m1","sessionID":"s1","type":"text","text":"hel"}}}}"#,
        ));
        store.apply(&envelope(
            r#"{"payload":{"type":"message.part.updated","properties":{"part":{"id":"p1","messageID":"m1","sessionID":"s1","type":"text","text":"hello"}}}}"#,
        ));
        assert_eq!(store.messages[0].parts.len(), 1);
        assert_eq!(store.messages[0].parts[0].part.display(), "hello");
    }

    #[test]
    fn permission_updates_set_and_replies_clear_the_pending_request() {
        let mut store = Store::default();
        store.open_session("s1".to_string(), Vec::new());
        store.apply(&envelope(
            r#"{"payload":{"type":"permission.updated","properties":{"id":"perm1","type":"bash","sessionID":"s1","title":"Run rm","pattern":"rm -rf /tmp/x"}}}"#,
        ));
        let pending = store.pending_permission.as_ref().expect("pending");
        assert!(pending.display().contains("Run rm"));

        store.apply(&envelope(
            r#"{"payload":{"type":"permission.replied","properties":{"sessionID":"s1","permissionID":"perm1","response":"always"}}}"#,
        ));
        assert!(store.pending_permission.is_none());
    }
}
