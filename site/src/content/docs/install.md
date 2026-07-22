---
title: Install
description: Install Ethira CLI via Homebrew, a verified install script, or from source.
order: 2
---

## Requirements

- Network access to Ethira (`https://api.ethira.dev` by default) for login/sync only
- Your Ethira workspace must allow CLI OAuth login with loopback redirect URIs (`http://127.0.0.1`)
- API inventory sync routes: `GET /v1/cli/inventory/*`
- From source: Rust 1.85+ (edition 2024; see `rust-toolchain.toml` in the repo)

## Homebrew

```bash
brew install ethira-dev/tap/ethira
ethira --version
```

Works on macOS and Linux (Homebrew on Linux).

## Linux / macOS install script

Verified installer: downloads a GitHub Release archive and checks `SHA256SUMS` before install.

```bash
curl -fsSL https://raw.githubusercontent.com/ethira-dev/cli/main/scripts/install.sh | sh
```

Optional env:

- `ETHIRA_VERSION=0.1.0` — pin a version (with or without leading `v`)
- `ETHIRA_INSTALL_DIR=~/.local/bin` — install location (default)

## GitHub Releases

Prebuilt archives: [Releases](https://github.com/ethira-dev/cli/releases) (`ethira-<target>.tar.gz`).

| Platform | Archive |
|----------|---------|
| macOS Apple Silicon | `ethira-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `ethira-x86_64-apple-darwin.tar.gz` |
| Linux x86_64 | `ethira-x86_64-unknown-linux-gnu.tar.gz` |

Each release includes `SHA256SUMS`, a CycloneDX SBOM (`ethira.cdx.json`), and GitHub artifact attestations.

### Verify checksum

```bash
curl -fsSLO https://github.com/ethira-dev/cli/releases/latest/download/SHA256SUMS
curl -fsSLO https://github.com/ethira-dev/cli/releases/latest/download/ethira-aarch64-apple-darwin.tar.gz
shasum -a 256 -c SHA256SUMS --ignore-missing
```

### Verify attestation

```bash
gh attestation verify ethira-aarch64-apple-darwin.tar.gz --repo ethira-dev/cli
```

## From source

```bash
git clone https://github.com/ethira-dev/cli.git
cd cli
cargo install --path .
```

Binary: `~/.cargo/bin/ethira` (or `target/release/ethira` after `cargo build --release`).

## Agent skill

Teach Cursor / Claude Code / other agents to query inventory via this CLI:

```bash
npx skills add ethira-dev/cli
# optional: global for all projects
npx skills add ethira-dev/cli -g
```

Requires `ethira` on `PATH`. See [Agents](./agents.md).

## Completions

```bash
# zsh (user-local example)
ethira completions zsh > ~/.zfunc/_ethira

# bash (user-local example)
mkdir -p ~/.local/share/bash-completion/completions
ethira completions bash > ~/.local/share/bash-completion/completions/ethira

# fish
ethira completions fish > ~/.config/fish/completions/ethira.fish
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.
