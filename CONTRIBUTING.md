# Contributing to Ethira CLI

Thanks for contributing. This repository is the open-source **Ethira CLI**
client. The Ethira API / inventory service is a separate hosted product.

## Development

Requirements: Rust 1.85+ (see `rust-toolchain.toml`).

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Do not add CI jobs that need live Ethira OAuth credentials. Keep tests
offline and synthetic.

## Developer Certificate of Origin (DCO)

This project uses the [Developer Certificate of Origin](https://developercertificate.org/).
Every commit must include a sign-off:

```bash
git commit -s -m "Your message"
```

That adds a `Signed-off-by:` line certifying you have the right to submit the
contribution under the project’s dual license (Apache-2.0 OR MIT).

## Compatibility (`--json` / schemas)

Machine-readable stdout is versioned under [`schemas/v1/`](schemas/v1/README.md)
with `"schemaVersion": "1"`.

- **Additive** JSON fields within v1 are OK.
- Removing/renaming fields or changing exit-code meanings requires a new schema
  major (`schemas/v2/`) and release notes.
- While the CLI is `0.x`, breaking human/agent behavior may land in minor
  bumps — call them out clearly in the PR and changelog.
- Update schema JSON files in the **same PR** as code that changes `--json`
  output.

SQL table columns can evolve with inventory; the stable contract is the JSON
command envelopes, not every SQLite column.

## Security

See [SECURITY.md](SECURITY.md). Never paste tokens, `~/.ethira` dumps, or full
inventory `raw_json` into public issues or PRs.

## Pull requests

Use the PR template checklist. Keep changes focused; match existing style.
