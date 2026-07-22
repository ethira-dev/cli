use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::auth::secret_store;
use crate::fs_util::{self, plaintext_tokens_allowed, write_private_atomic};
use crate::ui;

/// In-memory credentials. Tokens are loaded from the OS keychain when available.
#[derive(Clone, Default)]
pub struct Credentials {
    pub api_url: String,
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: Option<i64>,
    pub scope: Option<String>,
    pub active_workspace_id: Option<String>,
    pub active_workspace_name: Option<String>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("api_url", &self.api_url)
            .field("client_id", &self.client_id)
            .field("access_token", &redact_secret(&self.access_token))
            .field("refresh_token", &redact_secret(&self.refresh_token))
            .field("expires_at", &self.expires_at)
            .field("scope", &self.scope)
            .field("active_workspace_id", &self.active_workspace_id)
            .field("active_workspace_name", &self.active_workspace_name)
            .finish()
    }
}

fn redact_secret(value: &str) -> &'static str {
    if value.is_empty() {
        "<empty>"
    } else {
        "<redacted>"
    }
}

/// On-disk shape: tokens are omitted once migrated to the OS keychain.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredCredentials {
    pub api_url: String,
    pub client_id: String,
    /// Legacy plaintext field — migrated to keychain on load.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub access_token: String,
    /// Legacy plaintext field — migrated to keychain on load.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub refresh_token: String,
    pub expires_at: Option<i64>,
    pub scope: Option<String>,
    pub active_workspace_id: Option<String>,
    pub active_workspace_name: Option<String>,
}

impl Credentials {
    pub fn has_tokens(&self) -> bool {
        !self.access_token.is_empty() && !self.client_id.is_empty()
    }

    /// True when `expires_at` is set and already in the past.
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(ts) => Utc::now().timestamp() >= ts,
            None => false,
        }
    }

    pub fn clear_workspace(&mut self) {
        self.active_workspace_id = None;
        self.active_workspace_name = None;
    }
}

pub fn credentials_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not resolve home directory")?;
    Ok(home.join(".ethira").join("credentials.json"))
}

pub fn load() -> Result<Option<Credentials>> {
    let path = credentials_path()?;
    if !path.exists() {
        return Ok(None);
    }
    fs_util::refuse_symlink(&path)?;
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let stored: StoredCredentials = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    let mut access = stored.access_token.clone();
    let mut refresh = stored.refresh_token.clone();
    let mut migrated = false;

    if let Ok((kc_access, kc_refresh)) = secret_store::load_tokens(&stored.client_id) {
        if let Some(token) = kc_access {
            access = token;
        }
        if let Some(token) = kc_refresh {
            refresh = token;
        }
    }

    // Migrate legacy plaintext tokens into the keychain and rewrite the file.
    if (!stored.access_token.is_empty() || !stored.refresh_token.is_empty())
        && !stored.client_id.is_empty()
        && secret_store::store_tokens(&stored.client_id, &access, &refresh).is_ok()
    {
        migrated = true;
    }

    let creds = Credentials {
        api_url: stored.api_url,
        client_id: stored.client_id,
        access_token: access,
        refresh_token: refresh,
        expires_at: stored.expires_at,
        scope: stored.scope,
        active_workspace_id: stored.active_workspace_id,
        active_workspace_name: stored.active_workspace_name,
    };

    if migrated {
        let _ = save(&creds);
    }

    Ok(Some(creds))
}

pub fn save(creds: &Credentials) -> Result<()> {
    let path = credentials_path()?;
    if let Some(parent) = path.parent() {
        fs_util::ensure_private_dir(parent)?;
    }

    // Delete orphaned keychain entries when DCR issued a new client_id.
    if let Ok(Some(prev)) = load_metadata_only()
        && !prev.client_id.is_empty()
        && prev.client_id != creds.client_id
    {
        let _ = secret_store::delete_tokens(&prev.client_id);
    }

    let mut stored = StoredCredentials {
        api_url: creds.api_url.clone(),
        client_id: creds.client_id.clone(),
        access_token: String::new(),
        refresh_token: String::new(),
        expires_at: creds.expires_at,
        scope: creds.scope.clone(),
        active_workspace_id: creds.active_workspace_id.clone(),
        active_workspace_name: creds.active_workspace_name.clone(),
    };

    match secret_store::store_tokens(&creds.client_id, &creds.access_token, &creds.refresh_token) {
        Ok(()) => {}
        Err(err) => {
            if plaintext_tokens_allowed() {
                eprintln!(
                    "{}",
                    ui::warn(
                        "warning: OS keychain unavailable — storing tokens in plaintext \
                         ~/.ethira/credentials.json because ETHIRA_ALLOW_PLAINTEXT_TOKENS is set. \
                         Unset that variable and fix the keychain/secret-service for safer storage."
                    )
                );
                stored.access_token = creds.access_token.clone();
                stored.refresh_token = creds.refresh_token.clone();
            } else {
                bail!(
                    "failed to store tokens in the OS keychain: {err:#}\n\n\
                     Tokens are not written to disk by default. Fix your OS keychain / secret \
                     service, or set ETHIRA_ALLOW_PLAINTEXT_TOKENS=1 to opt in to plaintext \
                     storage in ~/.ethira/credentials.json (not recommended)."
                );
            }
        }
    }

    let json = serde_json::to_string_pretty(&stored)?;
    write_private_atomic(&path, json.as_bytes())?;
    Ok(())
}

pub fn delete() -> Result<()> {
    if let Ok(Some(creds)) = load_metadata_only() {
        let _ = secret_store::delete_tokens(&creds.client_id);
    }
    let path = credentials_path()?;
    if path.exists() {
        fs_util::refuse_symlink(&path)?;
        fs::remove_file(&path).with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

/// Load disk metadata without requiring keychain (used during delete).
fn load_metadata_only() -> Result<Option<StoredCredentials>> {
    let path = credentials_path()?;
    if !path.exists() {
        return Ok(None);
    }
    fs_util::refuse_symlink(&path)?;
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let stored: StoredCredentials = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(stored))
}
