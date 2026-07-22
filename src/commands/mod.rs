use anyhow::Result;
use dialoguer::Confirm;
use serde_json::{Map, Value, json};

use crate::api::{ApiClient, SessionError, Workspace};
use crate::auth;
use crate::auth::credentials::{self, Credentials};
use crate::db::{
    InventoryDb, JSON_SCHEMA_VERSION, MCP_SERVER_ROUTING_FIELDS, REPOSITORY_ROUTING_FIELDS,
};
use crate::exit_codes::CacheMissingError;
use crate::inventory;
use crate::onboarding::FirstSyncOutcome;
use crate::shell::workspace_select;
use crate::ui::{self, AuthFailure, UnauthorizedError};

/// Log in via OAuth, select a workspace, then offer a first sync when the cache is empty.
pub async fn login(api_url: &str, quiet: bool) -> Result<()> {
    let mut creds = auth::login(api_url, quiet).await?;
    workspace_select::select_and_persist(&mut creds).await?;
    let outcome = offer_first_sync_if_needed(api_url, quiet).await?;
    if outcome == FirstSyncOutcome::Declined && !quiet {
        println!(
            "{}",
            ui::muted(
                "Skipped sync. Run `ethira sync` when ready, then `ethira tables` or `ethira sql`."
            )
        );
    }
    Ok(())
}

/// After interactive login, offer to sync when the local cache is empty.
async fn offer_first_sync_if_needed(api_url: &str, quiet: bool) -> Result<FirstSyncOutcome> {
    if !InventoryDb::cache_needs_sync() {
        return Ok(FirstSyncOutcome::NotNeeded);
    }

    let sync_now = if quiet {
        false
    } else {
        Confirm::with_theme(&ui::dialoguer_theme())
            .with_prompt("Sync inventory now? (recommended)")
            .default(true)
            .interact()?
    };

    if sync_now {
        sync(api_url, false, false, quiet).await?;
        Ok(FirstSyncOutcome::Synced)
    } else {
        Ok(FirstSyncOutcome::Declined)
    }
}

pub async fn workspace(api_url: &str, quiet: bool) -> Result<()> {
    let (mut creds, _) = require_session(api_url).await?;
    workspace_select::select_and_persist(&mut creds).await?;
    if !quiet {
        println!(
            "{}",
            ui::muted(
                "note: API inventory sync uses the OAuth token workspace. Re-run `ethira login` to \
                 consent for another workspace."
            )
        );
    }
    Ok(())
}

pub async fn whoami(api_url: &str, as_json: bool, quiet: bool) -> Result<()> {
    let (creds, ws) = require_session(api_url).await?;
    let mismatch = creds
        .active_workspace_id
        .as_ref()
        .is_some_and(|id| id != &ws.id);
    let cache = local_cache_status();

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schemaVersion": JSON_SCHEMA_VERSION,
                "tokenWorkspace": { "id": ws.id, "name": ws.name },
                "shellWorkspaceId": creds.active_workspace_id,
                "shellWorkspaceName": creds.active_workspace_name,
                "mismatch": mismatch,
                "scope": creds.scope,
                "apiUrl": creds.api_url,
                "cache": cache,
            }))?
        );
    } else {
        println!(
            "{} {} ({})",
            ui::primary_bold("Workspace:"),
            ws.name,
            ui::muted(&ws.id)
        );
        if mismatch && !quiet {
            println!(
                "{}",
                ui::warn(&format!(
                    "warning: shell selection is {}; token-bound CLI workspace is {}",
                    creds.active_workspace_id.as_deref().unwrap_or("?"),
                    ws.id
                ))
            );
        }
        if let Some(scope) = &creds.scope {
            println!("{} {scope}", ui::muted("Scope:"));
        }
        if let Some(synced) = cache.get("lastSyncedAt").and_then(|v| v.as_str()) {
            println!("{} {synced}", ui::muted("Last sync:"));
        } else if !quiet {
            println!("{}", ui::muted("Last sync: (none — run ethira sync)"));
        }
    }
    Ok(())
}

