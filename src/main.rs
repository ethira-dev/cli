mod api;
mod api_url;
mod auth;
mod commands;
mod db;
mod exit_codes;
mod fs_util;
mod inventory;
mod onboarding;
mod redact;
mod shell;
mod ui;

use std::io;
use std::process::ExitCode;

use crate::ui::maybe_print_unauthorized;
use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};

const DEFAULT_API_URL: &str = "https://api.ethira.dev";

const AFTER_HELP: &str = "\
Examples:
  ethira login
  ethira sync && ethira sql 'SELECT name FROM systems LIMIT 10'
  ethira status --json
  ethira schema --json
  ethira routing --json
  ethira sql 'SELECT name, json_extract(raw_json, \"$.cloneUrl\") AS clone_url FROM repositories LIMIT 20'
  ethira sync --json
  ethira sql --sync --json 'SELECT kind, count(*) AS n FROM relationships GROUP BY kind'
  ethira logout

Agent contract: after sync, query commands are local-only (no Ethira API / MCP).
Exit codes: 0 ok · 1 error · 2 unauthorized · 3 cache missing · 4 SQL error

See also: ethira examples
";

#[derive(Parser, Debug)]
#[command(
    name = "ethira",
    version,
    about = "Ethira CLI — login, sync inventory, and sandboxed SQL",
    long_about = "Ethira CLI for OAuth login, inventory sync into a local SQLite cache, and \
sandboxed SQL queries.\n\n\
Agents should invoke this binary via the shell (`ethira … --json`), never via Ethira MCP. \
After `ethira sync`, `status` / `schema` / `tables` / `routing` / `sql` read only the local cache.\n\n\
Run with no subcommand to print getting-started examples. Use subcommands for day-to-day \
work and scripting (add --json and -q as needed).",
    after_help = AFTER_HELP
)]
struct Cli {
    /// Ethira API base URL
    #[arg(long, env = "ETHIRA_API_URL", global = true, default_value = DEFAULT_API_URL)]
    api_url: String,

    /// Allow non-default remote API hosts (OAuth tokens will be sent there)
    #[arg(long, global = true)]
    allow_untrusted_api_host: bool,

    /// Suppress non-essential progress, notes, and warnings
    #[arg(short = 'q', long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Log in via OAuth, select a workspace, then offer a first sync
    Login,
    /// Select the active shell workspace (sync still uses the OAuth token workspace)
    Workspace,
    /// Show the token-bound CLI workspace (and shell selection if it differs)
    Whoami {
        /// Machine-readable JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Local cache / login status (no network)
    Status {
        /// Machine-readable JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Describe local inventory tables and routing fields (no network)
    Schema {
        /// Machine-readable JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Repository and MCP-server routing directory from the local cache (no network)
    Routing {
        /// Machine-readable JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Sync inventory into ~/.ethira/inventory.sqlite
    Sync {
        /// Machine-readable JSON on stdout
        #[arg(long)]
        json: bool,
        /// Delete the local inventory cache and exit
        #[arg(long)]
        clear: bool,
    },
    /// List local cache tables and row counts (no network)
    Tables {
        /// Machine-readable JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Run a sandboxed SQL query against the local cache (no network unless --sync)
    Sql {
        /// SQL statement (SELECT / WITH only)
        query: String,
        /// Sync before running the query
        #[arg(long)]
        sync: bool,
        /// Machine-readable JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Remove saved credentials and inventory cache
    Logout,
    /// Print common usage examples
    Examples,
    /// Generate a shell completion script
    Completions {
        /// Target shell
        shell: Shell,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::from(exit_codes::SUCCESS),
        Err(err) => {
            let as_json = std::env::args().any(|a| a == "--json");
            let code = exit_codes::code_for(&err);
            if maybe_print_unauthorized(&err, as_json) {
                return ExitCode::from(code);
            }
            if as_json {
                let kind = match code {
                    exit_codes::CACHE_MISSING => "cache_missing",
                    exit_codes::SQL_ERROR => "sql_error",
                    exit_codes::UNAUTHORIZED => "unauthorized",
                    _ => "error",
                };
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "error": kind,
                        "message": format!("{err:#}"),
                        "exitCode": code,
                    })
                );
            } else {
                eprintln!("{} {err:#}", ui::error("error:"));
            }
            ExitCode::from(code)
        }
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let quiet = cli.quiet;

    // Local-only commands do not need a validated API URL.
    match &cli.command {
        None | Some(Commands::Examples) => {
            commands::print_examples();
            return Ok(());
        }
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            generate(*shell, &mut cmd, name, &mut io::stdout());
            return Ok(());
        }
        Some(Commands::Status { json }) => {
            return commands::status(*json);
        }
        Some(Commands::Schema { json }) => {
            return commands::schema(*json);
        }
        Some(Commands::Routing { json }) => {
            return commands::routing(*json);
        }
        Some(Commands::Tables { json }) => {
            return commands::tables(*json);
        }
        Some(Commands::Logout) => {
            return commands::logout(quiet).await;
        }
        Some(Commands::Sync { json, clear: true }) => {
            // clear is local-only; allow --json without network.
            return commands::sync("", *json, true, quiet).await;
        }
        Some(Commands::Sql {
            query,
            sync: false,
            json,
        }) => {
            // Local SQL: skip API URL validation entirely.
            return commands::sql("", query, false, *json, quiet).await;
        }
        _ => {}
    }

    let api_url = api_url::validate_and_normalize(&cli.api_url, cli.allow_untrusted_api_host)?;

    match cli.command {
        Some(Commands::Login) => commands::login(&api_url, quiet).await,
        Some(Commands::Workspace) => commands::workspace(&api_url, quiet).await,
        Some(Commands::Whoami { json }) => commands::whoami(&api_url, json, quiet).await,
        Some(Commands::Sync { json, clear }) => commands::sync(&api_url, json, clear, quiet).await,
        Some(Commands::Sql { query, sync, json }) => {
            commands::sql(&api_url, &query, sync, json, quiet).await
        }
        None
        | Some(Commands::Examples)
        | Some(Commands::Completions { .. })
        | Some(Commands::Status { .. })
        | Some(Commands::Schema { .. })
        | Some(Commands::Routing { .. })
        | Some(Commands::Tables { .. })
        | Some(Commands::Logout) => unreachable!("handled above"),
    }
}
