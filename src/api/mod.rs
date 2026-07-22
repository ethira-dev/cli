use anyhow::{Context, Result, bail};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

use crate::auth::credentials::Credentials;
use crate::redact::redact_error_body;

/// Connect timeout for all Ethira API HTTP clients.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Default request timeout (login/whoami/workspace).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Longer timeout for inventory page fetches during sync.
pub const SYNC_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
/// Max pages per inventory resource (limit=100 → up to 50_000 items).
pub const MAX_SYNC_PAGES: usize = 500;

fn build_http_client(headers: HeaderMap, timeout: Duration) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .default_headers(headers)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout)
        .build()
        .context("failed to build HTTP client")
}

#[derive(Debug, Clone, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
}

#[derive(Debug)]
pub enum SessionError {
    Unauthorized(String),
    Other(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized(msg) | Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for SessionError {}

#[derive(Debug, Deserialize)]
struct PaginatedItems {
    items: Vec<Value>,
    #[serde(rename = "nextCursor")]
    next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PaginatedEdges {
    edges: Vec<Value>,
    #[serde(rename = "nextCursor")]
    next_cursor: Option<String>,
}

pub struct ApiClient {
    http: reqwest::Client,
    api_url: String,
    workspace_id: Option<String>,
}

impl ApiClient {
    pub fn from_credentials(creds: &Credentials) -> Result<Self> {
        let mut headers = HeaderMap::new();
        let auth = format!("Bearer {}", creds.access_token);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth)
                .context("invalid access token for Authorization header")?,
        );
        if let Some(ws) = &creds.active_workspace_id {
            headers.insert(
                "x-workspace-id",
                HeaderValue::from_str(ws).context("invalid workspace id header")?,
            );
        }

        let http = build_http_client(headers, REQUEST_TIMEOUT)?;

        Ok(Self {
            http,
            api_url: creds.api_url.trim_end_matches('/').to_string(),
            workspace_id: creds.active_workspace_id.clone(),
        })
    }

    pub fn with_token(api_url: &str, access_token: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        let auth = format!("Bearer {access_token}");
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth)
                .context("invalid access token for Authorization header")?,
        );
        let http = build_http_client(headers, REQUEST_TIMEOUT)?;
        Ok(Self {
            http,
            api_url: api_url.trim_end_matches('/').to_string(),
            workspace_id: None,
        })
    }

    /// Client with a longer timeout for inventory sync pagination.
    pub fn for_sync(creds: &Credentials) -> Result<Self> {
        let mut headers = HeaderMap::new();
        let auth = format!("Bearer {}", creds.access_token);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth)
                .context("invalid access token for Authorization header")?,
        );
        if let Some(ws) = &creds.active_workspace_id {
            headers.insert(
                "x-workspace-id",
                HeaderValue::from_str(ws).context("invalid workspace id header")?,
            );
        }
        let http = build_http_client(headers, SYNC_REQUEST_TIMEOUT)?;
        Ok(Self {
            http,
            api_url: creds.api_url.trim_end_matches('/').to_string(),
            workspace_id: creds.active_workspace_id.clone(),
        })
    }

    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        let url = format!("{}/v1/workspaces", self.api_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("failed to call GET /v1/workspaces")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = redact_error_body(&resp.text().await.unwrap_or_default());
            bail!("list workspaces failed ({status}): {body}");
        }

        resp.json()
            .await
            .context("failed to parse workspaces response")
    }

    pub async fn cli_workspace(&self) -> Result<Workspace> {
        match self.cli_workspace_checked().await {
            Ok(ws) => Ok(ws),
            Err(SessionError::Unauthorized(msg) | SessionError::Other(msg)) => bail!("{msg}"),
        }
    }

    /// Probe token validity via `GET /v1/cli/workspace`.
    pub async fn cli_workspace_checked(&self) -> Result<Workspace, SessionError> {
        let url = format!("{}/v1/cli/workspace", self.api_url);
        let mut req = self.http.get(&url);
        if let Some(ws) = &self.workspace_id {
            req = req.header("x-workspace-id", ws);
        }

        let resp = req.send().await.map_err(|e| {
            SessionError::Other(format!("failed to call GET /v1/cli/workspace: {e}"))
        })?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            let body = redact_error_body(&resp.text().await.unwrap_or_default());
            return Err(SessionError::Unauthorized(format!(
                "cli workspace failed ({status}): {body}"
            )));
        }
        if !status.is_success() {
            let body = redact_error_body(&resp.text().await.unwrap_or_default());
            return Err(SessionError::Other(format!(
                "cli workspace failed ({status}): {body}"
            )));
        }

        resp.json().await.map_err(|e| {
            SessionError::Other(format!("failed to parse cli workspace response: {e}"))
        })
    }

    /// Page through `GET /v1/cli/inventory/{resource}` until exhausted.
    pub async fn list_inventory_all(&self, resource: &str) -> Result<Vec<Value>> {
        let mut items = Vec::new();
        let mut cursor: Option<String> = None;
        let mut pages = 0usize;
        loop {
            pages += 1;
            if pages > MAX_SYNC_PAGES {
                bail!(
                    "inventory {resource} exceeded max pages ({MAX_SYNC_PAGES}); \
                     contact Ethira support if your workspace is larger than this limit"
                );
            }
            let mut url = format!("{}/v1/cli/inventory/{resource}?limit=100", self.api_url);
            if let Some(c) = &cursor {
                url.push_str("&cursor=");
                url.push_str(&urlencoding_encode(c));
            }
            let resp = self
                .http
                .get(&url)
                .send()
                .await
                .with_context(|| format!("GET /v1/cli/inventory/{resource}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = redact_error_body(&resp.text().await.unwrap_or_default());
                bail!("inventory {resource} failed ({status}): {body}");
            }
            let page: PaginatedItems = resp
                .json()
                .await
                .with_context(|| format!("parse inventory {resource}"))?;
            items.extend(page.items);
            match page.next_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }
        Ok(items)
    }

    /// Page through `GET /v1/cli/inventory/relationships` until exhausted.
    pub async fn list_inventory_relationships_all(&self) -> Result<Vec<Value>> {
        let mut edges = Vec::new();
        let mut cursor: Option<String> = None;
        let mut pages = 0usize;
        loop {
            pages += 1;
            if pages > MAX_SYNC_PAGES {
                bail!(
                    "inventory relationships exceeded max pages ({MAX_SYNC_PAGES}); \
                     contact Ethira support if your workspace is larger than this limit"
                );
            }
            let mut url = format!("{}/v1/cli/inventory/relationships?limit=100", self.api_url);
            if let Some(c) = &cursor {
                url.push_str("&cursor=");
                url.push_str(&urlencoding_encode(c));
            }
            let resp = self
                .http
                .get(&url)
                .send()
                .await
                .context("GET /v1/cli/inventory/relationships")?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = redact_error_body(&resp.text().await.unwrap_or_default());
                bail!("inventory relationships failed ({status}): {body}");
            }
            let page: PaginatedEdges =
                resp.json().await.context("parse inventory relationships")?;
            edges.extend(page.edges);
            match page.next_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }
        Ok(edges)
    }
}

/// Minimal URL encoding for cursor tokens (base64url-safe + padding).
fn urlencoding_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{byte:02X}"));
            }
        }
    }
    out
}
