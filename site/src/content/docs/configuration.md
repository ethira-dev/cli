---
title: Configuration
description: API URL, trust overrides, colors, and quiet mode.
order: 8
---

| Flag / env | Default | Description |
| --- | --- | --- |
| `--api-url` | `https://api.ethira.dev` | API base URL |
| `ETHIRA_API_URL` | same | Same as `--api-url` |
| `--allow-untrusted-api-host` | off | Required for non-default, non-loopback HTTPS API hosts (tokens will be sent there) |
| `ETHIRA_ALLOW_PLAINTEXT_TOKENS` | unset | If `1`/`true`, allow storing tokens in `credentials.json` when the OS keychain is unavailable (loud warning) |
| `ETHIRA_COLOR_BG` | auto | Force `dark` or `light` primary (`#A9FBC0` / `#016853`) |
| `NO_COLOR` | unset | Disable ANSI colors |
| `-q` / `--quiet` | off | Suppress non-essential stderr/stdout chatter |

## API URL policy

`https://` is required. Cleartext `http://` is allowed only for loopback hosts (`localhost`, `127.0.0.1`, `::1`). Non-default remote hosts require `--allow-untrusted-api-host`.

```bash
ETHIRA_API_URL=http://localhost:3000 ethira login
ethira --api-url https://api.example.com --allow-untrusted-api-host login
```

This repository is an open-source **client**. The Ethira API and inventory service are a separate hosted product. The default API host is `https://api.ethira.dev`. Forks and self-hosted setups may point at another HTTPS API with an explicit trust override.
