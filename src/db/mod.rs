use anyhow::{Context, Result, bail};
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, params_from_iter};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::exit_codes::{CacheMissingError, SqlError};
use crate::fs_util;

/// Max rows returned by a single sandboxed SQL query.
pub const MAX_SQL_ROWS: usize = 10_000;
/// Wall-clock budget for a sandboxed SQL query.
pub const SQL_TIME_BUDGET: Duration = Duration::from_secs(30);

/// Agent-facing JSON contract version for `--json` outputs.
pub const JSON_SCHEMA_VERSION: &str = "1";

/// Asset tables populated by inventory sync.
pub const ASSET_TABLES: &[&str] = &[
    "systems",
    "microservices",
    "datasets",
    "products",
    "devices",
    "agents",
    "processes",
    "repositories",
    "datapoints",
    "policies",
    "physical_locations",
    "ai_skills",
    "ai_models",
    "third_party_systems",
    "mcp_servers",
    "teams",
    "vendors",
    "incidents",
    "data_breaches",
];

const RELATIONSHIPS_DDL: &str = "
CREATE TABLE relationships (
    kind TEXT NOT NULL,
    from_type TEXT NOT NULL,
    from_id TEXT NOT NULL,
    from_business_id TEXT,
    to_type TEXT NOT NULL,
    to_id TEXT NOT NULL,
    to_business_id TEXT,
    meta_json TEXT
);
";

const META_DDL: &str = "
CREATE TABLE IF NOT EXISTS ethira_meta (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
";

/// Column documentation for agent `schema --json` output.
#[derive(Debug, Clone)]
pub struct ColumnDoc {
    pub name: &'static str,
    pub description: &'static str,
}

/// Suggested `json_extract(raw_json, '$.…')` paths for cross-repo / integration routing.
#[derive(Debug, Clone)]
pub struct RoutingFieldDoc {
    pub path: &'static str,
    pub description: &'static str,
}

const ASSET_COLUMNS: &[ColumnDoc] = &[
    ColumnDoc {
        name: "id",
        description: "Internal Ethira UUID",
    },
    ColumnDoc {
        name: "business_id",
        description: "Human-facing business id (table-specific API key)",
    },
    ColumnDoc {
        name: "name",
        description: "Display name",
    },
    ColumnDoc {
        name: "workspace_id",
        description: "Owning workspace id",
    },
    ColumnDoc {
        name: "team_id",
        description: "Owning team id when set",
    },
    ColumnDoc {
        name: "owner_user_id",
        description: "Owner user id when set",
    },
    ColumnDoc {
        name: "physical_location_id",
        description: "Physical location id when set",
    },
    ColumnDoc {
        name: "parent_id",
        description: "Parent asset id when set",
    },
    ColumnDoc {
        name: "system_id",
        description: "Owning system id (populated for repositories)",
    },
    ColumnDoc {
        name: "raw_json",
        description: "Full API JSON object; use json_extract for extra fields",
    },
];

const RELATIONSHIP_COLUMNS: &[ColumnDoc] = &[
    ColumnDoc {
        name: "kind",
        description: "Relationship kind string",
    },
    ColumnDoc {
        name: "from_type",
        description: "Source asset type",
    },
    ColumnDoc {
        name: "from_id",
        description: "Source asset id",
    },
    ColumnDoc {
        name: "from_business_id",
        description: "Source business id when present",
    },
    ColumnDoc {
        name: "to_type",
        description: "Target asset type",
    },
    ColumnDoc {
        name: "to_id",
        description: "Target asset id",
    },
    ColumnDoc {
        name: "to_business_id",
        description: "Target business id when present",
    },
    ColumnDoc {
        name: "meta_json",
        description: "Optional relationship metadata JSON",
    },
];

/// Common repository routing keys inside `raw_json` (best-effort; API may omit some).
pub const REPOSITORY_ROUTING_FIELDS: &[RoutingFieldDoc] = &[
    RoutingFieldDoc {
        path: "$.cloneUrl",
        description: "Git clone URL when provided",
    },
    RoutingFieldDoc {
        path: "$.gitUrl",
        description: "Alternate git URL",
    },
    RoutingFieldDoc {
        path: "$.url",
        description: "Primary URL (often HTTPS clone or API URL)",
    },
    RoutingFieldDoc {
        path: "$.htmlUrl",
        description: "Browser / hosting URL",
    },
    RoutingFieldDoc {
        path: "$.defaultBranch",
        description: "Default branch name",
    },
    RoutingFieldDoc {
        path: "$.provider",
        description: "Hosting provider (github, gitlab, …)",
    },
    RoutingFieldDoc {
        path: "$.externalId",
        description: "Provider-native repository id",
    },
    RoutingFieldDoc {
        path: "$.fullName",
        description: "org/name style identifier",
    },
];

/// Common MCP server routing keys inside `raw_json`.
pub const MCP_SERVER_ROUTING_FIELDS: &[RoutingFieldDoc] = &[
    RoutingFieldDoc {
        path: "$.url",
        description: "Remote MCP endpoint URL",
    },
    RoutingFieldDoc {
        path: "$.endpoint",
        description: "Alternate endpoint field",
    },
    RoutingFieldDoc {
        path: "$.transport",
        description: "Transport hint (stdio, sse, http, …)",
    },
    RoutingFieldDoc {
        path: "$.command",
        description: "Local stdio command when applicable",
    },
    RoutingFieldDoc {
        path: "$.args",
        description: "Command arguments for stdio servers",
    },
];

fn asset_table_ddl(name: &str) -> String {
    format!(
        "CREATE TABLE {name} (
            id TEXT,
            business_id TEXT,
            name TEXT,
            workspace_id TEXT,
            team_id TEXT,
            owner_user_id TEXT,
            physical_location_id TEXT,
            parent_id TEXT,
            system_id TEXT,
            raw_json TEXT NOT NULL
        );"
    )
}

