---
title: SQL
description: Sandboxed SELECT/WITH queries over the local inventory cache.
order: 5
---

```bash
ethira sql 'SELECT name FROM systems LIMIT 10'
ethira sql --json 'SELECT name FROM systems LIMIT 10'
ethira sql --sync --json 'SELECT kind, count(*) AS n FROM relationships GROUP BY kind'
```

Without `--sync`, queries are **local-only** against `~/.ethira/inventory.sqlite`. With `--sync`, the CLI refreshes the cache first (network).

## Sandbox

SQL is sandboxed:

- Single-statement `SELECT` / `WITH` only
- Keyword filter + SQLite authorizer with a function allowlist
- Queries are capped (row/time limits)
- No write-back to Ethira

## Examples

```bash
ethira sql 'SELECT name, json_extract(raw_json, "$.criticality") AS criticality FROM incidents LIMIT 10'
ethira sql 'SELECT name, system_id, json_extract(raw_json, "$.cloneUrl") AS clone_url FROM repositories LIMIT 20'
```

Use `json_extract(raw_json, '$.field')` for fields not lifted into columns. Prefer `ethira schema --json` and `ethira tables --json` to discover the schema.
