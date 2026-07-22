# Security policy

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security problems involving
authentication, tokens, or local credential/cache handling.

Prefer **GitHub Private Vulnerability Reporting** on this repository
(Security → Report a vulnerability).

If private reporting is unavailable, email **hello@ethira.dev** with:

- A description of the issue and impact
- Steps to reproduce (or a proof of concept)
- Affected CLI version / commit when known

We aim to acknowledge reports within **3 business days**.

**Never paste** `~/.ethira/**` contents, OAuth tokens, or full inventory
`raw_json` rows into public issues or pull requests.

## Supported versions

- Latest released tag: security fixes
- `main` / default branch: best-effort

## Threat model (summary)

| Scenario | What the CLI assumes |
| --- | --- |
| Stolen laptop / shared UNIX account | Access/refresh tokens live in the OS keychain when available. `~/.ethira/credentials.json` and `inventory.sqlite` are sensitive (owner-only permissions on Unix). |
| Malicious API host (`--api-url`) | Non-default remote hosts require `--allow-untrusted-api-host`. Tokens are sent to whatever host you authorize. |
| Hostile or compromised workspace inventory | Clone URLs and MCP URLs from sync are **untrusted data**. Agents must get user approval before cloning or connecting. |
| Local `ethira sql` | Trusted local user/agent feature. It is not a multi-tenant sandbox against someone who can already run the binary. |
| Supply chain | Dependencies come from crates.io. Release archives ship with `SHA256SUMS`, a CycloneDX SBOM, and GitHub artifact attestations — see [docs/releasing.md](docs/releasing.md). |

### Local data sensitivity

Treat `~/.ethira/inventory.sqlite` as **credential-equivalent**. Sync may store
classifications, PII *flags*, routing URLs, and other inventory metadata. The
API is designed to omit secrets and email addresses from sync payloads; verify
server-side guarantees [docs/api-coordination.md](docs/api-coordination.md)
before relying on that claim.

### OAuth scopes

The CLI currently requests `mcp:tools mcp:resources offline_access`. Until the
API offers a narrower CLI/inventory scope, a stolen refresh token’s server-side
power may exceed “download inventory.” See [docs/api-coordination.md](docs/api-coordination.md).

### Related server guarantees

Server-side DCR policy, payload guarantees, rate limits, and URL validation are
tracked in [docs/api-coordination.md](docs/api-coordination.md).

## Maintainers: public-repo checklist

Before or when making this repository public:

1. Enable **secret scanning** and **push protection**
2. Enable **private vulnerability reporting**
3. Confirm GitHub Actions has no org secrets required by public forks
4. Keep the Gate C release pipeline intact (checksums, attestations/SBOM on
   every `v*` tag). macOS notarization and Windows Authenticode remain
   deferred — see [docs/releasing.md](docs/releasing.md)