/// Inventory database with a read-only SQL sandbox (memory or file-backed).
#[derive(Clone)]
pub struct InventoryDb {
    conn: Arc<Mutex<Connection>>,
}

impl InventoryDb {
    pub fn ethira_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("could not resolve home directory")?;
        Ok(home.join(".ethira"))
    }

    pub fn cache_path() -> Result<PathBuf> {
        Ok(Self::ethira_dir()?.join("inventory.sqlite"))
    }

    pub fn ensure_ethira_dir() -> Result<PathBuf> {
        let dir = Self::ethira_dir()?;
        fs_util::ensure_private_dir(&dir)?;
        Ok(dir)
    }

    pub fn clear_cache() -> Result<()> {
        let path = Self::cache_path()?;
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn new() -> Result<Self> {
        let conn = Connection::open_in_memory_with_flags(
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
        )
        .context("failed to open in-memory sqlite")?;
        conn.execute_batch("PRAGMA foreign_keys = OFF;")
            .context("failed to configure sqlite")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_file(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs_util::ensure_private_dir(parent)?;
        }
        // Create with owner-only perms before SQLite opens so there is no umask window.
        fs_util::ensure_private_file(path)?;
        let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        conn.execute_batch("PRAGMA foreign_keys = OFF;")
            .context("failed to configure sqlite")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_file_readonly(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(CacheMissingError::new(format!(
                "no inventory cache at {} — run `ethira sync` first",
                path.display()
            ))
            .into());
        }
        fs_util::refuse_symlink(path)?;
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .with_context(|| format!("open readonly {}", path.display()))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_cache() -> Result<Self> {
        Self::ensure_ethira_dir()?;
        Self::open_file(&Self::cache_path()?)
    }

    pub fn open_cache_readonly() -> Result<Self> {
        Self::open_file_readonly(&Self::cache_path()?)
    }

    /// True when there is no usable local inventory cache (first sync recommended).
    pub fn cache_needs_sync() -> bool {
        let Ok(path) = Self::cache_path() else {
            return true;
        };
        if !path.exists() {
            return true;
        }
        match Self::open_file_readonly(&path) {
            Ok(db) => {
                let assets_empty = ASSET_TABLES.iter().all(|t| db.count(t).unwrap_or(0) == 0);
                let rel_empty = db.count("relationships").unwrap_or(0) == 0;
                assets_empty && rel_empty
            }
            Err(_) => true,
        }
    }

    fn set_authorizer(conn: &Connection, enabled: bool) -> Result<()> {
        if enabled {
            conn.authorizer(Some(|ctx: AuthContext<'_>| match ctx.action {
                AuthAction::Select
                | AuthAction::Read { .. }
                | AuthAction::Transaction { .. }
                | AuthAction::Recursive => Authorization::Allow,
                AuthAction::Function { function_name } => {
                    if is_allowed_sql_function(function_name) {
                        Authorization::Allow
                    } else {
                        Authorization::Deny
                    }
                }
                _ => Authorization::Deny,
            }))
            .context("enable sqlite authorizer")?;
        } else {
            conn.authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
                .context("disable sqlite authorizer")?;
        }
        Ok(())
    }

    pub fn rebuild_schema(&self) -> Result<()> {
        let conn = self.conn.lock().expect("inventory db lock");
        Self::set_authorizer(&conn, false)?;
        for table in ASSET_TABLES {
            conn.execute(&format!("DROP TABLE IF EXISTS {table}"), [])
                .with_context(|| format!("drop {table}"))?;
            conn.execute_batch(&asset_table_ddl(table))
                .with_context(|| format!("create {table}"))?;
        }
        conn.execute_batch("DROP TABLE IF EXISTS relationships;")
            .context("drop relationships")?;
        conn.execute_batch(RELATIONSHIPS_DDL)
            .context("create relationships")?;
        // Meta is preserved across rebuilds (not dropped); ensure table exists.
        conn.execute_batch(META_DDL).context("create ethira_meta")?;
        Ok(())
    }

    /// Persist sync freshness / workspace metadata (local only; never read from API at query time).
    pub fn set_sync_meta(
        &self,
        workspace_id: &str,
        workspace_name: &str,
        synced_at_rfc3339: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("inventory db lock");
        Self::set_authorizer(&conn, false)?;
        conn.execute_batch(META_DDL).context("ensure ethira_meta")?;
        for (key, value) in [
            ("last_synced_at", synced_at_rfc3339),
            ("workspace_id", workspace_id),
            ("workspace_name", workspace_name),
            ("schema_version", JSON_SCHEMA_VERSION),
        ] {
            conn.execute(
                "INSERT INTO ethira_meta(key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [key, value],
            )
            .with_context(|| format!("set ethira_meta.{key}"))?;
        }
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().expect("inventory db lock");
        Self::set_authorizer(&conn, false)?;
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='ethira_meta'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if !exists {
            return Ok(None);
        }
        let mut stmt = conn
            .prepare("SELECT value FROM ethira_meta WHERE key = ?1")
            .context("prepare ethira_meta select")?;
        let mut rows = stmt.query([key]).context("query ethira_meta")?;
        if let Some(row) = rows.next().context("read ethira_meta row")? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    /// Build agent-facing schema documentation from the live cache (no network).
    pub fn schema_document(&self) -> Value {
        let counts: Vec<(&str, i64)> = self
            .tables()
            .into_iter()
            .map(|name| (name, self.count(name).unwrap_or(0)))
            .collect();
        Self::schema_document_with_counts(&counts)
    }

    /// Schema documentation with zero row counts (cache missing).
    pub fn static_schema_document() -> Value {
        let counts: Vec<(&str, i64)> = ASSET_TABLES
            .iter()
            .copied()
            .chain(std::iter::once("relationships"))
            .map(|name| (name, 0))
            .collect();
        let mut doc = Self::schema_document_with_counts(&counts);
        if let Some(obj) = doc.as_object_mut() {
            obj.insert("cachePresent".into(), json!(false));
        }
        doc
    }

    fn schema_document_with_counts(counts: &[(&str, i64)]) -> Value {
        let mut tables = Vec::new();
        for (name, count) in counts {
            let kind = if *name == "relationships" {
                "edge"
            } else {
                "asset"
            };
            let columns = if *name == "relationships" {
                RELATIONSHIP_COLUMNS
            } else {
                ASSET_COLUMNS
            };
            let routing = if *name == "repositories" {
                json!(
                    REPOSITORY_ROUTING_FIELDS
                        .iter()
                        .map(|f| json!({ "path": f.path, "description": f.description }))
                        .collect::<Vec<_>>()
                )
            } else if *name == "mcp_servers" {
                json!(
                    MCP_SERVER_ROUTING_FIELDS
                        .iter()
                        .map(|f| json!({ "path": f.path, "description": f.description }))
                        .collect::<Vec<_>>()
                )
            } else {
                Value::Null
            };
            tables.push(json!({
                "name": name,
                "kind": kind,
                "rowCount": count,
                "columns": columns.iter().map(|c| json!({
                    "name": c.name,
                    "description": c.description,
                })).collect::<Vec<_>>(),
                "routingFields": routing,
            }));
        }

        json!({
            "schemaVersion": JSON_SCHEMA_VERSION,
            "localOnly": true,
            "cachePresent": true,
            "sql": {
                "allowed": ["SELECT", "WITH"],
                "notes": [
                    "Single statement only; no writes, ATTACH, PRAGMA, or load_extension",
                    "Use json_extract(raw_json, '$.field') for fields not lifted into columns",
                    "Query commands never call the Ethira API (except ethira sql --sync)"
                ],
            },
            "examples": [
                "SELECT name, system_id, json_extract(raw_json, '$.cloneUrl') AS clone_url FROM repositories LIMIT 20",
                "SELECT name, json_extract(raw_json, '$.url') AS url, json_extract(raw_json, '$.transport') AS transport FROM mcp_servers",
                "SELECT kind, from_type, from_id, to_type, to_id FROM relationships WHERE from_id = '<id>' OR to_id = '<id>'",
            ],
            "tables": tables,
        })
    }

    /// Current unix timestamp as RFC3339 UTC string (second precision).
    pub fn now_rfc3339() -> String {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        chrono::DateTime::from_timestamp(secs, 0)
            .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_else(|| secs.to_string())
    }

    pub fn insert_asset(&self, table: &str, item: &Value) -> Result<()> {
        if !ASSET_TABLES.contains(&table) {
            bail!("unknown asset table: {table}");
        }
        let obj = item
            .as_object()
            .context("asset item must be a JSON object")?;
        let id = string_field(obj, "id");
        let business_id = string_field(obj, business_id_key(table));
        let name = string_field(obj, "name");
        let workspace_id = string_field(obj, "workspaceId");
        let team_id = string_field(obj, "teamId");
        let owner_user_id = string_field(obj, "ownerUserId");
        let physical_location_id = string_field(obj, "physicalLocationId");
        let parent_id = string_field(obj, "parentId");
        // Repository FK to owning system (not the system's business id).
        let system_id = if table == "repositories" {
            string_field(obj, "systemId")
        } else {
            None
        };
        let raw_json = serde_json::to_string(item).context("serialize asset json")?;

        let sql = format!(
            "INSERT INTO {table} (
                id, business_id, name, workspace_id, team_id, owner_user_id,
                physical_location_id, parent_id, system_id, raw_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"
        );
        let conn = self.conn.lock().expect("inventory db lock");
        Self::set_authorizer(&conn, false)?;
        conn.execute(
            &sql,
            params_from_iter([
                id,
                business_id,
                name,
                workspace_id,
                team_id,
                owner_user_id,
                physical_location_id,
                parent_id,
                system_id,
                Some(raw_json),
            ]),
        )
        .with_context(|| format!("insert into {table}"))?;
        Ok(())
    }

    pub fn insert_relationship(&self, edge: &Value) -> Result<()> {
        let obj = edge
            .as_object()
            .context("relationship edge must be a JSON object")?;
        let kind = required_string(obj, "kind")?;
        let from_type = required_string(obj, "fromType")?;
        let from_id = required_string(obj, "fromId")?;
        let from_business_id = string_field(obj, "fromBusinessId");
        let to_type = required_string(obj, "toType")?;
        let to_id = required_string(obj, "toId")?;
        let to_business_id = string_field(obj, "toBusinessId");
        let meta_json = obj.get("meta").map(serde_json::to_string).transpose()?;

        let conn = self.conn.lock().expect("inventory db lock");
        Self::set_authorizer(&conn, false)?;
        conn.execute(
            "INSERT INTO relationships (
                kind, from_type, from_id, from_business_id,
                to_type, to_id, to_business_id, meta_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params_from_iter([
                Some(kind),
                Some(from_type),
                Some(from_id),
                from_business_id,
                Some(to_type),
                Some(to_id),
                to_business_id,
                meta_json,
            ]),
        )
        .context("insert relationship")?;
        Ok(())
    }

    pub fn count(&self, table: &str) -> Result<i64> {
        if !is_allowed_table(table) {
            bail!("unknown table: {table}");
        }
        let conn = self.conn.lock().expect("inventory db lock");
        Self::set_authorizer(&conn, true)?;
        let mut stmt = conn
            .prepare(&format!("SELECT COUNT(*) FROM {table}"))
            .with_context(|| format!("count {table}"))?;
        let n: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(n)
    }

    pub fn tables(&self) -> Vec<&'static str> {
        let mut names = ASSET_TABLES.to_vec();
        names.push("relationships");
        names
    }

    /// Execute a single sandboxed read-only SQL statement.
    pub fn query(&self, sql: &str) -> Result<QueryResult> {
        let sql = validate_readonly_sql(sql).map_err(|e| SqlError::new(format!("{e:#}")))?;
        let conn = self.conn.lock().expect("inventory db lock");
        Self::set_authorizer(&conn, true)?;

        let started = Instant::now();
        // rusqlite: return `true` from the progress handler to interrupt.
        conn.progress_handler(1000, Some(move || started.elapsed() >= SQL_TIME_BUDGET))
            .map_err(|e| SqlError::new(format!("set progress handler: {e}")))?;

        let result = (|| {
            let mut stmt = conn
                .prepare(sql)
                .map_err(|e| SqlError::new(format!("prepare sql: {e}")))?;
            let columns: Vec<String> = stmt
                .column_names()
                .iter()
                .map(|name| (*name).to_string())
                .collect();
            let mut rows = stmt
                .query([])
                .map_err(|e| SqlError::new(format!("execute sql: {e}")))?;
            let mut out_rows = Vec::new();
            while let Some(row) = rows
                .next()
                .map_err(|e| SqlError::new(format!("read row: {e}")))?
            {
                if out_rows.len() >= MAX_SQL_ROWS {
                    return Err(SqlError::new(format!(
                        "query exceeded max rows ({MAX_SQL_ROWS}); add a tighter LIMIT or filter"
                    ))
                    .into());
                }
                let mut values = Vec::with_capacity(columns.len());
                for i in 0..columns.len() {
                    values
                        .push(value_ref_to_json(row.get_ref(i).map_err(|e| {
                            SqlError::new(format!("read column {i}: {e}"))
                        })?));
                }
                out_rows.push(values);
            }
            Ok(QueryResult {
                columns,
                rows: out_rows,
            })
        })();

        // Clear progress handler so it does not outlive this query.
        let _ = conn.progress_handler(0, None::<fn() -> bool>);
        result
    }
}

