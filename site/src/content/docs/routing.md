---
title: Routing
description: Repositories and MCP servers with URLs — a directory, not a proxy.
order: 6
---

```bash
ethira routing --json
```

Projects repository and MCP-server rows with best-effort URL / transport fields from `raw_json` (`cloneUrl`, `gitUrl`, `url`, `htmlUrl`, `defaultBranch`, `provider`, …).

## Trust

Treat those URLs as **untrusted until the user approves** them before `git clone`, `gh`, or connecting another MCP. Ethira does not proxy those integrations.

## Typical flow

1. Query inventory with `ethira sql` / `ethira routing`
2. Open the matching git checkout or call another product’s CLI/MCP as indicated
3. Confirm clone/MCP URLs with the user before acting on them
