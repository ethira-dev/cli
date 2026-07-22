---
title: Security
description: Token storage, OAuth scopes, SQL sandbox, and vulnerability reporting.
order: 9
---

## Reporting

Report vulnerabilities privately — see [SECURITY.md](https://github.com/ethira-dev/cli/blob/main/SECURITY.md).

## Highlights

- **Tokens** live in the OS keychain when available; never printed to the terminal
- **OAuth scopes:** `mcp:tools mcp:resources offline_access` (platform OAuth resource for CLI inventory auth — **not** an agent inventory interface). A stolen refresh token may authorize more than “download inventory” until a narrower CLI scope exists
- **HTTP clients** use connect/request timeouts; unreachable APIs fail rather than hang forever
- **SQL** is sandboxed (SELECT/WITH only, authorizer, row/time caps); no write-back
- **Cache** holds inventory metadata — treat `~/.ethira/inventory.sqlite` like credentials
- **Routing URLs** from sync are untrusted until the user approves them

Licensed under **Apache-2.0 OR MIT**. See [TRADEMARKS.md](https://github.com/ethira-dev/cli/blob/main/TRADEMARKS.md) for brand guidelines.
