# Rust PLC Bootstrap And Packaging Contract

**Status:** implementation contract, v1 (2026-07-03).
**Master:** `rust-support-architecture-spec-v1.md`.
**Applies to:** first-run, VSIX packaging, `trust` umbrella, SDK crates,
templates, rust-toolchain, rust-analyzer recommendation, version skew.

## 1. Product Rule

The first Rust PLC project must not fail because the user accidentally lacks a
toolchain or has a mismatched SDK. Bootstrap is part of the product, not a
README footnote.

## 2. Bundled And External Components

| Component | Preferred source | Notes |
|---|---|---|
| `trust-lsp` | bundled in VSIX | existing model |
| `trust-debug` | bundled in VSIX | existing model |
| `trust-runtime` | bundled or downloaded with checksum | existing model depends on platform |
| `trust` umbrella | bundled in VSIX or downloaded with checksum | required for Rust PLC UX |
| Rust toolchain | rustup-managed pinned toolchain | user consent and progress required |
| rust-analyzer | VS Code extension recommendation | degraded editing if absent |
| `trust-plc`/`trust-sim` | crates.io exact pins for v1 | exact version must match bundled tool |
| templates | bundled with `trust` | one scaffold truth |

## 3. SDK Distribution Decision

Decision for v1 (master D25): crates.io exact pins.

- scaffold writes `trust-plc = "=x.y.z"` and `trust-sim = "=x.y.z"`;
- versions match the bundled `trust` tool and VSIX manifest;
- first build may need network unless the user's cargo cache already has the
  crates;
- bundled local registry/source is deferred until an offline-first field need
  justifies the packaging cost.

Version skew is refused, not tolerated.

## 4. Preflight States

Before creating or running a Rust PLC project, VS Code computes:

- `trust` binary present and version-compatible;
- cargo present;
- rustup present or pinned toolchain already installed;
- pinned toolchain available or installable;
- SDK resolution available;
- rust-analyzer installed or absent;
- target platform supported;
- templates available.

States:

- `ready`
- `missing_toolchain`
- `missing_trust`
- `sdk_unresolved`
- `version_skew`
- `unsupported_platform`
- `degraded_editor`

Only `ready` creates a project without warning. `degraded_editor` may run but
must state that Rust editing features are reduced.

## 5. User-facing Behavior

- Rust project option remains visible even when gated.
- Selecting a gated option explains the missing prerequisite.
- The user can cancel without filesystem writes.
- Toolchain downloads require consent and progress.
- No silent multi-minute install happens behind Run.
- Errors include the command or action that fixes them.

## 6. Template Rules

Templates are owned by the `trust` scaffold, not TypeScript. VS Code shells the
same scaffold command used by CLI. Templates include:

- `Cargo.toml`;
- `Cargo.lock` when policy decides it belongs in the template;
- `rust-toolchain.toml`;
- `trust.toml`;
- `.vscode/launch.json`;
- `.vscode/extensions.json`;
- generated ST seed artifacts or a first-run generation step;
- motor-latch source;
- machine test;
- traces directory;
- protective defaults.

## 7. Version Skew

Refuse when:

- VSIX `trust` does not match SDK pin;
- generated artifacts were produced by incompatible codegen;
- module ABI major differs;
- `trust-runtime` cannot load the generated bundle;
- rust-toolchain pin is unsupported by the installed `trust`.

Diagnostics name expected and found values.

## 8. Release Packaging

Release workflow must package or publish:

- platform VSIX artifacts;
- `trust` umbrella binary;
- Rust PLC templates;
- SDK crates or local registry/source;
- checksums;
- version manifest;
- schema fixtures consumed by VS Code tests.

CI/release wiring rows:

- CI builds the `trust` umbrella used by the VSIX.
- CI verifies Rust PLC templates exist in the packaged artifact.
- CI verifies scaffolded exact SDK pins match the workspace/package version.
- CI uploads schema fixtures for BC-1, BC-5, and BC-6.
- Release workflow fails if VSIX/package manifests omit the Rust PLC
  templates, `trust` binary, or version manifest.
- Release evidence records crates.io SDK version, VSIX version, `trust`
  version, and schema fixture version separately.

## 9. Tests

- Preflight pure-model tests for all states.
- VS Code scaffold tests for missing cargo/rustup/trust/SDK.
- Cancel path leaves filesystem unchanged.
- Version skew fixture refuses with expected-vs-found.
- Fresh scaffold passes `trust check`.
- Offline-mode test if bundled SDK source is chosen.
- Release packaging test verifies templates and binaries are present.
