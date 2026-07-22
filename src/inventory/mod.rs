use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::api::ApiClient;
use crate::db::{ASSET_TABLES, InventoryDb};
use crate::ui;

/// API path segment → local SQLite table name.
pub const SYNC_RESOURCES: &[(&str, &str)] = &[
    ("systems", "systems"),
    ("microservices", "microservices"),
    ("datasets", "datasets"),
    ("products", "products"),
    ("devices", "devices"),
    ("agents", "agents"),
    ("processes", "processes"),
    ("repositories", "repositories"),
    ("datapoints", "datapoints"),
    ("policies", "policies"),
    ("physical-locations", "physical_locations"),
    ("ai-skills", "ai_skills"),
    ("ai-models", "ai_models"),
    ("third-party-systems", "third_party_systems"),
    ("mcp-servers", "mcp_servers"),
    ("teams", "teams"),
    ("vendors", "vendors"),
    ("incidents", "incidents"),
    ("data-breaches", "data_breaches"),
];

#[derive(Debug, Default, Clone)]
pub struct SyncSummary {
    pub counts: BTreeMap<String, i64>,
    pub relationships: i64,
    pub warnings: Vec<String>,
}

pub async fn sync_all(client: &ApiClient, db: &InventoryDb) -> Result<SyncSummary> {
    db.rebuild_schema()
        .context("failed to rebuild inventory schema")?;

    let mut summary = SyncSummary::default();

    for (api_path, table) in SYNC_RESOURCES {
        match client.list_inventory_all(api_path).await {
            Ok(items) => {
                for item in &items {
                    db.insert_asset(table, item)
                        .with_context(|| format!("insert {table}"))?;
                }
                summary
                    .counts
                    .insert((*table).to_string(), items.len() as i64);
            }
            Err(err) => {
                let warning = format!("{api_path}: skipped ({err})");
                summary.warnings.push(warning);
                summary.counts.insert((*table).to_string(), 0);
            }
        }
    }

    let edges = client
        .list_inventory_relationships_all()
        .await
        .context("failed to fetch inventory relationships")?;
    for edge in &edges {
        db.insert_relationship(edge)?;
    }
    summary.relationships = edges.len() as i64;

    for table in ASSET_TABLES {
        summary.counts.entry((*table).to_string()).or_insert(0);
    }

    Ok(summary)
}

pub fn print_summary(summary: &SyncSummary, quiet: bool) {
    if quiet {
        return;
    }
    println!(
        "{}",
        ui::success("Inventory sync complete (cache: ~/.ethira/inventory.sqlite).")
    );
    for (table, count) in &summary.counts {
        println!("  {} {}", ui::primary(table), ui::muted(&count.to_string()));
    }
    println!(
        "  {} {}",
        ui::primary("relationships"),
        ui::muted(&summary.relationships.to_string())
    );
    for warning in &summary.warnings {
        println!("{}", ui::warn(&format!("warning: {warning}")));
    }
    println!(
        "{}",
        ui::muted(
            "note: sync materializes inventory metadata (including classifications / PII flags). \
             Disk cache is mode 0600 under ~/.ethira; cleared on logout or `ethira sync --clear`."
        )
    );
}

#[allow(dead_code)]
pub fn edge_kind(edge: &Value) -> Option<&str> {
    edge.get("kind").and_then(|v| v.as_str())
}
