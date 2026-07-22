use anyhow::{Context, Result, bail};
use dialoguer::Select;

use crate::api::{ApiClient, Workspace};
use crate::auth::credentials::{self, Credentials};
use crate::ui;

pub async fn select_and_persist(creds: &mut Credentials) -> Result<()> {
    let workspaces = fetch_workspace_options(creds).await?;
    if workspaces.is_empty() {
        bail!("no workspaces available for this account");
    }

    let labels: Vec<String> = workspaces
        .iter()
        .map(|w| format!("{} ({})", w.name, w.id))
        .collect();

    let selection = Select::with_theme(&ui::dialoguer_theme())
        .with_prompt("Select a workspace")
        .items(&labels)
        .default(0)
        .interact()
        .context("workspace selection cancelled")?;

    let chosen = &workspaces[selection];
    creds.active_workspace_id = Some(chosen.id.clone());
    creds.active_workspace_name = Some(chosen.name.clone());
    credentials::save(creds)?;

    println!(
        "{} {} ({})",
        ui::success("Workspace set to"),
        chosen.name,
        ui::muted(&chosen.id)
    );
    Ok(())
}

async fn fetch_workspace_options(creds: &Credentials) -> Result<Vec<Workspace>> {
    let client = ApiClient::with_token(&creds.api_url, &creds.access_token)?;

    match client.list_workspaces().await {
        Ok(list) if !list.is_empty() => Ok(list),
        Ok(_) => {
            eprintln!(
                "{}",
                ui::warn(
                    "warning: GET /v1/workspaces returned empty; falling back to /v1/cli/workspace"
                )
            );
            Ok(vec![client.cli_workspace().await?])
        }
        Err(err) => {
            eprintln!(
                "{}",
                ui::warn(&format!(
                    "warning: GET /v1/workspaces failed ({err}); falling back to /v1/cli/workspace"
                ))
            );
            Ok(vec![client.cli_workspace().await?])
        }
    }
}
