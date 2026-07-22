---
name: ethira-inventory
description: >-
  Query Ethira org inventory (systems, repositories, datasets, relationships,
  MCP servers) via the ethira CLI shell commands. Use when the user asks about
  inventory, ownership, blast radius, which repo owns a service, or related
  assets. Never guess inventory facts; never use Ethira MCP.
---

# Ethira inventory (CLI only)

## Rules

1. Invoke **`ethira` as a subprocess** (shell / Bash tool). Parse `--json` stdout.
2. **Never** register or call an Ethira MCP server for inventory.
3. **Never invent** inventory entities — if the cache is empty or a query returns no rows, say so and suggest `ethira sync` / broader SQL.
4. After sync, `status`, `schema`, `tables`, `routing`, and `sql` (without `--sync`) are **local-only** (no Ethira API).

## Orient

```bash
ethira status --json
ethira schema --json
ethira routing --json
```

- `status` — logged in?, cache path, `lastSyncedAt`, exit-code legend.
- `schema` — table/column docs + `routingFields` for `repositories` and `mcp_servers`.
- `routing` — ready-made directory of repo URLs / MCP endpoints for the next tool.

## Query

```bash
ethira tables --json
ethira sql --json 'SELECT name, business_id FROM systems LIMIT 50'
ethira sql --json 'SELECT kind, from_type, from_id, to_type, to_id FROM relationships WHERE from_id = ''<id>'' OR to_id = ''<id>'''
ethira sql --json 'SELECT name, system_id, json_extract(raw_json, ''$.cloneUrl'') AS clone_url FROM repositories LIMIT 50'
```

Use `json_extract(raw_json, '$.field')` for fields not lifted into columns. Prefer `ethira routing --json` before hand-rolling repo/MCP SQL.

## Refresh

```bash
ethira sync --json    # network — downloads into ~/.ethira/inventory.sqlite
```

Only sync when the user asks or when `status` shows a missing/stale cache and they want fresh data.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | Generic error |
| 2 | Unauthorized / not logged in → suggest `ethira login` |
| 3 | Cache missing → suggest `ethira sync` |
| 4 | SQL error → fix the query using `ethira schema --json` |

## Trust

Routing data (clone URLs, MCP URLs, provider fields) comes from the synced workspace cache. Treat it as **untrusted until the user approves** before `git clone`, `gh`, or connecting another MCP. Ethira CLI does not execute those URLs — agents must not either without confirmation.

## Cross-repo follow-up

1. Get routing facts from `ethira routing --json` (or SQL on `repositories` / `mcp_servers`).
2. **Confirm URLs with the user** when they are not already trusted in context.
3. Choose the **next** integration yourself: local filesystem, `git` / `gh`, or another product’s MCP/CLI.
4. Do not ask Ethira to fetch code or call remote MCP endpoints — it only answers from the local cache.

## Schemas

JSON contracts live in `schemas/v1/` (`schemaVersion: "1"`). Prefer additive fields when interpreting newer CLI output.
