---
name: trust-architecture-automation
description: "Use when adding, installing, running, or reviewing automated architecture mapping and bug-finding tools in trust-platform, including architecture-doctor, generated software maps, diagram claim checks, cargo-deny/audit/machete/geiger/coverage/mutation/fuzz/Miri/sanitizer/perf gates, and CI evidence for architectural drift."
---

# trust-platform Architecture Automation

Use this skill for work that must replace hand-checked architecture claims with automated evidence.
Use `trust-test-authoring` first whenever the automation change adds or changes a gate, test,
mutation/fuzz claim, or proof-bearing behavior.

## Required posture

- Treat source-derived facts as truth; treat diagrams as views that must be checked.
- Do not trust a `.puml` claim unless it is generated from or verified against code facts.
- Prefer repo-specific doctor checks for project semantics; generic tools only supply raw facts.
- If a tool cannot be installed or run, record the exact blocker and the fallback.
- In trust-platform checkouts on a Raspberry Pi or other slow local host, use the remote builder for broad/full validation first, especially `just test-all`.
- Ask before starting expensive local commands such as workspace `cargo test`, `cargo test -p trust-runtime ...`, local `just test`, local `just clippy`, local `just test-all`, or heavy architecture/perf tools.
- Cheap local checks are allowed when narrowly scoped, for example formatting, small touched-crate tests, `cargo test -p xtask`, architecture-doctor checks for the touched area, and static inspections.

## Standard workflow

1. Read `docs/internal/testing/checklists/architecture-automation-tooling.md`.
2. For every new doctor/gate behavior, bug fix, or intentional behavior change, add the smallest focused self-test or known-bad fixture first and run it to the expected behavior assertion failure. Implement only the minimum change, then rerun the same test until green; harness, dependency, timeout, and unrelated failures do not count as red evidence.
3. For architecture changes, generate or update the software map before reviewing diagrams.
4. Run the architecture doctor for the touched area before declaring implementation complete.
5. Regenerate diagrams only after factual checks pass.
6. Run the relevant static/test/perf tools and record the focused red-green evidence plus broader evidence.

## Tool groups

- Code map: `cargo metadata`, `guppy`, `cargo tree`, `cargo-modules`, rustdoc JSON, `ra_ap_syntax`, `ast-grep`, tree-sitter, and `cargo-call-stack` where it applies.
- Doctor checks: repo-specific `cargo xtask architecture-doctor`, Semgrep/Dylint for simple forbidden patterns.
- Static/supply chain: `cargo deny`, `cargo audit`, `cargo machete`, `cargo udeps`, `cargo geiger`, `cargo about`, `cargo semver-checks`, `cargo public-api`.
- Test adequacy: `cargo nextest`, `cargo llvm-cov`, `cargo mutants`, `cargo fuzz`, property tests.
- Deep bug checks: Miri, sanitizers, loom, Valgrind/rr when relevant.
- Perf/size: Criterion, `trust-runtime bench`, `cargo bloat`, `cargo llvm-lines`, `cargo build --timings`, flamegraph/perf.

## Minimum acceptance for architecture-sensitive work

- A generated/checkable fact exists for each claim that previously drifted.
- The architecture doctor fails on the known-bad pattern before the fix and passes after.
- Diagrams pass both render drift and semantic claim checks.
- Any skipped tool has a documented reason and a follow-up item.

## Issue #51 class of checks

For initializer/parser/HIR/runtime work, the doctor must check:

- `parse_var_initializer` call sites are limited to initializer-aware contexts.
- HIR owns source initializer metadata and never stores runtime `Value`.
- Runtime owns lowered initializer records and materializes through the initializer service.
- VM local/static initializer behavior stays symmetric with normal runtime startup.
- Cross-project imports translate initializer IDs instead of dropping defaults.
- No `_initializer` discard or `default_initializer: None` regression silently reappears.