#[derive(Debug)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// Max characters shown per cell in human-readable table output.
const TABLE_CELL_MAX: usize = 64;

impl QueryResult {
    pub fn print_table(&self) {
        if self.columns.is_empty() {
            println!("(no columns)");
            return;
        }
        let headers: Vec<String> = self
            .columns
            .iter()
            .map(|c| truncate_cell(c, TABLE_CELL_MAX))
            .collect();
        let header_line = headers.join(" | ");
        println!("{header_line}");
        println!("{}", "-".repeat(header_line.chars().count().max(3)));
        if self.rows.is_empty() {
            println!("(0 rows)");
            return;
        }
        for row in &self.rows {
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let col = self.columns.get(i).map(String::as_str).unwrap_or("");
                    format_cell(col, v, TABLE_CELL_MAX)
                })
                .collect();
            println!("{}", cells.join(" | "));
        }
        println!("({} rows)", self.rows.len());
    }
}

fn format_cell(column: &str, value: &Value, max: usize) -> String {
    // raw_json is the full API payload; keep human tables scannable.
    if column == "raw_json" || column == "meta_json" {
        return match value {
            Value::Null => "NULL".into(),
            Value::String(s) if s.is_empty() => String::new(),
            _ => truncate_cell("<json>", max),
        };
    }
    truncate_cell(&json_cell(value), max)
}

