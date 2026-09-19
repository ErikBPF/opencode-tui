use std::time::Duration;

use serde::Deserialize;

/// Bounded exponential backoff for the event stream, mirroring the upstream
/// retry loop: 1s, 2s, 4s … capped at 30s.
pub fn backoff(attempt: u32) -> Duration {
    Duration::from_millis((1000u64 << (attempt.saturating_sub(1)).min(5)).min(30_000))
}

/// One frame from `GET /global/event`: the upstream server wraps the event in
/// `{ payload, directory, workspace }`.
#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)] // wire fields decoded for fidelity; not all read in M1.
pub struct Envelope {
    pub payload: Event,
    #[serde(default)]
    pub directory: String,
    #[serde(default)]
    pub workspace: Option<String>,
}

/// The subset of the upstream event union M1 consumes. Unknown types decode to
/// `Unknown` rather than failing the stream, matching the upstream client's
/// tolerance of server-version drift.
#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)] // some `properties` payloads are decoded but not read in M1.
#[serde(tag = "type")]
pub enum Event {
    #[serde(rename = "server.connected")]
    ServerConnected {
        #[serde(default)]
        properties: serde_json::Value,
    },
    #[serde(rename = "server.heartbeat")]
    ServerHeartbeat {
        #[serde(default)]
        properties: serde_json::Value,
    },
    #[serde(rename = "session.updated")]
    SessionUpdated { properties: serde_json::Value },
    #[serde(rename = "session.deleted")]
    SessionDeleted { properties: serde_json::Value },
    #[serde(rename = "message.updated")]
    MessageUpdated { properties: serde_json::Value },
    #[serde(rename = "message.part.updated")]
    MessagePartUpdated { properties: serde_json::Value },
    #[serde(rename = "permission.asked")]
    PermissionAsked { properties: serde_json::Value },
    #[serde(rename = "sync")]
    Sync,
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_bounded_exponential() {
        assert_eq!(backoff(1).as_millis(), 1_000);
        assert_eq!(backoff(2).as_millis(), 2_000);
        assert_eq!(backoff(3).as_millis(), 4_000);
        assert_eq!(backoff(6).as_millis(), 30_000);
        assert_eq!(backoff(30).as_millis(), 30_000);
    }

    #[test]
    fn unknown_event_types_do_not_fail_the_stream() {
        let envelope: Envelope =
            serde_json::from_str(r#"{"payload":{"type":"brand.new"},"directory":"/d"}"#).unwrap();
        assert!(matches!(envelope.payload, Event::Unknown));
    }

    #[test]
    fn sync_frames_decode() {
        let envelope: Envelope =
            serde_json::from_str(r#"{"payload":{"type":"sync"},"directory":"/d"}"#).unwrap();
        assert!(matches!(envelope.payload, Event::Sync));
    }
}
