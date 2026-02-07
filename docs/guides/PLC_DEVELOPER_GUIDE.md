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