fn truncate_cell(s: &str, max: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn json_cell(value: &Value) -> String {
    match value {
        Value::Null => "NULL".into(),
        Value::String(s) => s.replace('\n', "\\n"),
        other => other.to_string(),
    }
}

fn value_ref_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => Value::from(f),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Value::String(format!("<blob {} bytes>", b.len())),
    }
}

fn string_field(obj: &Map<String, Value>, key: &str) -> Option<String> {
    obj.get(key).and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    })
}

fn business_id_key(table: &str) -> &'static str {
    match table {
        "systems" => "systemId",
        "microservices" => "microserviceId",
        "datasets" => "datasetId",
        "products" => "productId",
        "devices" => "deviceId",
        "agents" => "agentId",
        "processes" => "processId",
        "repositories" => "repositoryId",
        "policies" => "policyId",
        "physical_locations" => "physicalLocationId",
        "ai_skills" => "aiSkillId",
        "ai_models" => "aiModelId",
        "third_party_systems" => "thirdPartySystemId",
        "teams" => "teamId",
        "mcp_servers" | "datapoints" | "vendors" | "incidents" | "data_breaches" => "id",
        _ => "id",
    }
}

fn required_string(obj: &Map<String, Value>, key: &str) -> Result<String> {
    string_field(obj, key).with_context(|| format!("missing string field {key}"))
}

