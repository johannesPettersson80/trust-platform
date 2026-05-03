# Architecture Automation Tooling Checklist

Status: In progress - tool substrate installed; Issue #51 architecture map/doctor implemented; broader static, coverage, mutation, fuzz, Miri, sanitizer, and perf campaign gates still pending.

Purpose: replace hand-verified architecture claims with code-derived facts wherever possible. Diagrams remain useful views, but generated facts and doctor checks are the source of truth.

Execution board for the full-map doctor command: `architecture-doctor-full-map-execution-checklist.md`. That board is the executable plan for the command; this tooling checklist remains the broader tool-install and automation inventory.

## Phase 0 - Tool Install Inventory

- [x] `AUTO-TOOL-001` Install core cargo automation tools: `cargo-deny`, `cargo-audit`, `cargo-machete`, `cargo-semver-checks`, `cargo-public-api`, `cargo-llvm-cov`, `cargo-mutants`, `cargo-geiger`, `cargo-about`, `cargo-auditable`, `cargo-bloat`, `cargo-llvm-lines`.
- [x] `AUTO-TOOL-002` Install architecture graph command-line tools: `cargo-modules`, `cargo-depgraph`, `cargo-deps`, and Graphviz `dot`. Guppy support is tracked in `AUTO-MAP-002`.
- [x] `AUTO-TOOL-003` Install deeper analysis tools where supported on this ARM64 machine: `cargo-udeps`, `cargo-tarpaulin`, `cargo-careful`, `cargo-afl`, `cargo-flamegraph`, `cargo-vet`, `cargo-crev`, `cargo-call-stack`, Semgrep, `ast-grep`, `tree-sitter`, Syft, Trivy, Grype, OSV-Scanner, D2, PlantUML, Valgrind, and rr. CodeQL is deferred to x86-64 CI because the local host is Linux ARM64 and the current CodeQL CLI Linux asset is x86-64.
- [x] `AUTO-TOOL-004` Install Rust deep-check components/toolchains: nightly toolchain, `miri`, `rust-src`, `llvm-tools`, and sanitizer-ready nightly support.
- [x] `AUTO-TOOL-005` Record install results below, including explicit deferrals when a tool requires a system package manager, unsupported architecture, nightly-only behavior, or a long-running build.

## Phase 1 - Code-Derived Software Map

- [x] `AUTO-MAP-001` Add `cargo xtask architecture-map` to emit `docs/internal/architecture/generated/software-map.json`.
- [x] `AUTO-MAP-002` Map workspace crates, targets, features, and package dependencies from `cargo metadata` / `guppy`.
- [ ] `AUTO-MAP-003` Map crate module trees from Rust source and/or `cargo-modules`.
- [ ] `AUTO-MAP-004` Map selected public API surfaces from rustdoc JSON / `cargo-public-api`.
- [x] `AUTO-MAP-005` Map selected function call sites and struct fields for architecture-sensitive paths.
- [ ] `AUTO-MAP-006` Keep generated map deterministic and publish it as a CI artifact.

## Phase 2 - Architecture Doctor

- [x] `AUTO-DOC-001` Add `cargo xtask architecture-doctor --all` and `--changed` modes.
- [x] `AUTO-DOC-002` Check parser gate facts against allowed initializer-aware contexts.
- [x] `AUTO-DOC-003` Check HIR/runtime dependency boundaries: no HIR dependency on runtime `Value`, no raw CST semantic contract in runtime paths unless explicitly allowlisted.
- [x] `AUTO-DOC-004` Check runtime initializer routing: VAR, TYPE/member defaults, VAR_CONFIG, retain/restart, interpreter, and VM local/static paths must use the initializer service/catalog.
- [x] `AUTO-DOC-005` Check symmetric runtime/VM behavior for feature-critical initializer semantics.
- [x] `AUTO-DOC-006` Check forbidden silent-drop patterns such as `_initializer` discards and `default_initializer: None` in import/collector paths.
- [x] `AUTO-DOC-007` Check agreed file/function size caps for initializer/runtime orchestration modules.
- [x] `AUTO-DOC-008` Fail with exact file:line evidence and a short remediation hint.

## Phase 3 - Diagram Automation

- [ ] `AUTO-DIAG-001` Generate factual PlantUML/Graphviz fragments from `software-map.json`.
- [ ] `AUTO-DIAG-002` Add `python scripts/check_diagram_claims.py` to compare diagram claims against generated facts.
- [ ] `AUTO-DIAG-003` Keep hand-authored `.puml` files for intent/layout only; factual call-site and ownership lists must be generated or checked.
- [ ] `AUTO-DIAG-004` Continue running `scripts/render_diagrams.sh` and `python scripts/check_diagram_drift.py`.

## Phase 4 - Static Quality Gates

- [ ] `AUTO-STATIC-001` Add PR gates for `cargo deny check`, `cargo audit`, `cargo machete`, and architecture doctor changed-area checks.
- [ ] `AUTO-STATIC-002` Add scheduled gates for `cargo udeps`, `cargo geiger`, `cargo public-api`, and `cargo semver-checks`.
- [ ] `AUTO-STATIC-003` Add Semgrep or Dylint rules for forbidden imports/calls that are awkward to express in the architecture doctor.
- [x] `AUTO-STATIC-004` Add a CI-enforced external unsafe AST scanner to replace unreliable `cargo geiger` enforcement. Evidence: `scripts/architecture_external_safety_ast_grep_gate.sh` uses `ast-grep 0.42.1` to structurally scan Rust unsafe constructs, compares matches against the full-map unsafe register and delegated unsafe path register, writes artifacts under `target/gate-artifacts/architecture-external-safety-*`, and runs in the CI `Architecture Safety` job.

