---
title: Development
description: Build from source, contribute, and explore the repo layout.
order: 10
---

## Build

```bash
git clone https://github.com/ethira-dev/cli.git
cd cli
cargo build --release
cargo test
```

Requirements: Rust 1.85+ (edition 2024; see `rust-toolchain.toml`).

## Contributing

See [CONTRIBUTING.md](https://github.com/ethira-dev/cli/blob/main/CONTRIBUTING.md) (DCO required) and [CODE_OF_CONDUCT.md](https://github.com/ethira-dev/cli/blob/main/CODE_OF_CONDUCT.md).

## Useful paths

| Path | Role |
| --- | --- |
| `AGENTS.md` | Agent-facing inventory rules |
| `skills/ethira-inventory/` | Installable agent skill (`npx skills add ethira-dev/cli`) |
| `schemas/v1/` | Versioned JSON contracts (`schemaVersion: "1"`) |
| `docs/api-coordination.md` | API-side trust checklist (maintainers) |
| `docs/releasing.md` | Signed/attested release pipeline (Gate C) |
| `site/` | This marketing + docs site |

## Site (this package)

```bash
cd site
npm install
npm run dev    # http://localhost:4321/cli/
npm run build  # → site/dist
```

Or from the repo root with Just: `just site-dev` / `just site-build`.