/// Local-only status for agents (no Ethira API).
pub fn status(as_json: bool) -> Result<()> {
    let logged_in = credentials::load()?
        .map(|c| c.has_tokens())
        .unwrap_or(false);
    let cache = local_cache_status();
    let cache_present = cache
        .get("present")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schemaVersion": JSON_SCHEMA_VERSION,
                "localOnly": true,
                "loggedIn": logged_in,
                "cache": cache,
                "agentHints": {
                    "runtime": "Invoke ethira via shell; parse --json stdout. Do not use Ethira MCP.",
                    "query": "ethira sql --json and ethira tables --json never hit the network",
                    "refresh": "ethira sync downloads inventory; then query the local cache",
                    "orient": ["ethira status --json", "ethira schema --json", "ethira routing --json"],
                    "exitCodes": {
                        "0": "success",
                        "1": "generic error",
                        "2": "unauthorized / not logged in",
                        "3": "cache missing",
                        "4": "SQL error",
                    },
                },
            }))?
        );
    } else {
        println!(
            "{} {}",
            ui::primary_bold("Logged in:"),
            if logged_in { "yes" } else { "no" }
        );
        println!(
            "{} {}",
            ui::primary_bold("Cache:"),
            if cache_present {
                cache
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("~/.ethira/inventory.sqlite")
            } else {
                "missing — run ethira sync"
            }
        );
        if let Some(synced) = cache.get("lastSyncedAt").and_then(|v| v.as_str()) {
            println!("{} {synced}", ui::primary_bold("Last sync:"));
        }
        if let Some(ws) = cache.get("workspaceName").and_then(|v| v.as_str()) {
            println!("{} {ws}", ui::primary_bold("Synced workspace:"));
        }
    }
    Ok(())
}

/// Local-only schema documentation for agents.
pub fn schema(as_json: bool) -> Result<()> {
    let doc = match InventoryDb::open_cache_readonly() {
        Ok(db) => db.schema_document(),
        Err(err) if err.downcast_ref::<CacheMissingError>().is_some() => {
            InventoryDb::static_schema_document()
        }
        Err(err) => return Err(err),
    };

    if as_json {
        println!("{}", serde_json::to_string_pretty(&doc)?);
    } else {
        println!("{}", ui::primary_bold("Local inventory schema"));
        println!(
            "{}",
            ui::muted(
                "Query path is local-only after sync. Use --json for machine-readable output."
            )
        );
        if let Some(tables) = doc.get("tables").and_then(|t| t.as_array()) {
            for table in tables {
                let name = table.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let count = table.get("rowCount").and_then(|v| v.as_i64()).unwrap_or(0);
                println!("{} {}", ui::primary(name), ui::muted(&count.to_string()));
            }
        }
        println!();
        println!("{}", ui::primary("Example routing query:"));
        println!(
            "  ethira sql --json 'SELECT name, system_id, json_extract(raw_json, \"$.cloneUrl\") AS clone_url FROM repositories LIMIT 20'"
        );
        println!(
            "{}",
            ui::muted("See also: ethira routing --json · ethira status --json")
        );
    }
    Ok(())
}

/// Best-effort cross-repo / MCP routing directory from the local cache (no network).
pub fn routing(as_json: bool) -> Result<()> {
    let db = InventoryDb::open_cache_readonly()?;

    let repos = db.query(
        "SELECT
            id,
            business_id,
            name,
            system_id,
            team_id,
            owner_user_id,
            coalesce(
                json_extract(raw_json, '$.cloneUrl'),
                json_extract(raw_json, '$.gitUrl'),
                json_extract(raw_json, '$.url'),
                json_extract(raw_json, '$.htmlUrl')
            ) AS url,
            json_extract(raw_json, '$.defaultBranch') AS default_branch,
            json_extract(raw_json, '$.provider') AS provider,
            json_extract(raw_json, '$.fullName') AS full_name,
            json_extract(raw_json, '$.externalId') AS external_id
         FROM repositories
         ORDER BY name",
    )?;

    let mcps = db.query(
        "SELECT
            id,
            business_id,
            name,
            team_id,
            owner_user_id,
            coalesce(
                json_extract(raw_json, '$.url'),
                json_extract(raw_json, '$.endpoint')
            ) AS url,
            json_extract(raw_json, '$.transport') AS transport,
            json_extract(raw_json, '$.command') AS command
         FROM mcp_servers
         ORDER BY name",
    )?;

    let repositories = query_result_to_objects(&repos);
    let mcp_servers = query_result_to_objects(&mcps);

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schemaVersion": JSON_SCHEMA_VERSION,
                "localOnly": true,
                "note": "Use these facts to choose the next tool (git, gh, another product MCP, filesystem). Ethira itself is CLI-only.",
                "repositoryRoutingFields": REPOSITORY_ROUTING_FIELDS.iter().map(|f| {
                    json!({ "path": f.path, "description": f.description })
                }).collect::<Vec<_>>(),
                "mcpServerRoutingFields": MCP_SERVER_ROUTING_FIELDS.iter().map(|f| {
                    json!({ "path": f.path, "description": f.description })
                }).collect::<Vec<_>>(),
                "repositories": repositories,
                "mcpServers": mcp_servers,
            }))?
        );
    } else {
        println!("{}", ui::primary_bold("Repositories (routing)"));
        for row in &repositories {
            let name = row.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let url = row.get("url").and_then(|v| v.as_str()).unwrap_or("-");
            println!("  {} {}", ui::primary(name), ui::muted(url));
        }
        println!("{}", ui::primary_bold("MCP servers (routing)"));
        for row in &mcp_servers {
            let name = row.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let url = row.get("url").and_then(|v| v.as_str()).unwrap_or("-");
            println!("  {} {}", ui::primary(name), ui::muted(url));
        }
    }
    Ok(())
}

