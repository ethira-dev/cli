---
title: Auth & credentials
description: OAuth PKCE login, keychain tokens, and workspace selection.
order: 3
---

## Login

```bash
ethira login
```

Opens a browser for OAuth (PKCE S256) with a loopback callback. You pick a workspace; the CLI offers a first sync when the local cache is empty.

```bash
ethira whoami [--json]
ethira workspace
ethira logout
```

Sync always uses the **OAuth token workspace**. `ethira workspace` changes the local shell selection only. Re-run `ethira login` to consent for another token workspace.

## OAuth endpoints

- DCR: `POST /v1/oauth/register`
- Authorize: `GET /v1/oauth/authorize` (PKCE S256, loopback callback, 5-minute timeout)
- Token: `POST /v1/oauth/token` (authorization code + refresh)
- Revoke: `POST /v1/oauth/revoke` (best-effort on logout)

Expired access tokens are refreshed automatically when a refresh token is present.

## Credentials

- **Tokens:** stored in the OS keychain (`ethira-cli` service) when available (fail closed if keychain is unavailable, unless `ETHIRA_ALLOW_PLAINTEXT_TOKENS` is set)
- **Metadata:** `~/.ethira/credentials.json` (`0600`, written atomically) — API URL, client id, expiry, shell workspace
- Legacy plaintext tokens in `credentials.json` are migrated into the keychain on load
- Tokens are never printed to the terminal
- `ethira logout` best-effort revokes the refresh/access token server-side, then deletes local credentials and cache
