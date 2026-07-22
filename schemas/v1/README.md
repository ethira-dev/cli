# Ethira CLI JSON schemas (v1)

Machine-readable contracts for `ethira … --json` stdout. Agents and scripts should treat these as the stable wire format.

| File | Command |
| --- | --- |
| [`status.json`](status.json) | `ethira status --json` |
| [`schema.json`](schema.json) | `ethira schema --json` |
| [`routing.json`](routing.json) | `ethira routing --json` |
| [`tables.json`](tables.json) | `ethira tables --json` |
| [`sql.json`](sql.json) | `ethira sql --json '…'` |
| [`sync.json`](sync.json) | `ethira sync --json` |
| [`whoami.json`](whoami.json) | `ethira whoami --json` |
| [`error.json`](error.json) | stderr JSON on failure when `--json` is set |

All success payloads include `"schemaVersion": "1"`. Prefer additive changes within a major schema version.

## Compatibility

- `schemaVersion: "1"` is the wire **major**. Additive fields within v1 are OK.
- Removing or renaming fields, or changing exit-code meanings, requires a new
  schema major (`schemas/v2/`) plus CLI release notes.
- CLI SemVer: while the package is `0.x`, breaking human/agent behavior may land
  in minor bumps — document them clearly (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).
- Update schema JSON files in the same PR as code that alters `--json` output.
- SQL table columns can evolve with inventory; the stable contract is these JSON
  command envelopes (and `ethira schema`), not every SQLite column forever.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | Generic error |
| 2 | Unauthorized / not logged in |
| 3 | Local inventory cache missing |
| 4 | SQL rejected or failed |

## Local-only guarantee

After `ethira sync`, these commands never call the Ethira API: `status`, `schema`, `tables`, `routing`, `sql` (without `--sync`). Do not use Ethira MCP for inventory — invoke this CLI via the shell.