pub async fn sync(api_url: &str, as_json: bool, clear: bool, quiet: bool) -> Result<()> {
    if clear {
        InventoryDb::clear_cache()?;
        if as_json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "schemaVersion": JSON_SCHEMA_VERSION,
                    "cleared": true,
                    "cache": InventoryDb::cache_path()?.to_string_lossy(),
                }))?
            );
        } else if !quiet {
            println!("{}", ui::success("Cleared inventory cache."));
        }
        return Ok(());
    }

    let (creds, token_ws) = require_session(api_url).await?;
    let client = ApiClient::for_sync(&creds)?;
    if let Some(selected) = &creds.active_workspace_id
        && selected != &token_ws.id
        && !quiet
    {
        eprintln!(
            "{}",
            ui::warn(&format!(
                "warning: shell workspace ({selected}) differs from token workspace ({} / {}). \
                 Sync will use the token workspace.",
                token_ws.name, token_ws.id
            ))
        );
    }

    if !quiet {
        eprintln!(
            "{} {} ({})…",
            ui::primary("Syncing inventory for token workspace"),
            token_ws.name,
            ui::muted(&token_ws.id)
        );
    }

    let db = InventoryDb::open_cache()?;
    let summary = inventory::sync_all(&client, &db).await?;
    let synced_at = InventoryDb::now_rfc3339();
    db.set_sync_meta(&token_ws.id, &token_ws.name, &synced_at)?;

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schemaVersion": JSON_SCHEMA_VERSION,
                "workspace": { "id": token_ws.id, "name": token_ws.name },
                "lastSyncedAt": synced_at,
                "counts": summary.counts,
                "relationships": summary.relationships,
                "warnings": summary.warnings,
                "cache": InventoryDb::cache_path()?.to_string_lossy(),
                "localOnlyAfterSync": true,
            }))?
        );
    } else {
        inventory::print_summary(&summary, quiet);
    }
    Ok(())
}

pub fn tables(as_json: bool) -> Result<()> {
    let db = InventoryDb::open_cache_readonly()?;
    let mut rows = Vec::new();
    for name in db.tables() {
        let count = db.count(name).unwrap_or(0);
        rows.push((name.to_string(), count));
    }
    if as_json {
        let map: Map<String, Value> = rows.into_iter().map(|(k, v)| (k, json!(v))).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schemaVersion": JSON_SCHEMA_VERSION,
                "localOnly": true,
                "tables": map,
            }))?
        );
    } else {
        for (name, count) in rows {
            println!("{} {}", ui::primary(&name), ui::muted(&count.to_string()));
        }
    }
    Ok(())
}

pub async fn sql(
    api_url: &str,
    query: &str,
    do_sync: bool,
    as_json: bool,
    quiet: bool,
) -> Result<()> {
    if do_sync {
        sync(api_url, false, false, quiet).await?;
    }
    let db = match InventoryDb::open_cache_readonly() {
        Ok(db) => db,
        Err(err) => {
            if let Some(cache_err) = err.downcast_ref::<CacheMissingError>() {
                return Err(CacheMissingError::new(format!(
                    "{}\n\nSee `ethira examples` for getting started.",
                    cache_err.message
                ))
                .into());
            }
            return Err(err);
        }
    };
    let result = db.query(query)?;
    if as_json {
        let rows: Vec<Map<String, Value>> = result
            .rows
            .iter()
            .map(|row| {
                result
                    .columns
                    .iter()
                    .zip(row.iter())
                    .map(|(c, v)| (c.clone(), v.clone()))
                    .collect()
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schemaVersion": JSON_SCHEMA_VERSION,
                "localOnly": !do_sync,
                "columns": result.columns,
                "rows": rows,
            }))?
        );
    } else {
        result.print_table();
    }
    Ok(())
}

pub async fn logout(quiet: bool) -> Result<()> {
    auth::logout(quiet).await?;
    if !quiet {
        println!("{}", ui::success("Cleared inventory cache."));
    }
    Ok(())
}

