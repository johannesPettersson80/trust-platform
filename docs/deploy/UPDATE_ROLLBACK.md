# Update & Rollback Flow

This document describes a production‑safe update strategy for truST.

## Goals
- Atomic swap to new project version.
- Fast rollback to last known good project.
- Preserve retain data when desired.

## Recommended Layout

```
/opt/trust/
  bundles/                 # internal storage for project versions
    project-2026-01-26_1200/
    project-2026-01-26_1300/
  current -> /opt/trust/bundles/project-2026-01-26_1300
```

## Update Procedure (Warm)

1. Build a new project folder with `trust-runtime build --project /path/to/project`:
   - `program.tpi` and `deployment-record.bin`
   - `runtime.toml`
   - `io.toml`
2. Deploy with versioning (keeps last two known‑good projects):
   - `trust-runtime deploy --project /path/to/project --root /opt/trust`
3. Restart runtime:
   - `trust-runtime ctl --project /opt/trust/current restart warm`
   - or `trust-runtime deploy --project /path/to/project --root /opt/trust --restart warm`

## Rollback Procedure

1. Roll back project pointer:
   - `trust-runtime rollback --root /opt/trust`
2. Restart runtime:
   - `trust-runtime ctl --project /opt/trust/current restart warm`

## Cold Start Updates

If the update requires a full cold start (schema changes, non‑compatible retain data):

```
trust-runtime ctl --project /opt/trust/current restart cold
```

## Notes
- Project versioning is enforced by `bundle.version` (internal).
- Store retain data outside the project folder for safe rollback.
- Use OS supervision (systemd) to ensure restarts are reliable.
