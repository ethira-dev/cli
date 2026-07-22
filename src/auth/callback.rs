use anyhow::{Context, Result, bail};
use axum::Router;
use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

pub struct AuthCode {
    pub code: String,
    pub state: Option<String>,
}

impl std::fmt::Debug for AuthCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthCode")
            .field(
                "code",
                if self.code.is_empty() {
                    &"<empty>"
                } else {
                    &"<redacted>"
                },
            )
            .field("state", &self.state.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

struct CallbackState {
    sender: Arc<tokio::sync::Mutex<Option<oneshot::Sender<Result<AuthCode>>>>>,
    shutdown: Arc<tokio::sync::Mutex<Option<oneshot::Sender<()>>>>,
}

/// Ethira brand tokens (https://ethira.dev/brand) — system fonts only (no CDN).
const PAGE_STYLE: &str = r#"
    :root {
      --background: #000000;
      --mint: #A9FBC0;
      --text: #F5F5F4;
      --muted: #A3A3A3;
      --danger: #F87171;
    }
    @media (prefers-color-scheme: light) {
      :root {
        --background: #F5F5F4;
        --mint: #016853;
        --text: #1A1D23;
        --muted: #5C6370;
        --danger: #B91C1C;
      }
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      min-height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      background: var(--background);
      color: var(--text);
      font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
    }
    .card {
      text-align: center;
      padding: 3rem 2rem;
      max-width: 28rem;
    }
    .mark-wrap {
      margin: 0 auto;
      padding: 48px;
      width: fit-content;
    }
    .mark {
      width: 48px;
      height: 48px;
      display: block;
      fill: #FFFFFF;
    }
    @media (prefers-color-scheme: light) {
      .mark { fill: #1A1D23; }
    }
    .wordmark {
      margin: 0;
      font-size: 1.25rem;
      font-weight: 500;
      letter-spacing: -0.01em;
      color: var(--text);
    }
    h1 {
      margin: 1.25rem 0 0.5rem;
      font-size: 1.5rem;
      font-weight: 600;
      letter-spacing: -0.02em;
    }
    h1.ok { color: var(--mint); }
    h1.err { color: var(--danger); }
    p {
      margin: 0;
      font-size: 1rem;
      font-weight: 400;
      line-height: 1.5;
      color: var(--muted);
    }
    #countdown {
      margin-top: 1.25rem;
      font-variant-numeric: tabular-nums;
      color: var(--mint);
    }
"#;

fn success_html() -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Ethira CLI</title>
  <style>{PAGE_STYLE}</style>
</head>
<body>
  <div class="card">
    <div class="mark-wrap">
      <svg class="mark" xmlns="http://www.w3.org/2000/svg" viewBox="826.7 406.7 266.6 266.6" role="img" aria-label="Ethira">
        <path d="M960,406.7c-73.5,0-133.3,59.8-133.3,133.3s59.8,133.3,133.3,133.3,122.7-49.8,132-114.3h-21c-7.8,21.1-28,36.1-51.7,36.1s-55.1-24.7-55.1-55.1h129.2c0-.9,0-1.8,0-2.7-1.4-72.3-60.7-130.7-133.3-130.7ZM960,654.3c-63,0-114.3-51.3-114.3-114.3s51.3-114.3,114.3-114.3,37.7,4.9,53.7,13.4c-7.7-1.8-15.8-2.8-24.1-2.8-57.2,0-103.8,46.5-103.8,103.8s46.5,103.8,103.8,103.8,16.4-1,24.1-2.8c-16,8.6-34.3,13.4-53.7,13.4h0ZM945.1,540c0,40.9,33.2,74.1,74.1,74.1s9.2-.4,13.6-1.3c-12.7,7.6-27.5,11.9-43.3,11.9-46.7,0-84.8-38-84.8-84.8s38-84.7,84.8-84.7,30.6,4.4,43.3,11.9c-4.4-.8-9-1.3-13.6-1.3-40.9,0-74.1,33.2-74.1,74.1ZM967.5,521c7.8-21.1,28-36.1,51.7-36.1s44,15,51.7,36.1h-103.4Z"/>
      </svg>
    </div>
    <p class="wordmark">Ethira</p>
    <h1 class="ok">Login complete</h1>
    <p>Return to the terminal to continue.</p>
    <p id="countdown" aria-live="polite">Returning in 3…</p>
  </div>
  <script>
    (function () {{
      var seconds = 3;
      var el = document.getElementById("countdown");
      var timer = setInterval(function () {{
        seconds -= 1;
        if (seconds > 0) {{
          el.textContent = "Returning in " + seconds + "…";
          return;
        }}
        clearInterval(timer);
        el.textContent = "Returning to the terminal…";
        window.close();
      }}, 1000);
    }})();
  </script>
</body>
</html>
"#
    )
}

fn error_html() -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Ethira CLI — Login failed</title>
  <style>{PAGE_STYLE}</style>
</head>
<body>
  <div class="card">
    <p class="wordmark">Ethira</p>
    <h1 class="err">Login failed</h1>
    <p>Return to the terminal and try <code>ethira login</code> again.</p>
  </div>
</body>
</html>
"#
    )
}

/// Bind `127.0.0.1:0`, serve `/callback`, and wait for the OAuth redirect.
pub async fn wait_for_callback() -> Result<(u16, oneshot::Receiver<Result<AuthCode>>)> {
    let (tx, rx) = oneshot::channel::<Result<AuthCode>>();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let state = CallbackState {
        sender: Arc::new(tokio::sync::Mutex::new(Some(tx))),
        shutdown: Arc::new(tokio::sync::Mutex::new(Some(shutdown_tx))),
    };

    let app = Router::new()
        .route("/callback", get(callback_handler))
        .with_state(Arc::new(state));

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .context("failed to bind loopback callback server")?;
    let port = listener.local_addr()?.port();

    tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    Ok((port, rx))
}

async fn callback_handler(
    State(state): State<Arc<CallbackState>>,
    Query(query): Query<CallbackQuery>,
) -> Html<String> {
    let (result, ok) = if let Some(error) = query.error {
        let detail = query
            .error_description
            .unwrap_or_else(|| "authorization failed".into());
        (
            Err(anyhow::anyhow!("OAuth error: {error}: {detail}")),
            false,
        )
    } else if let Some(code) = query.code {
        (
            Ok(AuthCode {
                code,
                state: query.state,
            }),
            true,
        )
    } else {
        (
            Err(anyhow::anyhow!("callback missing code parameter")),
            false,
        )
    };

    if let Some(tx) = state.sender.lock().await.take() {
        let _ = tx.send(result);
    }
    if let Some(shutdown) = state.shutdown.lock().await.take() {
        let _ = shutdown.send(());
    }

    if ok {
        Html(success_html())
    } else {
        Html(error_html())
    }
}

pub async fn receive_code(
    rx: oneshot::Receiver<Result<AuthCode>>,
    expected_state: &str,
) -> Result<String> {
    let auth = match tokio::time::timeout(CALLBACK_TIMEOUT, rx).await {
        Ok(joined) => {
            joined.context("callback channel closed before authorization completed")??
        }
        Err(_) => bail!(
            "timed out waiting for OAuth callback ({}s) — try `ethira login` again",
            CALLBACK_TIMEOUT.as_secs()
        ),
    };

    match auth.state.as_deref() {
        Some(state) if state == expected_state => Ok(auth.code),
        Some(_) => bail!("OAuth state mismatch — possible CSRF; try login again"),
        None => bail!("OAuth callback missing state"),
    }
}