pub fn print_examples() {
    println!(
        "\
{}

{}
  ethira login
  # After login, sync when prompted (or run ethira sync)

{}
  ethira sync
  ethira status
  ethira schema
  ethira tables
  ethira routing
  ethira sql 'SELECT name FROM systems LIMIT 10'
  ethira sql 'SELECT name, json_extract(raw_json, \"$.criticality\") AS criticality FROM incidents LIMIT 10'
  ethira sql 'SELECT name, system_id, json_extract(raw_json, \"$.cloneUrl\") AS clone_url FROM repositories LIMIT 20'

{}
  ethira status --json
  ethira schema --json
  ethira routing --json
  ethira sync --json
  ethira sql --json 'SELECT kind, count(*) AS n FROM relationships GROUP BY kind'
  ethira whoami --json
  ethira sync -q && ethira tables --json

{}
  ethira logout

{}
  ethira completions zsh > ~/.zfunc/_ethira

{}
  0 success · 1 generic · 2 unauthorized · 3 cache missing · 4 SQL error

{}
",
        ui::primary_bold("Ethira — common examples"),
        ui::primary("First-time:"),
        ui::primary("Explore locally (no API after sync):"),
        ui::primary("Agent / scripting:"),
        ui::primary("Sign out:"),
        ui::primary("Shell completions:"),
        ui::primary("Exit codes:"),
        ui::muted("More: ethira --help · ethira <command> --help · ethira examples")
    );
}

fn query_result_to_objects(result: &crate::db::QueryResult) -> Vec<Map<String, Value>> {
    result
        .rows
        .iter()
        .map(|row| {
            result
                .columns
                .iter()
                .zip(row.iter())
                .map(|(c, v)| (c.clone(), v.clone()))
                .collect()
        })
        .collect()
}

fn local_cache_status() -> Value {
    let path = InventoryDb::cache_path().ok();
    let present = path.as_ref().is_some_and(|p| p.exists());
    if !present {
        return json!({
            "present": false,
            "path": path.map(|p| p.to_string_lossy().to_string()),
            "lastSyncedAt": Value::Null,
            "workspaceId": Value::Null,
            "workspaceName": Value::Null,
            "mtime": Value::Null,
        });
    }
    let path = path.expect("present implies path");
    let mtime = fs_mtime_rfc3339(&path);
    let (last_synced_at, workspace_id, workspace_name) =
        match InventoryDb::open_file_readonly(&path) {
            Ok(db) => (
                db.get_meta("last_synced_at").ok().flatten(),
                db.get_meta("workspace_id").ok().flatten(),
                db.get_meta("workspace_name").ok().flatten(),
            ),
            Err(_) => (None, None, None),
        };
    json!({
        "present": true,
        "path": path.to_string_lossy(),
        "lastSyncedAt": last_synced_at,
        "workspaceId": workspace_id,
        "workspaceName": workspace_name,
        "mtime": mtime,
    })
}

fn fs_mtime_rfc3339(path: &std::path::Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// Load credentials and verify the session with the API.
async fn require_session(api_url: &str) -> Result<(Credentials, Workspace)> {
    let mut creds = credentials::load()?.unwrap_or_default();
    if creds.api_url.is_empty() {
        creds.api_url = api_url.to_string();
    } else {
        crate::api_url::ensure_api_url_consistent(api_url, &creds.api_url)?;
    }
    if !creds.has_tokens() {
        return Err(UnauthorizedError::new(AuthFailure::NotLoggedIn).into());
    }

    if creds.is_expired() && auth::refresh_tokens(&mut creds).await.is_err() {
        return Err(UnauthorizedError::new(AuthFailure::Expired).into());
    }

    let client = ApiClient::from_credentials(&creds)?;
    match client.cli_workspace_checked().await {
        Ok(ws) => Ok((creds, ws)),
        Err(SessionError::Unauthorized(_)) => {
            if auth::refresh_tokens(&mut creds).await.is_ok() {
                let client = ApiClient::from_credentials(&creds)?;
                match client.cli_workspace_checked().await {
                    Ok(ws) => return Ok((creds, ws)),
                    Err(SessionError::Unauthorized(_)) => {
                        return Err(UnauthorizedError::new(AuthFailure::Rejected).into());
                    }
                    Err(SessionError::Other(msg)) => anyhow::bail!("{msg}"),
                }
            }
            Err(UnauthorizedError::new(AuthFailure::Rejected).into())
        }
        Err(SessionError::Other(msg)) => anyhow::bail!("{msg}"),
    }
}
