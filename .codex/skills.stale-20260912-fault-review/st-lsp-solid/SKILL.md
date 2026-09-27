---
name: st-lsp-solid
description: "Enforce SOLID/KISS/DRY architecture in trust-platform when adding features or refactoring runtime/LSP/IDE/CLI AND xtask gate/proof/validator code. Use for module splits, ownership changes, runtime launcher/control/IO/retain/watchdog/debugger flow, any file approaching ~1k lines, or any master plan refactor item."
---

# ST LSP SOLID Workflow

Use `trust-test-authoring` first for planner, specification, catalog, and proof routing. This skill
then owns structure, boundaries, and SOLID/KISS/DRY acceptance for the implementation.

## Before coding
- Identify the module(s) affected and their single responsibility.
- Identify whether the change is behavior-preserving refactor or behavior-changing feature.
- For every new feature, bug fix, or intentional behavior change, write the smallest focused test first and run it to the expected behavior assertion failure before changing production code. Implement the minimum change, then rerun the same test until it is green.
- A compile, dependency, harness, registration, timeout, or unrelated failure is not valid red evidence.
- If refactor-only, define behavior-lock tests and confirm they are green before editing; keep them green throughout.
- For architecture-program board work, map the work item through `docs/internal/testing/checklists/architecture-workboard-index.md`, then `docs/internal/testing/checklists/full-architecture-refactor-program-checklist.md`, then the active dedicated board checklist. Do not use `docs/internal/masterPlan.md` to sequence architecture boards.
- For non-architecture master-plan work, map the work item to `docs/internal/masterPlan.md` and phase gates.
- Review `docs/internal/testing/checklists/architecture-improvements.md` (Runtime SOLID Refactor section).
- In trust-platform checkouts on a Raspberry Pi or other slow local host, use the remote builder for broad/full validation first, especially `just test-all`.
- Ask before starting expensive local commands such as workspace `cargo test`, `cargo test -p trust-runtime ...`, local `just test`, local `just clippy`, local `just test-all`, or heavy architecture/perf tools.
- Cheap local checks are allowed when narrowly scoped, for example formatting, small touched-crate tests, `cargo test -p xtask`, architecture-doctor checks for the touched area, and static inspections.
- If a change crosses boundaries (launcher/control/IO/retain/watchdog/debug), plan an interface or subsystem split first.

## Implementation rules
- Keep **transport** separate from **business logic** (control server + handlers, CLI parsing + runtime assembly).
- Prefer **traits/interfaces** for external dependencies (IO drivers, retain stores, watchdog actions).
- Avoid expanding any “god object” or god-file (runtime structs and xtask gate/proof files); introduce small subsystem structs / per-responsibility modules and compose them.
- Split files that exceed ~1k lines or mix unrelated responsibilities. New validators/proof checks go in a new per-responsibility module, never appended to an already-large file.
- Keep public API and contract behavior stable unless the work item explicitly changes behavior.

## Diagram + checklist sync
- Update PlantUML source files under `docs/diagrams/**/*.puml` whenever ownership, execution flow, or component contracts change.
- Regenerate diagram outputs after `.puml` edits (`scripts/render_diagrams.sh`).
- Refresh `docs/diagrams/manifest.json` and verify no drift (`python scripts/check_diagram_drift.py`).
- Add/refine items in `docs/internal/testing/checklists/architecture-improvements.md` for any new SOLID gaps.

## Required gates
- Preserve the recorded red-then-green evidence for every behavior-changing slice (unit/integration/negative/regression as appropriate).
- Do not ship a behavior-changing refactor without behavior-lock tests.
- Update docs/checklists when behavior or ownership changes.
- Run cheap local checks only when narrowly scoped, then run full gates on the remote builder:
  - `just fmt`
  - remote `just test-all` before completion (or staged-checklist full gate if a project checklist explicitly defines milestone-only full tests)
  - `cd editors/vscode && npm run lint && npm run compile` when extension files are touched

## Runtime vertical validation (required for runtime changes)
- When `trust-runtime` behavior is touched, run end-to-end checks from interface/control surfaces to runtime core on the remote builder unless the user explicitly approves local execution:
  - `cargo test -p trust-runtime --test api_smoke`
  - `cargo test -p trust-runtime --test debug_control`
  - `cargo test -p trust-runtime --test complete_program`
  - `cargo test -p trust-runtime --test runtime_reliability`
- If runtime protocol/debug behavior impacts VS Code flows, also run extension integration tests:
  - `cd editors/vscode && ST_LSP_TEST_SERVER=<path>/trust-lsp npm test`
- If runtime web UI behavior is touched, execute the relevant sections of `docs/internal/testing/manual-tests-ui.md` and capture evidence.

## Deviation handling
- If a SOLID deviation is unavoidable, record it in `docs/notes/runtime-refactor-notes.md` and add a follow-up checklist item.
