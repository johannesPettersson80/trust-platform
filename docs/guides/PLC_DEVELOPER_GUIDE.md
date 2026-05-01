# truST PLC Developer Guide

Build and deploy PLC project folders.
It assumes you already have the runtime installed.

## Project Layout (Structure)

A PLC project folder typically contains:

```
runtime.toml
io.toml
program.stbc
trust-lsp.toml
src/
```

- `runtime.toml`: runtime configuration (tasks, control, web, watchdog, retain).
- `io.toml`: I/O driver config and safe-state outputs.
- `program.stbc`: compiled bytecode.
- `trust-lsp.toml`: optional project config for include paths, package dependencies, vendor profile, and runtime-assisted editor features.
- `src/`: project-owned Structured Text sources.

## Reusable Libraries

Project-owned Structured Text belongs in `<project>/src/`.

Reusable truST libraries should live in their own package directory. In this
repo the convention is `libraries/<name>/`; in user projects any separate
package directory is fine as long as consuming projects point to it through
`[dependencies]`.

Example layout:

```text
my-project/
  runtime.toml
  io.toml
  trust-lsp.toml
  src/

libraries/
  my_motion_lib/
    trust-lsp.toml
    src/
```

A reusable library package should contain:

- `trust-lsp.toml` with `[package].version` and `[project].include_paths = ["src"]`
- `src/` with the library Structured Text sources

Consumers reference reusable packages from their own `trust-lsp.toml`:

```toml
[dependencies]
MyMotionLib = { path = "../libraries/my_motion_lib", version = "0.1.0" }
```

`trust-runtime build --project ...` and `trust-dev test --project ...`
compile the project's own `src/` plus any local `[dependencies]` packages.

Use `[[libraries]]` for external/index-only library trees, vendor stub packs,
or attached documentation packs. Do not place reusable libraries under
`crates/.../tests/fixtures/`; that path is test-only repo infrastructure.

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

### Host-Specific Performance Builds

Use the portable release build when the binary needs to run across different CPU families or when you are preparing a shared/distributed artifact:

```bash
cargo build --release -p trust-runtime --bin trust-runtime
```

