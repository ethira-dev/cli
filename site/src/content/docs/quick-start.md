---
title: Quick start
description: Login, sync, and run your first inventory queries in a few commands.
order: 1
---

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

## Examples

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

Quick tour in the binary: `ethira` · `ethira examples` · `ethira --help`.
