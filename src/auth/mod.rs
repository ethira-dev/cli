pub mod callback;
pub mod credentials;
pub mod pkce;
pub mod secret_store;

use anyhow::{Context, Result, bail};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::api::{CONNECT_TIMEOUT, REQUEST_TIMEOUT};
use crate::auth::callback::{receive_code, wait_for_callback};
use crate::auth::credentials::Credentials;
use crate::auth::pkce::{generate_pkce, generate_state};
use crate::redact::{authorize_url_for_display, redact_error_body};
use crate::ui;

fn auth_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("failed to build auth HTTP client")
}

/// Scopes required for Ethira CLI inventory routes (OAuth resource = `/v1/mcp`).
/// Kept minimal to the MCP resource + refresh; not expanded further without API support
/// for a dedicated CLI scope set.
const DEFAULT_SCOPE: &str = "mcp:tools mcp:resources offline_access";
const CLIENT_NAME: &str = "Ethira CLI";

#[derive(Debug, Serialize)]
struct RegisterRequest {
    client_name: String,
    redirect_uris: Vec<String>,
    token_endpoint_auth_method: String,
    grant_types: Vec<String>,
    response_types: Vec<String>,
    scope: String,
}

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    client_id: String,
}

#[derive(Debug, Serialize)]
struct TokenRequest {
    grant_type: String,
    client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    redirect_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_verifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    scope: Option<String>,
}

pub async fn login(api_url: &str, quiet: bool) -> Result<Credentials> {
    let api_url = api_url.trim_end_matches('/').to_string();
    let http = auth_http_client()?;

    let (port, code_rx) = wait_for_callback().await?;
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    if !quiet {
        println!("{}", ui::primary("Registering OAuth client…"));
    }
    let register_url = format!("{api_url}/v1/oauth/register");
    let register_body = RegisterRequest {
        client_name: CLIENT_NAME.into(),
        redirect_uris: vec![redirect_uri.clone()],
        token_endpoint_auth_method: "none".into(),
        grant_types: vec!["authorization_code".into(), "refresh_token".into()],
        response_types: vec!["code".into()],
        scope: DEFAULT_SCOPE.into(),
    };

    let register_resp = http
        .post(&register_url)
        .json(&register_body)
        .send()
        .await
        .context("failed to call /v1/oauth/register")?;

    if !register_resp.status().is_success() {
        let status = register_resp.status();
        let body = redact_error_body(&register_resp.text().await.unwrap_or_default());
        bail!(
            "client registration failed ({status}): {body}\n\n\
             Hint: the API must allow OAuth dynamic client registration with a loopback \
             redirect URI (http://127.0.0.1). If this persists, contact your Ethira admin \
             or support, or point --api-url at an API that allows loopback (use \
             --allow-untrusted-api-host for non-default remote hosts)."
        );
    }

    let registration: RegisterResponse = register_resp
        .json()
        .await
        .context("failed to parse registration response")?;

    let pkce = generate_pkce();
    let state = generate_state();
    let resource = format!("{api_url}/v1/mcp");

    let mut authorize =
        Url::parse(&format!("{api_url}/v1/oauth/authorize")).context("invalid authorize URL")?;
    {
        let mut q = authorize.query_pairs_mut();
        q.append_pair("response_type", "code");
        q.append_pair("client_id", &registration.client_id);
        q.append_pair("redirect_uri", &redirect_uri);
        q.append_pair("code_challenge", &pkce.challenge);
        q.append_pair("code_challenge_method", "S256");
        q.append_pair("scope", DEFAULT_SCOPE);
        q.append_pair("state", &state);
        q.append_pair("resource", &resource);
    }

    if !quiet {
        println!("{}", ui::primary("Opening browser for Ethira login…"));
        println!(
            "{}\n  {}",
            ui::muted("If the browser does not open, visit:"),
            authorize_url_for_display(&authorize)
        );
    }
    if let Err(err) = open::that(authorize.as_str())
        && !quiet
    {
        eprintln!(
            "{}",
            ui::warn(&format!(
                "warning: could not open browser automatically: {err}"
            ))
        );
        eprintln!(
            "{}",
            ui::muted(
                "Full authorize URL was not printed (contains CSRF state). Re-run login or check browser."
            )
        );
    }

    if !quiet {
        println!(
            "{}",
            ui::primary("Waiting for authorization in the browser…")
        );
    }
    let code = receive_code(code_rx, &state).await?;

    // Match the callback page countdown: bring the terminal forward after 3s.
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        focus_terminal();
    });

    if !quiet {
        println!("{}", ui::primary("Exchanging authorization code…"));
    }
    let token_url = format!("{api_url}/v1/oauth/token");
    let token_body = TokenRequest {
        grant_type: "authorization_code".into(),
        client_id: registration.client_id.clone(),
        code: Some(code),
        redirect_uri: Some(redirect_uri),
        code_verifier: Some(pkce.verifier),
        refresh_token: None,
        resource: Some(resource),
    };

    let token_resp = http
        .post(&token_url)
        .json(&token_body)
        .send()
        .await
        .context("failed to call /v1/oauth/token")?;

    if !token_resp.status().is_success() {
        let status = token_resp.status();
        let body = redact_error_body(&token_resp.text().await.unwrap_or_default());
        bail!("token exchange failed ({status}): {body}");
    }

    let tokens: TokenResponse = token_resp
        .json()
        .await
        .context("failed to parse token response")?;

    let expires_at = tokens
        .expires_in
        .map(|secs| (Utc::now() + Duration::seconds(secs)).timestamp());

    let mut creds = Credentials {
        api_url,
        client_id: registration.client_id,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token.unwrap_or_default(),
        expires_at,
        scope: tokens.scope.or_else(|| Some(DEFAULT_SCOPE.into())),
        active_workspace_id: None,
        active_workspace_name: None,
    };
    creds.clear_workspace();
    credentials::save(&creds)?;

    if !quiet {
        println!(
            "{} {}",
            ui::success("Logged in successfully."),
            ui::muted("Tokens saved to the OS keychain (metadata in ~/.ethira/credentials.json)")
        );
    }
    Ok(creds)
}

