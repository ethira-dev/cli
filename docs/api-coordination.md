# API coordination checklist

Work tracked here lives on the **Ethira API** side, not in this CLI repository.
Public CLI adoption amplifies these trust and abuse surfaces.

| Item | Status | Notes |
| --- | --- | --- |
| DCR / public-client policy appropriate for an open CLI | [ ] | Loopback redirect URIs (`http://127.0.0.1`) must be allowed for the Ethira CLI |
| Inventory-only OAuth scope for the CLI | [ ] | CLI currently uses `mcp:tools mcp:resources offline_access`; do not change the client scope string until the API supports a narrower scope |
| Sync payloads never include secrets or email addresses | [ ] | Backs README / SECURITY wording; treat cache as sensitive regardless |
| Rate limits for public OAuth clients | [ ] | Protect register / token / inventory endpoints |
| Refresh-token revoke behavior | [ ] | CLI best-effort calls `/v1/oauth/revoke` on logout |
| Server-side validation of MCP / clone URLs in inventory | [ ] | Routing URLs are untrusted client-side until user approval |

## CLI acceptance criteria

When a dedicated inventory/CLI scope exists and is documented:

1. Update `DEFAULT_SCOPE` in `src/auth/mod.rs`
2. Update README OAuth section and SECURITY.md
3. Keep login working against production before releasing

Owner: API team. CLI maintainers update the client when items land.
