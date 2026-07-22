# Releasing (Gate C)

Tag a release (`v*`) to build signed/attested binaries, update Homebrew, and
keep the verified install script working.

## Tooling already in-repo

- [`rust-toolchain.toml`](../rust-toolchain.toml) — pinned stable toolchain
- [`deny.toml`](../deny.toml) — license + advisory policy (`cargo deny check`)
- CI multi-OS test matrix — see [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)
- Release workflow — [`.github/workflows/release.yml`](../.github/workflows/release.yml)
- Installer — [`scripts/install.sh`](../scripts/install.sh)
- Homebrew bump — [`scripts/bump-homebrew-formula.sh`](../scripts/bump-homebrew-formula.sh)

## What a tag publishes

1. Archives: `ethira-<target>.tar.gz` for
   `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`
2. **`SHA256SUMS`** attached to the GitHub Release
3. **GitHub artifact attestations** (`actions/attest-build-provenance`)
4. **CycloneDX SBOM** (`ethira.cdx.json` via `cargo cyclonedx`)
5. Homebrew formula bump in [`ethira-dev/homebrew-tap`](https://github.com/ethira-dev/homebrew-tap)
   (`Formula/ethira.rb`) when `HOMEBREW_TAP_TOKEN` is set

## How to cut a release

```bash
# on main, clean tree
git tag v0.1.0
git push origin v0.1.0
```

Watch the **Release** workflow. After it finishes:

```bash
brew install ethira-dev/tap/ethira
# or
curl -fsSL https://raw.githubusercontent.com/ethira-dev/cli/main/scripts/install.sh | sh
```

### Secrets

| Secret | Where | Purpose |
|--------|--------|---------|
| `HOMEBREW_TAP_TOKEN` | `ethira-dev/cli` (and `ethira-dev/mizpah`) | PAT or fine-grained token with `contents:write` on `ethira-dev/homebrew-tap` |

Without the secret, the release still publishes GitHub assets; the tap bump job skips with a warning. Bump manually:

```bash
# after downloading release archives into ./dist
VERSION=0.1.0 HOMEBREW_TAP_TOKEN="$(gh auth token)" DIST_DIR=dist \
  ./scripts/bump-homebrew-formula.sh
```

## Still deferred

- **macOS notarization** when shipping beyond ad-hoc signed binaries
- **Windows Authenticode** / Windows release archives
- **crates.io** (`publish = false` in `Cargo.toml` until intentional)

## Rules

- Never commit release binaries to git
- No `curl | sh` install without checksum verification (`scripts/install.sh` checks `SHA256SUMS`)
- If auto-update is added later: signed artifacts only, verify before replace
