# truST PLC Developer Guide

This guide is for automation engineers and developers building and deploying PLC project folders.
It assumes you already have the runtime installed.

## Project Layout (Structure)

A PLC project folder contains:

```
runtime.toml
io.toml
program.stbc
sources/
```

- `runtime.toml`: runtime configuration (tasks, control, web, watchdog, retain).
- `io.toml`: I/O driver config and safe-state outputs.
- `program.stbc`: compiled bytecode.
- `sources/`: Structured Text sources.

## Config Paths + Apply Semantics

Runtime reads configuration from these canonical paths:

- Project runtime config: `<project-folder>/runtime.toml` (required).
- Project I/O config: `<project-folder>/io.toml` (optional).
- System I/O fallback if project `io.toml` is missing:
  - Linux/macOS: `/etc/trust/io.toml`
  - Windows: `C:\ProgramData\truST\io.toml`

Apply/restart behavior:

- Offline edits to `runtime.toml` and `io.toml` are loaded on next runtime start/restart.
- `trust-runtime validate --project <project-folder>` validates both files against the canonical schema (required keys, types/ranges, unknown-key policy).
- Browser UI and deploy preflight use the same schema checks before writing/applying config.
- `config.set` updates running settings in memory and returns `restart_required` keys when a restart is needed to apply the change surface (web/discovery/mesh/control mode/retain mode).

## Build Flow

Compile sources into bytecode:
```
trust-runtime build --project <project-folder>
```

Validate a project folder (config + bytecode):
```
trust-runtime validate --project <project-folder>
```

Generate API docs from tagged ST comments (`@brief`, `@param`, `@return`):
```
trust-runtime docs --project <project-folder> --format both --out-dir <project-folder>/docs/api
```

PLCopen XML interchange (strict ST subset profile):
```
trust-runtime plcopen profile
trust-runtime plcopen export --project <project-folder> --output <project-folder>/interop/plcopen.xml
trust-runtime plcopen import --input <plcopen.xml> --project <target-project-folder>
```

Import writes migrated sources to `sources/` and a migration report to:

`<project-folder>/interop/plcopen-migration-report.json`

The report includes detected vendor ecosystem, discovered/imported/skipped POU
counts, source coverage, semantic-loss score, and per-POU skip reasons.

Start runtime:
```
trust-runtime --project <project-folder>
```

## Runtime Configuration (runtime.toml)

Key sections:

- `[resource]`: name + cycle time.
- `[runtime.control]`: control endpoint + debug gating.
- `[runtime.web]`: browser UI.
- `[runtime.discovery]`: local mDNS.
- `[runtime.mesh]`: runtime-to-runtime sharing.
- `[runtime.retain]`: retain store.
- `[runtime.watchdog]`: fault policy + safe halt.
- `simulation.toml`: simulation couplings, delays, and scripted disturbances/fault injection.

## I/O Configuration (io.toml)

See `docs/guides/PLC_IO_BINDING_GUIDE.md` for full examples.

Supported `io.driver` values include `loopback`, `simulated`, `gpio`, `modbus-tcp`, and `mqtt`.

## Browser UI (Operations)

If enabled:
```
runtime.web.enabled = true
runtime.web.listen = "0.0.0.0:8080"
```

Open:
```
http://<device-ip>:8080
```

Operations UI:
- `http://<device-ip>:8080` for status, I/O, settings, deploy.
- `http://<device-ip>:8080/hmi` for auto-generated read-only HMI.

Dedicated HMI control API (via `POST /api/control`):
- `hmi.schema.get`
- `hmi.values.get`
- `hmi.write` (currently disabled in read-only mode)

## Debug Attach (Development)

Debug is off in production mode by default. For development:
```
runtime.control.mode = "debug"
runtime.control.debug_enabled = true
```

Use the VS Code extension or `trust-runtime ctl` for stepping and breakpoints.

## Deploy + Rollback

Deploy a project folder into a versioned store:
```
trust-runtime deploy --project <project-folder> --root <deploy-root>
```

Rollback:
```
trust-runtime rollback --root <deploy-root>
```

## Local Discovery + Mesh

Enable local discovery:
```
runtime.discovery.enabled = true
```

Enable mesh sharing:
```
runtime.mesh.enabled = true
runtime.mesh.publish = ["Status.PLCState"]
[runtime.mesh.subscribe]
"RemoteA:Status.PLCState" = "Local.Status.RemoteState"
```

## Testing

Recommended checks: run the runtime reliability and GPIO hardware checklists before deployment.

For CI/CD pipelines and stable machine-readable outputs, see:

`docs/guides/PLC_CI_CD.md`

For simulation-first workflows, see:

`docs/guides/PLC_SIMULATION_WORKFLOW.md`
