---
title: Sync & cache
description: One-shot inventory sync into a local SQLite cache.
order: 4
---

```bash
ethira sync [--json] [--clear]
ethira status [--json]
ethira schema [--json]
ethira tables [--json]
```

## Local cache

Path: `~/.ethira/inventory.sqlite` (directory `0700`, file `0600` on Unix; owner-restricted ACL on Windows)

- Written by `ethira sync` (also stores `last_synced_at` / workspace in `ethira_meta`)
- Read by `status` / `schema` / `tables` / `routing` / `sql`
- Cleared by `ethira logout` or `ethira sync --clear`

## Sensitive data

Sync materializes inventory metadata (tags, CIA, PII *flags*, classifications, vendor metadata, routing URLs) locally. The API is **designed** to omit secrets and email addresses from sync payloads; treat the cache like credentials regardless.

See [SECURITY.md](https://github.com/ethira-dev/cli/blob/main/SECURITY.md) in the repository.

## Network vs local

| Command | Network? |
| --- | --- |
| `login`, `whoami`, `sync`, `sql --sync`, `workspace` | Yes |
| `status`, `schema`, `tables`, `routing`, `sql` (without `--sync`) | Local-only |
