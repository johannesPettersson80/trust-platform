# Rust PLC Architecture Ownership Map

**Status:** ownership contract, v1 (2026-07-03).
**Master:** `rust-support-architecture-spec-v1.md`.
**Applies to:** runtime/SDK/CLI/IDE boundaries, module splits, diagram updates,
future implementation reviews.

## 1. Rule

Each subsystem owns one reason to change. Transport, business logic, schema
models, rendering, and runtime execution are separated. New Rust PLC work must
not be appended to existing large files when a new per-responsibility module is
the cleaner ownership boundary.

## 2. Subsystems

| Subsystem | Owns | Does not own |
|---|---|---|
| `trust-module-abi` | ABI structs, descriptors, status enums | host policy, codegen, UI |
| `trust-plc` | user SDK traits/API, safe sequence API | runtime scheduling, CLI parsing |
| `trust-plc-macros` | proc macros, validators, span diagnostics | runtime execution |
| runtime module host | discovery, handshake, call, fault boundary, telemetry | SDK public API, VS Code rendering |
| codegen engine | Rust/trust.toml -> generated artifacts | admission math, UI diff rendering |
| digest engine | manifest, hash, F22 drift | code formatting, editor UI |
| admission engine | timing/crate verdicts | VS Code chart rendering |
| `trust` CLI | orchestration and JSON output | admission internals, editor state |
| `trust-sim` | machine test facade | production runtime control |
| debug/control pipeline | live snapshots, writes, forcing, reset | CLI JSON reports |
| VS Code project model | project kind, visible actions | runtime decisions |
| VS Code reports | render JSON/control payloads | recompute admission or digests |

## 3. Proposed Module Layout

Runtime-side new modules are examples; final names may change but ownership
must stay comparable:

```text
crates/
  trust-module-abi/
  trust-plc/
  trust-plc-macros/
  trust-sim/
  trust-service-sdk/
  trust-runtime/src/
    module_host/
      mod.rs
      registry.rs
      loader.rs
      handshake.rs
      call.rs
      faults.rs
      retain.rs
      telemetry.rs
    rust_codegen/
      mod.rs
      manifest.rs
      declarations.rs
      configuration.rs
      digest.rs
      location_map.rs
    admission/
      timing.rs
      crates.rs
      records.rs
      report.rs
    replay/
      trace.rs
      compare.rs
      explain.rs
editors/vscode/src/
  projectKind.ts
  rustProject/
    scaffold.ts
    checkModel.ts
    admissionModel.ts
    admissionPanel.ts
    generatedSt.ts
    machineTests.ts
    traces.ts
  io-panel/
    instances.ts
```

Do not add broad Rust PLC logic directly to `ioPanel.ts`, `trustHomeView.ts`,
control server handlers, or CLI command files when a model/module split is
available.

## 4. Data Flow

### Build/check flow

```text
Rust source + trust.toml
  -> SDK macro metadata
  -> codegen engine
  -> generated ST + runtime/io config + manifest
  -> digest check
  -> cargo/ST compile
  -> timing/crate admission
  -> trust check --json
  -> VS Code Problems/report
```

### Runtime flow

```text
task scheduler
  -> module host call boundary
  -> copy-in exchange
  -> catch_unwind/deadline/accounting
  -> copy-out or fault policy
  -> metrics/fault/live snapshot
  -> debug/control pipeline
  -> VS Code Live Values
```

### Replay flow

```text
.trusttrace
  -> trust replay --json
  -> diff/explain model
  -> CLI/CI result
  -> VS Code Testing/report
```

## 5. Interface Rules

- Runtime internals expose typed APIs to CLI; CLI emits JSON.
- VS Code consumes JSON/control snapshots; it does not call admission internals.
- `trust-sim` drives public harness APIs; tests should not reach through
  private runtime internals unless explicitly marked conformance-only.
- Codegen metadata is consumed by digest, admission, LSP/IDE, and reports
  through stable structs or JSON fixtures.
- New validators live in per-slice modules.

## 6. Diagram Obligations

When implementation starts, add/update diagrams for:

- Rust PLC build/check pipeline;
- module host runtime call path;
- generated artifact and digest chain;
- debug/live-values instance snapshot;
- admission report data flow;
- deploy gate data flow.

Diagram outputs and manifest must be regenerated as required by repo rules.

## 7. Review Questions

Before merging each slice:

- Did any file cross approximately 1k lines because of this work?
- Did any module gain two unrelated reasons to change?
- Does VS Code recompute something owned by runtime/CLI?
- Does runtime parse editor concepts?
- Are generated artifacts and diagnostics produced from one source of truth?
- Are diagrams/checklists updated for ownership or execution-flow changes?
