# Releasing (Gate C)

Source-first install (`cargo build` / `cargo install --path .`) is supported
before this pipeline exists. **Do not** market “download the binary” until the
items below work.

## Tooling already in-repo

- [`rust-toolchain.toml`](../rust-toolchain.toml) — pinned stable toolchain
- [`deny.toml`](../deny.toml) — license + advisory policy (`cargo deny check`)
- CI multi-OS test matrix — see [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)

## Release pipeline checklist

1. **Tag-driven workflow** (GitHub Actions or goreleaser) building
   linux / macOS / windows archives of `ethira`
2. Attach **`SHA256SUMS`** to the GitHub Release
3. Enable **GitHub artifact attestations**
   (`actions/attest-build-provenance`) or cosign/Sigstore
4. Attach a **CycloneDX SBOM** (e.g. `cargo cyclonedx`)
5. Document verification steps in the README install section
6. **macOS notarization** when shipping beyond ad-hoc signed binaries
7. **Windows Authenticode** when shipping `.msi` / installer `.exe`
8. Only then: mention GitHub Releases in README; consider Homebrew / crates.io
   (`publish = false` in `Cargo.toml` until crates.io is intentional)

## Rules

- Never commit release binaries to git
- No `curl | sh` install without checksum (+ signature) verification
- If auto-update is added later: signed artifacts only, verify before replace