If you control the deployment hardware and want maximum performance on that machine class, build a host-native binary:

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release -p trust-runtime --bin trust-runtime
```

That host-native binary may not run on other CPUs. It is appropriate for self-built deployments to a homogeneous hardware target and for local performance tuning, but not for portable/shared release artifacts.

The benchmark scripts support the same build-mode policy so you can validate the exact build style you intend to ship:

- `TRUST_RUNTIME_HOST_CODEGEN=auto`: default; picks `native` on Raspberry Pi benchmark hosts and `generic` elsewhere
- `TRUST_RUNTIME_HOST_CODEGEN=generic`: force portable benchmark builds
- `TRUST_RUNTIME_HOST_CODEGEN=native`: force host-native benchmark builds

For a host-native validation run on the current machine, use one of the shipped benchmark surfaces:

```bash
TRUST_RUNTIME_HOST_CODEGEN=native ./scripts/runtime_motion_example_bench_gate.sh
```

For the broader shipped motion breakdown:

```bash
TRUST_RUNTIME_HOST_CODEGEN=native ./scripts/runtime_motion_benchmark_breakdown.sh
```

### Profile-Guided Optimization (PGO)

When you want the fastest self-built runtime for one machine class, use PGO on top of the host-native build. This is not a portable release workflow.

1. Install the LLVM merge tool bundled for Rust:

```bash
rustup component add llvm-tools-preview
```

2. Collect training data with representative workloads on the target machine:

```bash
PGO_ROOT="$PWD/target/pgo/runtime-motion-native"
rm -rf "$PGO_ROOT" target/gate-artifacts/runtime-motion-pgo-gen*
mkdir -p "$PGO_ROOT/raw"
OUT_DIR=target/gate-artifacts/runtime-motion-pgo-gen-gate \
TRUST_RUNTIME_HOST_CODEGEN=native \
RUSTFLAGS="-Cprofile-generate=$PGO_ROOT/raw" \
./scripts/runtime_motion_example_bench_gate.sh
OUT_DIR=target/gate-artifacts/runtime-motion-pgo-gen-breakdown \
TRUST_RUNTIME_HOST_CODEGEN=native \
RUSTFLAGS="-Cprofile-generate=$PGO_ROOT/raw" \
./scripts/runtime_motion_benchmark_breakdown.sh
```

3. Merge the raw profiles:

```bash
SYSROOT=$(rustc --print sysroot)
LLVM_PROFDATA=$(find "$SYSROOT" -path '*/bin/llvm-profdata' -type f | head -n 1)
"$LLVM_PROFDATA" merge -output="$PGO_ROOT/merged.profdata" "$PGO_ROOT"/raw/*.profraw
```

4. Rebuild and rerun the same validation surfaces with the merged profile:

```bash
OUT_DIR=target/gate-artifacts/runtime-motion-pgo-gate \
TRUST_RUNTIME_HOST_CODEGEN=native \
RUSTFLAGS="-Cprofile-use=$PGO_ROOT/merged.profdata" \
./scripts/runtime_motion_example_bench_gate.sh
OUT_DIR=target/gate-artifacts/runtime-motion-pgo-breakdown \
TRUST_RUNTIME_HOST_CODEGEN=native \
RUSTFLAGS="-Cprofile-use=$PGO_ROOT/merged.profdata" \
./scripts/runtime_motion_benchmark_breakdown.sh
```

Current Raspberry Pi 5 evidence on the `full_demo` motion path:

- portable baseline `full_demo p50`: `433.501 us`
- host-native `full_demo p50`: `404.353 us`
- host-native + PGO `full_demo p50`: `292.298 us`

Generate API docs from tagged ST comments (`@brief`, `@param`, `@return`):
```
trust-dev docs --project <project-folder> --format both --out-dir <project-folder>/docs/api
```

PLCopen XML interchange (strict ST subset profile):
```
trust-runtime plcopen profile
trust-runtime plcopen export --project <project-folder> --output <project-folder>/interop/plcopen.xml
trust-runtime plcopen export --project <project-folder> --output <project-folder>/interop/plcopen.xml --json
trust-runtime plcopen export --project <project-folder> --target ab --json
trust-runtime plcopen export --project <project-folder> --target siemens --json
trust-runtime plcopen export --project <project-folder> --target schneider --json
trust-runtime plcopen import --input <plcopen.xml> --project <target-project-folder>
trust-runtime plcopen import --input <plcopen.xml> --project <target-project-folder> --json
```

Import writes migrated sources to `src/` and a migration report to:

`<project-folder>/interop/plcopen-migration-report.json`

The report includes detected vendor ecosystem, discovered/imported/skipped POU
counts, source coverage, semantic-loss score, compatibility coverage summary,
structured unsupported-node diagnostics, applied vendor-library shims, and
per-POU skip reasons.

For compatibility matrix, round-trip limits, and known gaps, see:

`docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md`

For multi-vendor export adapter manual steps/limitations, see:

`docs/guides/PLCOPEN_EXPORT_ADAPTERS_V1.md`

For direct Siemens `.scl` export/import tutorial (TIA External source files path), see:

`docs/guides/SIEMENS_TIA_SCL_IMPORT_TUTORIAL.md`

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
- `[runtime.observability]`: historian sampling + Prometheus export.
- `[runtime.retain]`: retain store.
- `[runtime.watchdog]`: fault policy + safe halt.
- `simulation.toml`: simulation couplings, delays, and scripted disturbances/fault injection.

## I/O Configuration (io.toml)

See `docs/guides/PLC_IO_BINDING_GUIDE.md` for full examples.

Supported I/O backends are `loopback`, `simulated`, `gpio`, `modbus-tcp`, `mqtt`, and `ethercat`.

`io.toml` supports:

- single-driver form: `io.driver` + `io.params`
- multi-driver form: `io.drivers = [{ name = \"...\", params = {...} }, ...]`

Use one form at a time (do not mix `io.driver` with `io.drivers`).

For EtherCAT backend scope and setup details, see:
`docs/guides/ETHERCAT_BACKEND_V1.md`.

For protocol-commissioning example projects (including GPIO and composed
multi-driver setup), see:
`examples/communication/README.md`.

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
- `hmi.write` (phase-gated: enabled only when `[write].enabled = true` in `hmi.toml` and target is explicitly allowlisted)

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