/// Tables allowed in internal `COUNT(*)` helpers.
fn is_allowed_table(table: &str) -> bool {
    ASSET_TABLES.contains(&table) || table == "relationships"
}

/// SQLite functions permitted under the read-only authorizer.
fn is_allowed_sql_function(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        // Aggregates
        "count"
            | "sum"
            | "avg"
            | "min"
            | "max"
            | "group_concat"
            | "total"
            // Scalar / string / json helpers used by ethira examples
            | "json_extract"
            | "json_type"
            | "json_array_length"
            | "json_valid"
            | "lower"
            | "upper"
            | "length"
            | "substr"
            | "substring"
            | "trim"
            | "ltrim"
            | "rtrim"
            | "replace"
            | "instr"
            | "coalesce"
            | "ifnull"
            | "nullif"
            | "typeof"
            | "hex"
            | "quote"
            | "abs"
            | "round"
            | "cast"
            | "like"
            | "glob"
            | "char"
            | "unicode"
    )
}

/// Reject writes, multi-statements, attach/load, and dangerous pragmas.
pub fn validate_readonly_sql(sql: &str) -> Result<&str> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    if trimmed.is_empty() {
        bail!("empty SQL");
    }
    if trimmed.contains(';') {
        bail!("multi-statement SQL is not allowed");
    }
    // Normalize all Unicode/ASCII whitespace so deny-list checks cannot be
    // bypassed with newlines/tabs (e.g. `SELECT\nload_extension(...)`).
    let normalized: String = trimmed
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let first = normalized.split(' ').next().unwrap_or("");
    if first != "select" && first != "with" {
        bail!("only SELECT / WITH statements are allowed");
    }
    // Match regardless of surrounding whitespace (normalized already).
    // Statement keywords only (not scalar functions like replace()).
    let denied_tokens = [
        "attach",
        "detach",
        "load_extension",
        "pragma",
        "insert",
        "update",
        "delete",
        "drop",
        "alter",
        "create",
        "vacuum",
        "reindex",
    ];
    let padded = format!(" {normalized} ");
    for token in denied_tokens {
        if padded.contains(&format!(" {token} ")) || padded.contains(&format!(" {token}(")) {
            bail!("disallowed SQL keyword/construct: {token}");
        }
    }
    if padded.contains(" replace into ") {
        bail!("disallowed SQL keyword/construct: replace");
    }
    // Also catch pragma_* table-valued functions without a space before '('.
    if normalized.contains("pragma_") || normalized.contains("load_extension") {
        bail!("disallowed SQL keyword/construct: pragma/load_extension");
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_writes_and_attach() {
        assert!(validate_readonly_sql("DELETE FROM systems").is_err());
        assert!(validate_readonly_sql("SELECT 1; SELECT 2").is_err());
        assert!(validate_readonly_sql("ATTACH 'x.db' AS other").is_err());
        assert!(validate_readonly_sql("SELECT * FROM systems").is_ok());
        assert!(validate_readonly_sql("WITH x AS (SELECT 1) SELECT * FROM x").is_ok());
    }

    #[test]
    fn rejects_whitespace_keyword_bypasses() {
        assert!(validate_readonly_sql("SELECT\nload_extension('x')").is_err());
        assert!(validate_readonly_sql("SELECT\tload_extension('x')").is_err());
        assert!(validate_readonly_sql("SELECT * FROM pragma_table_info('systems')").is_err());
        assert!(validate_readonly_sql("SELECT  pragma  ('x')").is_err());
    }

    #[test]
    fn authorizer_denies_disallowed_functions() {
        let db = InventoryDb::new().unwrap();
        db.rebuild_schema().unwrap();
        // load_extension must be denied by the authorizer even if the filter missed it.
        let err = db
            .query("SELECT hex(randomblob(4))")
            .expect_err("randomblob should be denied");
        let msg = format!("{err:#}");
        assert!(
            msg.to_ascii_lowercase().contains("authorize")
                || msg.to_ascii_lowercase().contains("not authorized")
                || msg.to_ascii_lowercase().contains("denied"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn count_rejects_unknown_tables() {
        let db = InventoryDb::new().unwrap();
        db.rebuild_schema().unwrap();
        assert!(db.count("systems").is_ok());
        assert!(db.count("relationships").is_ok());
        assert!(db.count("sqlite_master;--").is_err());
    }

    #[test]
    fn allows_json_extract_helper() {
        let db = InventoryDb::new().unwrap();
        db.rebuild_schema().unwrap();
        db.insert_asset(
            "systems",
            &json!({
                "id": "s1",
                "systemId": "auth",
                "name": "Auth",
                "criticality": "high"
            }),
        )
        .unwrap();
        let result = db
            .query("SELECT json_extract(raw_json, '$.criticality') FROM systems")
            .unwrap();
        assert_eq!(result.rows[0][0], json!("high"));
    }

    #[test]
    fn human_table_hides_raw_json_and_truncates() {
        assert_eq!(format_cell("raw_json", &json!({"a": 1}), 64), "<json>");
        let long = "x".repeat(80);
        let shown = truncate_cell(&long, 64);
        assert_eq!(shown.chars().count(), 65); // 64 + ellipsis
        assert!(shown.ends_with('…'));
    }

    #[test]
    fn sync_and_query_roundtrip() {
        let db = InventoryDb::new().unwrap();
        db.rebuild_schema().unwrap();
        db.insert_asset(
            "systems",
            &json!({
                "id": "s1",
                "systemId": "auth",
                "name": "Auth",
                "workspaceId": "w1"
            }),
        )
        .unwrap();
        db.insert_relationship(&json!({
            "kind": "system_datasets",
            "fromType": "system",
            "fromId": "s1",
            "fromBusinessId": "auth",
            "toType": "dataset",
            "toId": "d1"
        }))
        .unwrap();
        assert_eq!(db.count("systems").unwrap(), 1);
        let result = db
            .query("SELECT name FROM systems WHERE business_id = 'auth'")
            .unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], json!("Auth"));
    }

    #[test]
    fn file_roundtrip_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inv.sqlite");
        let db = InventoryDb::open_file(&path).unwrap();
        db.rebuild_schema().unwrap();
        db.insert_asset(
            "systems",
            &json!({ "id": "s1", "systemId": "auth", "name": "Auth" }),
        )
        .unwrap();
        drop(db);

        let ro = InventoryDb::open_file_readonly(&path).unwrap();
        assert_eq!(ro.count("systems").unwrap(), 1);
    }

    #[test]
    fn sync_meta_roundtrip() {
        let db = InventoryDb::new().unwrap();
        db.rebuild_schema().unwrap();
        db.set_sync_meta("w1", "Acme", "2026-07-22T08:00:00Z")
            .unwrap();
        assert_eq!(
            db.get_meta("last_synced_at").unwrap().as_deref(),
            Some("2026-07-22T08:00:00Z")
        );
        assert_eq!(db.get_meta("workspace_id").unwrap().as_deref(), Some("w1"));
    }

    #[test]
    fn schema_document_includes_routing_fields() {
        let doc = InventoryDb::static_schema_document();
        assert_eq!(doc["schemaVersion"], json!("1"));
        let tables = doc["tables"].as_array().unwrap();
        let repos = tables.iter().find(|t| t["name"] == "repositories").unwrap();
        assert!(repos["routingFields"].as_array().unwrap().len() >= 3);
        let mcps = tables.iter().find(|t| t["name"] == "mcp_servers").unwrap();
        assert!(mcps["routingFields"].as_array().unwrap().len() >= 2);
    }

    #[test]
    fn query_sql_errors_are_typed() {
        let db = InventoryDb::new().unwrap();
        db.rebuild_schema().unwrap();
        let err = db.query("DELETE FROM systems").unwrap_err();
        assert!(err.downcast_ref::<SqlError>().is_some());
    }

    #[test]
    fn query_rejects_over_max_rows() {
        let db = InventoryDb::new().unwrap();
        db.rebuild_schema().unwrap();
        for i in 0..=MAX_SQL_ROWS {
            db.insert_asset(
                "systems",
                &json!({
                    "id": format!("s{i}"),
                    "systemId": format!("sys-{i}"),
                    "name": format!("system-{i}"),
                }),
            )
            .unwrap();
        }
        let err = db.query("SELECT name FROM systems").unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("max rows"),
            "expected max-rows error, got: {msg}"
        );
        assert!(err.downcast_ref::<SqlError>().is_some());
    }

    #[test]
    fn repository_routing_extracts_clone_url() {
        let db = InventoryDb::new().unwrap();
        db.rebuild_schema().unwrap();
        db.insert_asset(
            "repositories",
            &json!({
                "id": "r1",
                "repositoryId": "payments-api",
                "name": "payments-api",
                "systemId": "s1",
                "cloneUrl": "https://github.com/acme/payments-api.git",
                "defaultBranch": "main",
                "provider": "github",
                "fullName": "acme/payments-api"
            }),
        )
        .unwrap();
        db.insert_asset(
            "mcp_servers",
            &json!({
                "id": "m1",
                "name": "tickets",
                "url": "https://mcp.example.com/tickets",
                "transport": "http"
            }),
        )
        .unwrap();
        let repos = db
            .query(
                "SELECT coalesce(json_extract(raw_json, '$.cloneUrl'), json_extract(raw_json, '$.url')) AS url,
                        json_extract(raw_json, '$.provider') AS provider
                 FROM repositories",
            )
            .unwrap();
        assert_eq!(
            repos.rows[0][0],
            json!("https://github.com/acme/payments-api.git")
        );
        assert_eq!(repos.rows[0][1], json!("github"));
        let mcps = db
            .query("SELECT json_extract(raw_json, '$.url') FROM mcp_servers")
            .unwrap();
        assert_eq!(mcps.rows[0][0], json!("https://mcp.example.com/tickets"));
    }
}
