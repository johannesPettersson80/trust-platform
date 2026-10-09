---
name: st-lsp-solid
description: Keeps truST's architecture simple and well separated, and runs the automated architecture checks. Use for refactors, module splits, ownership or boundary changes in the runtime, language server, IDE, CLI or xtask gates; for files nearing 1,000 lines; for runtime launcher, control, I/O, retain, watchdog or debugger flow; and for architecture-doctor, software maps, diagram claims, and static-analysis, fuzz, Miri, mutation or performance tooling.
---

# Architecture and structure

**Hard rule (AGENTS.md): all logic is in Rust.** Logic is anything whose result is stored, sent to
a runtime or PLC, or decides an engineering or operator outcome. TypeScript only presents (drawing,
layout, mouse interaction, view state) and forwards requests to Rust. Never add logic in
TypeScript. Existing TypeScript logic is debt to move into Rust, not to extend.

A structural change never moves logic into TypeScript. Reshaping code that holds TypeScript logic
moves that logic into a Rust crate, reached through a `trust-lsp` command or the runtime protocol.

## Before changing structure

- **Name the change.** State the module's single responsibility, and say whether the change is a
  behaviour-preserving refactor or a behaviour change.
- **Refactors** keep behavior-lock tests passing before and after (`trust-test-authoring`).
- **Boundary crossings**: when a change crosses runtime boundaries (launcher, control, I/O, retain,
  watchdog, debug), design the interface or subsystem split first.
- **Known gaps**: read the Runtime SOLID section of
  `docs/internal/testing/checklists/architecture-improvements.md`, and add any new gaps you find there.

## Rules

- **Transport and logic**: keep transport separate from business logic. The control server stays
  apart from its handlers, and CLI parsing apart from runtime assembly.
- **External dependencies**: put them behind traits. These include I/O drivers, retain stores and
  watchdog actions.
- **No god objects or god files**: this includes xtask gate and proof files.
  - Add a small module for each responsibility, and compose them.
  - Split a file that passes about 1,000 lines or mixes responsibilities.
  - New validators and proof checks always go in a new module.
- **Stable contracts**: keep public APIs and contracts stable unless the task changes them.
- **Unavoidable deviations**: record them in `docs/notes/runtime-refactor-notes.md` with a follow-up
  item.
- **Diagrams**: changes to ownership, data flow or execution flow update the PlantUML diagrams
  (AGENTS.md, "Diagrams").

## Architecture automation

- **Facts first**: code facts are the truth, and diagrams are views checked against them. Prefer
  repo-specific doctor checks for project semantics; generic tools supply raw facts.
- **The doctor**: run `cargo xtask architecture-doctor` before declaring architecture work complete.
  - Add `--full-map` for module splits, for large-file waivers, or for changes to
    `xtask/config/full_map_policy.json`.
  - `bash scripts/architecture_safety_gate.sh` is the CI form.
  - A new doctor check is proven by a known-bad fixture that fails before the fix and passes after.
- **Diagram order**: regenerate diagrams only after the factual checks pass.
- **Tool records**: plan and record tool work in
  `docs/internal/testing/checklists/architecture-automation-tooling.md`. If a tool cannot be
  installed or run, record the blocker and the fallback.
- **Tools by purpose**:

  | Purpose | Tools |
  | --- | --- |
  | Code map | `cargo metadata`, guppy, `cargo tree`, `cargo-modules`, rustdoc JSON, `ra_ap_syntax`, ast-grep, tree-sitter, `cargo-call-stack` |
  | Forbidden patterns | Semgrep, Dylint |
  | Supply chain and API | `cargo deny`, `cargo audit`, `cargo machete`, `cargo udeps`, `cargo geiger`, `cargo about`, `cargo semver-checks`, `cargo public-api` |
  | Test adequacy | `cargo nextest`, `cargo llvm-cov`, `cargo mutants`, `cargo fuzz`, property tests |
  | Deep checks | Miri, sanitizers, loom, Valgrind, rr |
  | Performance and size | Criterion, `trust-runtime bench`, `cargo bloat`, `cargo llvm-lines`, `cargo build --timings`, flamegraph |
- **Variable initializers** (parser, HIR, runtime): the doctor keeps these properties:
  - `parse_var_initializer` is called only in initializer-aware contexts;
  - HIR owns the source initializer metadata and never stores a runtime `Value`;
  - the runtime materializes lowered initializers through the initializer service;
  - VM local and static initializers behave like normal startup;
  - cross-project imports translate initializer IDs;
  - no `_initializer` discard or `default_initializer: None` regression comes back.

## Validation

- **Runtime changes**: run the runtime vertical (AGENTS.md).
- **Local checks**: `cargo test -p xtask` and the doctor for the touched area are cheap enough to run
  on the Pi.
- **Performance**: measure on the product CPU, an AM6442 Cortex-A53. Results from the Pi 5's
  Cortex-A76 or from the builder do not transfer.