/// Exchange a refresh token for a new access token and persist credentials.
pub async fn refresh_tokens(creds: &mut Credentials) -> Result<()> {
    if creds.refresh_token.is_empty() {
        bail!("no refresh token available — run `ethira login`");
    }
    let http = auth_http_client()?;
    let api_url = creds.api_url.trim_end_matches('/');
    let token_url = format!("{api_url}/v1/oauth/token");
    let resource = format!("{api_url}/v1/mcp");
    let body = TokenRequest {
        grant_type: "refresh_token".into(),
        client_id: creds.client_id.clone(),
        code: None,
        redirect_uri: None,
        code_verifier: None,
        refresh_token: Some(creds.refresh_token.clone()),
        resource: Some(resource),
    };

    let resp = http
        .post(&token_url)
        .json(&body)
        .send()
        .await
        .context("failed to call /v1/oauth/token (refresh)")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = redact_error_body(&resp.text().await.unwrap_or_default());
        bail!("token refresh failed ({status}): {body}");
    }

    let tokens: TokenResponse = resp
        .json()
        .await
        .context("failed to parse refresh token response")?;

    creds.access_token = tokens.access_token;
    if let Some(refresh) = tokens.refresh_token
        && !refresh.is_empty()
    {
        creds.refresh_token = refresh;
    }
    creds.expires_at = tokens
        .expires_in
        .map(|secs| (Utc::now() + Duration::seconds(secs)).timestamp());
    if let Some(scope) = tokens.scope {
        creds.scope = Some(scope);
    }
    credentials::save(creds)?;
    Ok(())
}

/// Best-effort RFC 7009-style revocation. Always succeeds from the caller's perspective.
pub async fn revoke_tokens(creds: &Credentials) -> Result<()> {
    let http = auth_http_client()?;
    let api_url = creds.api_url.trim_end_matches('/');
    let revoke_url = format!("{api_url}/v1/oauth/revoke");

    let token = if !creds.refresh_token.is_empty() {
        (&creds.refresh_token, "refresh_token")
    } else if !creds.access_token.is_empty() {
        (&creds.access_token, "access_token")
    } else {
        return Ok(());
    };

    let _ = http
        .post(&revoke_url)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!(
            "token={}&token_type_hint={}&client_id={}",
            urlencoding_form(token.0),
            token.1,
            urlencoding_form(&creds.client_id)
        ))
        .send()
        .await;
    Ok(())
}

fn urlencoding_form(value: &str) -> String {
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

/// Best-effort: return focus to the terminal that launched the CLI.
/// Browsers cannot do this; we activate the owning GUI process from here.
fn focus_terminal() {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        if let Ok(term) = std::env::var("TERM_PROGRAM") {
            let app = match term.as_str() {
                "iTerm.app" => Some("iTerm"),
                "Apple_Terminal" => Some("Terminal"),
                "WarpTerminal" | "Warp" => Some("Warp"),
                "ghostty" => Some("Ghostty"),
                // vscode / Cursor integrated terminals: activate via parent process walk.
                _ => None,
            };
            if let Some(name) = app {
                let _ = Command::new("osascript")
                    .args(["-e", &format!("tell application \"{name}\" to activate")])
                    .output();
                return;
            }
        }

        // Walk parents and front the first process System Events recognizes.
        let mut pid = std::os::unix::process::parent_id();
        for _ in 0..10 {
            if pid <= 1 {
                break;
            }
            let script = format!(
                "tell application \"System Events\" to set frontmost of \
                 (first process whose unix id is {pid}) to true"
            );
            let _ = Command::new("osascript").args(["-e", &script]).output();
            let Ok(output) = Command::new("ps")
                .args(["-o", "ppid=", "-p", &pid.to_string()])
                .output()
            else {
                break;
            };
            let Ok(ppid) = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u32>()
            else {
                break;
            };
            pid = ppid;
        }
    }
}

pub async fn logout(quiet: bool) -> Result<()> {
    if let Ok(Some(creds)) = credentials::load() {
        let _ = revoke_tokens(&creds).await;
    }
    credentials::delete()?;
    let _ = crate::db::InventoryDb::clear_cache();
    if !quiet {
        println!("{}", ui::success("Logged out."));
    }
    Ok(())
}
