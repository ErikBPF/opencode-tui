use std::pin::Pin;

use anyhow::{anyhow, bail, Context, Result};
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use reqwest::{Client, Method};

use crate::context::event::Envelope;

/// A server session, trimmed to the fields M1 renders. Mirrors the upstream
/// SDK `Session` type; more fields are added as the transcript slice needs them.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Session {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
}

/// HTTP client for one opencode server. Mirrors upstream `context/sdk.tsx`:
/// the base URL, the `x-opencode-directory` header (URL-encoded, as the SDK
/// does), and the `/global/event` SSE subscription.
#[derive(Clone)]
pub struct OpencodeClient {
    base_url: String,
    directory: Option<String>,
    http: Client,
}

impl OpencodeClient {
    pub fn new(base_url: impl Into<String>, directory: Option<String>) -> Result<Self> {
        let http = Client::builder().build().context("build HTTP client")?;
        Ok(Self {
            base_url: base_url.into(),
            directory,
            http,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let request = self
            .http
            .request(method, format!("{}{}", self.base_url, path))
            .header("accept", "application/json");
        match &self.directory {
            Some(directory) => request.header(
                "x-opencode-directory",
                utf8_percent_encode(directory, NON_ALPHANUMERIC).to_string(),
            ),
            None => request,
        }
    }

    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let response = self
            .request(Method::GET, "/session")
            .send()
            .await
            .with_context(|| format!("GET {}/session", self.base_url))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("GET /session -> {status}: {body}");
        }
        serde_json::from_str(&body).context("decode session list")
    }

    /// Subscribe to the server's global event stream. Mirrors the upstream
    /// `sdk.global.event` subscription; reconnect/backoff is the caller's job.
    pub async fn event_stream(
        &self,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Envelope>> + Send>>> {
        let response = self
            .request(Method::GET, "/global/event")
            .header("accept", "text/event-stream")
            .send()
            .await
            .with_context(|| format!("GET {}/global/event", self.base_url))?;
        let status = response.status();
        if !status.is_success() {
            bail!("GET /global/event -> {status}");
        }

        let stream = response
            .bytes_stream()
            .eventsource()
            .filter_map(|item| async move {
                match item {
                    Ok(event) => serde_json::from_str::<Envelope>(&event.data).ok().map(Ok),
                    Err(err) => Some(Err(anyhow!(err))),
                }
            });
        Ok(Box::pin(stream))
    }
}
