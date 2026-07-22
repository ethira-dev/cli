---
title: Agents
description: Shell out to ethira with --json. No Ethira MCP for inventory.
order: 7
---

Sync once, then query only via this CLI (`ethira … --json`). There is no Ethira MCP for inventory. Query commands do not call the API except `ethira sql --sync`.

## Contract

| Concern | Behavior |
| --- | --- |
| Runtime | Shell out to `ethira`; parse `--json` stdout |
| Not supported | Ethira MCP for inventory |
| Local-only | `status`, `schema`, `tables`, `routing`, `sql` (without `--sync`) |
| Network | `login`, `whoami`, `sync`, `sql --sync`, `workspace` |
| Schemas | [`schemas/v1/`](https://github.com/ethira-dev/cli/tree/main/schemas/v1), field `schemaVersion: "1"` |

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | Generic error |
| 2 | Unauthorized / not logged in |
| 3 | Local inventory cache missing |
| 4 | SQL rejected or failed |

On failure with `--json`, stderr is a small JSON object (`error`, `message`, …).

## Discovery

Install the inventory skill into Cursor / Claude Code / other agents:

```bash
npx skills add ethira-dev/cli
# optional: global for all projects
npx skills add ethira-dev/cli -g
```

- [AGENTS.md](https://github.com/ethira-dev/cli/blob/main/AGENTS.md)
- [`skills/ethira-inventory/SKILL.md`](https://github.com/ethira-dev/cli/blob/main/skills/ethira-inventory/SKILL.md)
- JSON contracts in [`schemas/v1/`](https://github.com/ethira-dev/cli/tree/main/schemas/v1)

## Typical agent flow

```bash
ethira status --json
ethira schema --json
ethira routing --json
ethira tables --json
ethira sql --json 'SELECT name FROM systems LIMIT 20'
```

If the cache is missing (exit code `3`), tell the user to run `ethira sync` (requires prior `ethira login`).