## Phase 5 - Test Adequacy and Bug Discovery

- [ ] `AUTO-TEST-001` Add `cargo llvm-cov` workspace coverage reporting.
- [ ] `AUTO-TEST-002` Add `cargo mutants` focused jobs for parser, HIR diagnostics, runtime initializer, retain/restart, and VM local init.
- [ ] `AUTO-TEST-003` Add `cargo fuzz` targets for parser, config, and initializer lowering inputs.
- [ ] `AUTO-TEST-004` Add Miri/sanitizer/loom scheduled jobs for unsafe and concurrency-sensitive code through `unsafe-concurrency-hardening-execution-checklist.md`.

## Phase 6 - Performance and Size Automation

- [ ] `AUTO-PERF-001` Add reproducible perf baselines for runtime startup/init, first-cycle, retain restart, and steady-cycle paths.
- [ ] `AUTO-PERF-002` Add `cargo bloat`, `cargo llvm-lines`, and `cargo build --timings` release/nightly reports.
- [ ] `AUTO-PERF-003` Store perf and size reports as CI artifacts and compare them across release-sensitive branches.

## Phase 7 - Codex Workflow

- [x] `AUTO-SKILL-001` Add a Codex skill for architecture automation tool usage.
- [ ] `AUTO-SKILL-002` Update future architecture/refactor work to run the architecture doctor before trusting diagrams or hand-written plans.
- [ ] `AUTO-SKILL-003` When a plan says a path is routed/forbidden, add or update an automated doctor rule before implementation is declared complete.

## Install Evidence

Setup run: 2026-04-27 on Linux ARM64 (`aarch64`, Debian/Raspberry Pi OS family).

Installed cargo/Rust CLI tools:

- `cargo-deny 0.19.4`
- `cargo-audit 0.22.1`
- `cargo-machete 0.9.2`
- `cargo-semver-checks 0.47.0`
- `cargo-public-api 0.51.0`
- `cargo-llvm-cov`
- `cargo-mutants`
- `cargo-geiger 0.13.0`
- `cargo-about 0.9.0`
- `cargo-auditable`
- `cargo-bloat`
- `cargo-llvm-lines 0.4.45`
- `cargo-modules 0.26.0`
- `cargo-depgraph 1.6.0`
- `cargo-deps 1.5.1`
- `cargo flamegraph` / `flamegraph 0.6.12`
- `cargo-careful`
- `cargo-vet 0.10.2`
- `cargo-crev 0.27.1`
- `cargo-afl`
- `cargo-tarpaulin 0.35.4`
- `cargo-udeps`
- `cargo-dylint 5.0.0`
- `dylint-link`
- `cargo-call-stack` plus pinned toolchain `nightly-2023-11-13` with `rust-src` / `llvm-tools-preview`
- Existing: `cargo-nextest 0.9.128`, `cargo-fuzz 0.13.1`, `cargo-binstall 1.18.1`

Installed system/external tools:

- Graphviz `dot 2.42.4`
- `perf 6.12.75`
- Semgrep `1.161.0`
- `ast-grep 0.42.1` (`ast-grep` and `sg`)
- `tree-sitter 0.26.8`
- Syft `1.43.0` (`linux/arm64`)
- Grype `0.111.1` (`linux/arm64`)
- Trivy `0.70.0`
- OSV-Scanner `2.3.5`
- D2 `0.7.1-HEAD`
- PlantUML `1.2020.02`
- OpenJDK `21.0.10`
- Go `1.24.4`
- Valgrind `3.24.0`
- rr `5.9.0`

Installed Rust toolchain support:

- Stable default toolchain with `clippy`, `rustfmt`, `rust-analyzer`, `rust-src`, and `llvm-tools`.
- Nightly toolchain with `miri`, `rust-src`, and `llvm-tools`.
- Pinned `nightly-2023-11-13` for `cargo-call-stack`.

System prerequisites installed during setup:

- `pkg-config`
- `libssl-dev`
- `graphviz`
- `linux-perf`
- `plantuml`
- `default-jre-headless`
- `pipx`
- `golang-go`
- `valgrind`
- `rr`

Deferrals and caveats:

- CodeQL CLI is not installed locally. The current CodeQL release assets list Linux as `codeql-linux64.zip`, and CodeQL's supported Linux platform is x86-64. This ARM64 machine should run CodeQL through an x86-64 CI runner or a supported remote environment instead of pretending local CodeQL is available.
- SonarQube/SonarCloud is not installed as a local service. Treat it as a CI/service integration item that needs a server or SonarCloud token before it can provide useful project evidence.
- Installing `linux-perf` upgraded Raspberry Pi kernel packages from the package repository. A later reboot may be required for the new kernel package to become active, but the `perf` user-space command is installed.
- `cargo-udeps`, Miri, `cargo-call-stack`, and sanitizer-style checks remain nightly-sensitive even though the tools/components are installed.
