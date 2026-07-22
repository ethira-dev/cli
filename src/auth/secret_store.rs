use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE: &str = "ethira-cli";

fn access_entry(client_id: &str) -> Result<Entry> {
    Entry::new(SERVICE, &format!("{client_id}:access"))
        .context("failed to open keychain entry for access token")
}

fn refresh_entry(client_id: &str) -> Result<Entry> {
    Entry::new(SERVICE, &format!("{client_id}:refresh"))
        .context("failed to open keychain entry for refresh token")
}

pub fn store_tokens(client_id: &str, access_token: &str, refresh_token: &str) -> Result<()> {
    if client_id.is_empty() {
        anyhow::bail!("cannot store tokens without client_id");
    }
    access_entry(client_id)?
        .set_password(access_token)
        .context("failed to store access token in OS keychain")?;
    if refresh_token.is_empty() {
        let _ = refresh_entry(client_id)?.delete_credential();
    } else {
        refresh_entry(client_id)?
            .set_password(refresh_token)
            .context("failed to store refresh token in OS keychain")?;
    }
    Ok(())
}

pub fn load_tokens(client_id: &str) -> Result<(Option<String>, Option<String>)> {
    if client_id.is_empty() {
        return Ok((None, None));
    }
    let access = match access_entry(client_id)?.get_password() {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    };
    let refresh = match refresh_entry(client_id)?.get_password() {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    };
    Ok((access, refresh))
}

pub fn delete_tokens(client_id: &str) -> Result<()> {
    if client_id.is_empty() {
        return Ok(());
    }
    let _ = access_entry(client_id)?.delete_credential();
    let _ = refresh_entry(client_id)?.delete_credential();
    Ok(())
}
