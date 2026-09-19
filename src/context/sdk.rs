use std::pin::Pin;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::{Client, Method};

use crate::context::event::Envelope;

/// The same Unicode set JavaScript's `encodeURIComponent` escapes, which is what
/// the upstream SDK uses for the directory value. Alphanumerics and
/// `- _ . ! ~ * ' ( )` stay literal; non-ASCII bytes are always encoded.
const DIRECTORY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

fn directory_query(directory: &str) -> String {
    utf8_percent_encode(directory, DIRECTORY_ENCODE_SET).to_string()
}

/// Strip any userinfo (`user:token@`) from a URL before display or logging, so a
/// gateway credential in `--url` never reaches the terminal or stderr.
pub fn redact_url(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let rest = &url[scheme_end + 3..];
        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        if let Some(at) = rest[..authority_end].rfind('@') {
            return format!("{}://***@{}", &url[..scheme_end], &rest[at + 1..]);
        }
    }
    url.to_string()
}

/// A server session, trimmed to the fields M1 renders. Mirrors the upstream
/// SDK `Session` type; more fields are added as the transcript slice needs them.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Session {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
}

/// HTTP client for one opencode server. Mirrors upstream `context/sdk.tsx`:
/// the base URL, the directory scope (a `directory` query param on GET/HEAD and
/// the URL-encoded `x-opencode-directory` header on writes, as the SDK does),
/// and the `/global/event` SSE subscription.
#[derive(Clone)]
pub struct OpencodeClient {
    base_url: String,
    directory: Option<String>,
    http: Client,
}

impl OpencodeClient {
    pub fn new(base_url: impl Into<String>, directory: Option<String>) -> Result<Self> {
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("build HTTP client")?;
        Ok(Self {
            base_url: base_url.into(),
            directory,
            http,
        })
    }

    /// The base URL with any userinfo removed, safe for errors and the footer.
    pub fn display_url(&self) -> String {
        redact_url(&self.base_url)
    }

    fn request(&self, method: Method, path: &str, accept: &str) -> reqwest::RequestBuilder {
        let is_read = matches!(method, Method::GET | Method::HEAD);
        let mut url = format!("{}{}", self.base_url, path);
        let mut header_directory = None;
        if let Some(directory) = &self.directory {
            let value = directory_query(directory);
            if is_read {
                url.push(if url.contains('?') { '&' } else { '?' });
                url.push_str("directory=");
                url.push_str(&value);
            } else {
                header_directory = Some(value);
            }
        }
        let request = self.http.request(method, url).header("accept", accept);
        match header_directory {
            Some(value) => request.header("x-opencode-directory", value),
            None => request,
        }
    }

    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let response = self
            .request(Method::GET, "/session", "application/json")
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .with_context(|| format!("GET {}/session", self.display_url()))?;
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
            .request(Method::GET, "/global/event", "text/event-stream")
            .send()
            .await
            .with_context(|| format!("GET {}/global/event", self.display_url()))?;
        let status = response.status();
        if !status.is_success() {
            bail!("GET /global/event -> {status}");
        }

        let stream = response
            .bytes_stream()
            .eventsource()
            .filter_map(|item| async move {
                match item {
                    Ok(event) => match serde_json::from_str::<Envelope>(&event.data) {
                        Ok(envelope) => Some(Ok(envelope)),
                        Err(err) => {
                            tracing::debug!(%err, "dropping undecodable event frame");
                            None
                        }
                    },
                    Err(err) => Some(Err(anyhow!(err))),
                }
            });
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_encoding_matches_encode_uri_component() {
        assert_eq!(directory_query("/a b/c-d_e.f!"), "%2Fa%20b%2Fc-d_e.f!");
    }

    #[test]
    fn read_requests_carry_directory_as_query() {
        let client = OpencodeClient::new("http://host:4096", Some("/a b".into())).unwrap();
        let request = client
            .request(Method::GET, "/session", "application/json")
            .build()
            .unwrap();
        assert_eq!(request.url().query(), Some("directory=%2Fa%20b"));
        assert!(request.headers().get("x-opencode-directory").is_none());
        assert_eq!(request.headers().get("accept").unwrap(), "application/json");
    }

    #[test]
    fn write_requests_carry_directory_as_header() {
        let client = OpencodeClient::new("http://host:4096", Some("/a b".into())).unwrap();
        let request = client
            .request(Method::POST, "/session/s1/message", "application/json")
            .build()
            .unwrap();
        assert_eq!(request.url().query(), None);
        assert_eq!(
            request.headers().get("x-opencode-directory").unwrap(),
            "%2Fa%20b"
        );
    }

    #[test]
    fn absent_directory_adds_nothing() {
        let client = OpencodeClient::new("http://host:4096", None).unwrap();
        let request = client
            .request(Method::GET, "/session", "application/json")
            .build()
            .unwrap();
        assert_eq!(request.url().query(), None);
        assert!(request.headers().get("x-opencode-directory").is_none());
    }

    #[test]
    fn redacts_userinfo() {
        assert_eq!(
            redact_url("http://user:tok@host:4096/session"),
            "http://***@host:4096/session"
        );
        assert_eq!(
            redact_url("http://host:4096/session"),
            "http://host:4096/session"
        );
    }

    /// Opt-in: exercises the real server. Run with `just live` against a
    /// running `opencode serve`.
    #[tokio::test]
    #[ignore = "requires a running server; set OPENCODE_TUI_LIVE_URL"]
    async fn live_lists_sessions() {
        let url = std::env::var("OPENCODE_TUI_LIVE_URL")
            .expect("OPENCODE_TUI_LIVE_URL must be set for the live test");
        let client = OpencodeClient::new(url, None).unwrap();
        client.list_sessions().await.unwrap();
    }
}
