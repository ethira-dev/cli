# Ethira CLI

Open-source Rust client for Ethira: OAuth login, inventory sync into a local SQLite cache, and sandboxed SQL queries.

**Docs:** [ethira-dev.github.io/cli](https://ethira-dev.github.io/cli/)

**Positioning:** this repository is an open-source **client**. The Ethira API and inventory service are a separate hosted product. The default API host is `https://api.ethira.dev`. Forks and self-hosted setups may point at another HTTPS API with an explicit trust override (see Configuration).

**Agent model:** sync downloads inventory once; agents query **only via this CLI** (`ethira … --json`). There is no Ethira MCP surface for inventory, and query commands do not call the Ethira API (except explicit `ethira sql --sync`).

Licensed under **Apache-2.0 OR MIT**. See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT), [NOTICE](NOTICE), and [TRADEMARKS.md](TRADEMARKS.md).

API docs: [https://api.ethira.dev/api](https://api.ethira.dev/api)

Quick tour in the binary: `ethira` · `ethira examples` · `ethira --help`

Agent discovery: [AGENTS.md](AGENTS.md) · [`skills/ethira-inventory/SKILL.md`](skills/ethira-inventory/SKILL.md) · JSON contracts in [`schemas/v1/`](schemas/v1/README.md)

Install the agent skill into Cursor / Claude Code / other agents:

```bash
npx skills add ethira-dev/cli
# optional: global for all projects
npx skills add ethira-dev/cli -g
```

## Requirements

- Rust 1.85+ (edition 2024; see `rust-toolchain.toml`)
- Network access to Ethira (`https://api.ethira.dev` by default) for login/sync only
- Your Ethira workspace must allow CLI OAuth login with loopback redirect URIs (`http://127.0.0.1`)
- API inventory sync routes: `GET /v1/cli/inventory/*`

## Install

**Homebrew**

```bash
brew install ethira-dev/tap/ethira
```

**Linux / macOS (verified script)** — checks `SHA256SUMS` before install:

```bash
curl -fsSL https://raw.githubusercontent.com/ethira-dev/cli/main/scripts/install.sh | sh
```

**From source**

```bash
cargo build --release
# or
cargo install --path .
```

Binary: `target/release/ethira` (or `~/.cargo/bin/ethira` / `~/.local/bin/ethira`).

Prebuilt archives, checksums, SBOM, and attestations: [GitHub Releases](https://github.com/ethira-dev/cli/releases). See [docs/releasing.md](docs/releasing.md).

## Getting started

```bash
ethira              # print examples
ethira login        # OAuth + workspace; offers first sync when cache is empty
ethira sync
ethira status
ethira tables
ethira routing
ethira sql 'SELECT name FROM systems LIMIT 10'
```

Sync always uses the **OAuth token workspace**. `ethira workspace` changes the local shell selection only; the header/`whoami` warning notes a mismatch if they differ. Re-run `ethira login` to consent for another token workspace.

## Commands

```bash
ethira login
ethira workspace
ethira whoami [--json]
ethira status [--json]          # local-only
ethira schema [--json]          # local-only
ethira routing [--json]         # local-only: repos + MCP servers with URLs
ethira sync [--json] [--clear]
ethira tables [--json]          # local-only
ethira sql 'SELECT …' [--sync] [--json]   # local-only unless --sync
ethira logout
ethira examples
ethira completions <shell>
```

Global flags: `--api-url`, `--allow-untrusted-api-host`, `-q` / `--quiet` (suppress progress, notes, and non-security warnings).

Examples:

```bash
ethira sync && ethira sql 'SELECT name FROM systems LIMIT 10'
ethira status --json
ethira schema --json
ethira routing --json
ethira sql 'SELECT name, json_extract(raw_json, "$.criticality") AS criticality FROM incidents LIMIT 10'
ethira sql 'SELECT name, system_id, json_extract(raw_json, "$.cloneUrl") AS clone_url FROM repositories LIMIT 20'
ethira sync --json
ethira sql --sync --json 'SELECT kind, count(*) AS n FROM relationships GROUP BY kind'
ethira sync -q && ethira tables --json
```

## Agent / scripting contract

| Concern | Behavior |
| --- | --- |
| Runtime | Shell out to `ethira`; parse `--json` stdout |
| Not supported | Ethira MCP for inventory; guessing inventory facts |
| Local-only | `status`, `schema`, `tables`, `routing`, `sql` (without `--sync`) |
| Network | `login`, `whoami`, `sync`, `sql --sync`, `workspace` |
| Schemas | [`schemas/v1/`](schemas/v1/README.md), field `schemaVersion: "1"` |

### Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | Generic error |
| 2 | Unauthorized / not logged in |
| 3 | Local inventory cache missing |
| 4 | SQL rejected or failed |

On failure with `--json`, stderr is a small JSON object (`error`, `message`, …).

## Completions

```bash
# zsh (user-local example)
ethira completions zsh > ~/.zfunc/_ethira

# bash (user-local example)
mkdir -p ~/.local/share/bash-completion/completions
ethira completions bash > ~/.local/share/bash-completion/completions/ethira

# fish
ethira completions fish > ~/.config/fish/completions/ethira.fish
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

## Local cache

Path: `~/.ethira/inventory.sqlite` (directory `0700`, file `0600` on Unix; owner-restricted ACL on Windows)

- Written by `ethira sync` (also stores `last_synced_at` / workspace in `ethira_meta`)
- Read by `status` / `schema` / `tables` / `routing` / `sql`
- Cleared by `ethira logout` or `ethira sync --clear`

**Sensitive data:** sync materializes inventory metadata (tags, CIA, PII *flags*, classifications, vendor metadata, routing URLs) locally. The API is **designed** to omit secrets and email addresses from sync payloads; treat the cache like credentials regardless. See [SECURITY.md](SECURITY.md) and [docs/api-coordination.md](docs/api-coordination.md).

SQL is sandboxed: single-statement `SELECT` / `WITH` only (keyword filter + SQLite authorizer with a function allowlist). Queries are capped (row/time limits). No write-back to Ethira.

### Cross-repo routing

`ethira routing --json` projects repository and MCP-server rows with best-effort URL / transport fields from `raw_json` (`cloneUrl`, `gitUrl`, `url`, `htmlUrl`, `defaultBranch`, `provider`, …). Treat those URLs as **untrusted until the user approves** them before `git clone`, `gh`, or connecting another MCP. Ethira does not proxy those integrations.

## Configuration

| Flag / env | Default | Description |
| --- | --- | --- |
| `--api-url` | `https://api.ethira.dev` | API base URL |
| `ETHIRA_API_URL` | same | Same as `--api-url` |
| `--allow-untrusted-api-host` | off | Required for non-default, non-loopback HTTPS API hosts (tokens will be sent there) |
| `ETHIRA_ALLOW_PLAINTEXT_TOKENS` | unset | If `1`/`true`, allow storing tokens in `credentials.json` when the OS keychain is unavailable (loud warning) |
| `ETHIRA_COLOR_BG` | auto | Force `dark` or `light` primary (`#A9FBC0` / `#016853`) |
| `NO_COLOR` | unset | Disable ANSI colors |
| `-q` / `--quiet` | off | Suppress non-essential stderr/stdout chatter |

**API URL policy:** `https://` is required. Cleartext `http://` is allowed only for loopback hosts (`localhost`, `127.0.0.1`, `::1`). Non-default remote hosts require `--allow-untrusted-api-host`.

```bash
ETHIRA_API_URL=http://localhost:3000 ethira login
ethira --api-url https://api.example.com --allow-untrusted-api-host login
```

## Credentials

- **Tokens:** stored in the OS keychain (`ethira-cli` service) when available (fail closed if keychain is unavailable, unless `ETHIRA_ALLOW_PLAINTEXT_TOKENS` is set)
- **Metadata:** `~/.ethira/credentials.json` (`0600`, written atomically) — API URL, client id, expiry, shell workspace
- Legacy plaintext tokens in `credentials.json` are migrated into the keychain on load
- Tokens are never printed to the terminal
- `ethira logout` best-effort revokes the refresh/access token server-side, then deletes local credentials and cache

## Security

- Report vulnerabilities privately — see [SECURITY.md](SECURITY.md)
- OAuth scopes: `mcp:tools mcp:resources offline_access` (platform OAuth resource for CLI inventory auth — **not** an agent inventory interface). A stolen refresh token may authorize more than “download inventory” until a narrower CLI scope exists ([docs/api-coordination.md](docs/api-coordination.md)).
- HTTP clients use connect/request timeouts; unreachable APIs fail rather than hang forever.

## OAuth

- DCR: `POST /v1/oauth/register`
- Authorize: `GET /v1/oauth/authorize` (PKCE S256, loopback callback, 5-minute timeout)
- Token: `POST /v1/oauth/token` (authorization code + refresh)
- Revoke: `POST /v1/oauth/revoke` (best-effort on logout)
- Expired access tokens are refreshed automatically when a refresh token is present

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) (DCO required) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
