# Rust PLC Codegen And Digest Contract

**Status:** implementation contract, v1 (2026-07-03).
**Master:** `rust-support-architecture-spec-v1.md`.
**Applies to:** Rust-first codegen, generated IEC artifacts, generated
runtime/io config, digest chain, generated-ST diff workflow, F22.

## 1. Purpose

Rust-first authoring produces IEC-compatible artifacts. Those artifacts are
reviewable and deterministic, but not hand-authored. The digest chain makes
stale or edited generated files fail loudly.

## 2. Inputs

Codegen inputs:

- `Cargo.toml`
- `Cargo.lock`
- `rust-toolchain.toml`
- `trust.toml`
- Rust source files that export `#[trust_program]`, `#[trust_fb]`, devices,
  services, or sequences
- SDK/ABI version
- target triple, only for target-specific bundle outputs
- selected profile, only for profile-specific bundle/admission outputs
- imported device schema snapshots
- codegen version

All inputs included in a generated artifact must be represented in the digest
manifest.

## 3. Outputs

Generated project artifacts:

```text
src/generated/trust/declarations.st
src/generated/trust/configuration.st
src/generated/trust/devices_*.st
src/generated/trust/generated_manifest.json
runtime.toml
io.toml
target/trust/<profile>/bundle_manifest.json
target/trust/<profile>/*.trusttime
target/trust/<profile>/*.trustcrate
```

Generated artifacts under `src/generated/trust/`, `runtime.toml`, and
`io.toml` are reviewable and may be committed when the template requires it.
Admission and crate records under `target/trust/` are build outputs and are
not hand-committed unless a later release process defines an evidence archive.

Committed generated artifacts MUST be target/profile invariant. If an output
depends on target triple, measured target identity, optimization profile, or
deployment target, it belongs under `target/trust/<profile>/` or another
build-output directory and is not part of the committed generated source set.

## 4. Determinism Rules

- Same inputs produce byte-identical outputs.
- Output ordering is stable.
- Paths inside generated files are normalized.
- Timestamps are excluded from generated source artifacts.
- Tool versions appear only in the manifest, not in unstable source comments.
- Formatting is stable and covered by snapshot tests.
- Codegen does not read ambient environment except declared target/profile.

## 5. Digest Manifest

`generated_manifest.json` records:

- schema version;
- codegen version;
- SDK/ABI version;
- source file digests;
- `trust.toml` digest;
- Cargo lock digest;
- target triple/profile only for target-specific bundle manifests;
- artifact class: committed-invariant or target/profile-specific;
- generated file digests;
- source-to-generated location map;
- command that regenerates the files.

The manifest must be machine-readable by CLI, VS Code, and CI.

## 6. F22 Generated-artifact Drift

F22 is raised when:

- a generated file digest differs from the manifest;
- a generated file is missing;
- the manifest is missing or stale;
- a generated artifact was produced by an incompatible codegen version;
- source inputs changed and generated artifacts were not refreshed.

Reaction:

- `trust check` fails;
- Problems entry points at the generated artifact or manifest;
- message names the regeneration command;
- VS Code offers Review generated ST changes or Regenerate where honest.

## 7. Generated ST Policy

- Generated ST opens read-only by default in VS Code.
- Generated ST carries a banner: generated, reviewable, edit Rust/trust.toml
  instead.
- Direct edits are not silently accepted.
- Brownfield users can inspect generated declarations without learning Rust.
- Generated diagnostics are mapped back to Rust/trust.toml when the cause is
  user-authored; otherwise they are labeled as toolchain bugs.

## 8. Source-location Map

The map supports:

- Rust source -> generated declaration;
- generated ST declaration -> Rust source;
- trust.toml task/config line -> generated configuration;
- sequence wait point -> source file/line;
- admission contributor -> source program/function/block.

The location map must be deterministic and included in generated metadata.

## 9. Generated Runtime And I/O Config

`runtime.toml` and `io.toml` generated from `trust.toml` must:

- preserve protective defaults;
- include safe-state outputs;
- preserve one I/O binding truth;
- include only generated values;
- refuse manual drift under the same F22 policy unless a later contract marks
  specific sections as user-owned.

## 10. Diff Workflow

The check pipeline can generate to a temporary directory and compare against
committed/generated artifacts. VS Code uses native diff views:

- generated ST diff;
- runtime/io generated diff;
- manifest diff when relevant.

No bespoke diff viewer is required.

## 11. Tests

- Golden snapshots for all generated files.
- Repeated generation byte-stability test.
- F22 drift tests: edit generated ST, delete manifest, stale source, stale
  codegen version.
- Location-map tests.
- Generated ST parse/compile tests.
- VS Code read-only/diff contract tests.
- Cross-platform path normalization tests.
- Cross-target invariance test for committed generated artifacts on the
  supported dev-host triples.
- Profile-invariance test for committed generated artifacts; profile-specific
  differences must appear only in build outputs.
