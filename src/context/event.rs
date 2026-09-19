use serde::Deserialize;

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
