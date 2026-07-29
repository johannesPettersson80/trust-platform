# Phase 18 Rust assertion-helper closure

Date: 2026-07-29

## Scope and result

This verification-tooling repair closed the false unresolved-helper population
across the complete extractable remainder of the Phase 18 product-test
denominator:

- 338 remaining product tests were selected from the frozen denominator;
- two Windows-only facts remained excluded from the Linux-host extraction;
- all 336 extractable tests were regenerated on one exact clean source commit;
- unresolved function helpers fell from 418 at recovery to zero;
- bound assertion sites rose from 2,905 to 3,525;
- assertion-empty inventories fell from 20 to 11;
- the only residual extractor findings are 85 explicit dynamic
  `unresolved-oracle` reads across 79 tests.

This repair does not catalog-map the 336 tests or retire the 85 dynamic oracle
facts. The frozen repository equations remain:

```text
4,411 live = 3,983 catalog_mapped + 428 reviewed_nonmapping
0 missing specification authority
0 confirmed missing behavior tests
```

It is a large supporting batch for `VERIF-P18-005` and the later
assertion-strength rows, not global row closure.

## Exact checkout and bootstrap

- Canonical agent-file source:
  `/home/johannes/projects/trust-platform`
- Local branch: `plc-verification-program`
- Exact source checkpoint:
  `84d6c31e8b930efc1b4c550e432dee74d4e612ae`
- Clean validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-rust-assertion-84d6`
- Builder host: `scena-rust-builder`
- Builder checkout mode: detached HEAD

The destination `AGENTS.md` and complete `.codex/skills/**` directory were
manually copied from the canonical checkout and verified byte-for-byte:

```text
AGENTS.md
  76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263
.codex/skills aggregate
  253377375e0009b82f732dfd26758e46b173fcedec8e0bf7bd85460946ea37e6
```

## Test-first repair

Focused expected-red assertions independently exposed these missing behaviors
before each implementation:

- `pub(super)` helper visibility across sibling test modules;
- repeated `super::super` import anchoring;
- full `cfg(test)` unit-module sibling context;
- selection of the Cargo lib or bin target that actually owns the test;
- exclusion of ordinary cross-file and same-file production execution from
  delegated test oracles;
- transitive named imports through restricted and glob re-exports;
- function-local named imports.

Each expected red was the intended unresolved-helper or delegated-oracle
assertion failure. The same exact focused test passed after its minimal repair.
Compile, dependency, timeout, and unrelated failures were not counted as red
evidence.

The resulting ownership remains split:

- `unit_context.rs` resolves the owning Cargo target and test-inclusive module
  graph;
- `import_resolution.rs` owns visibility-aware local, named, glob, and
  transitive import resolution;
- `test_assertions.rs` owns traversal and the boundary between test-only
  helper closure and ordinary production execution;
- separate focused test modules keep the main extractor and its pre-existing
  test module below the large-file guardrail.

## Exact focused proof

On the clean builder source checkpoint:

```text
scripts/verification_assertion_inventory_gate.sh
  8 Python checks + 31 Rust assertion checks = 39/39 pass

cargo test -p xtask code_spec_scan::tests:: -- --nocapture
  17/17 pass

python3 scripts/check_diagram_drift.py
  pass
```

The first combined-gate attempt in the detached worktree failed before the
tests because the worktree lacked `editors/vscode/node_modules/typescript`.
After linking the builder's existing dependency cache, the unchanged gate
passed. That environmental setup failure is not red evidence.

The exact 336-inventory regeneration reported:

```text
336 inventories
3,525 assertions
79 tests with unresolved findings
85 unresolved-oracle findings
0 other unresolved helpers
11 assertion-empty inventories
```

After replacing only the temporary generator's stale source-commit label with
the exact validated commit in memory, the canonical normalized inventory
digest is:

```text
sha256:6f82603fc44e6f98f6b4192936c8291706905ea16a058946c7084d138a878c1e
```

The compact machine-readable summary is retained at
`docs/internal/testing/evidence/plc-verification-program/2026-07-29/p18-rust-assertion-helper-closure/summary.json`.

## Remaining debt and cadence

The next coherent batch classifies the 85 dynamic reads into digest-bound
repository inputs, closed generated-path reads, or genuine runtime/host
observations. The 11 assertion-empty inventories remain explicit assertion
strength debt. No mapping, reachability, mutation, coverage, hardware, UX, or
platform row was promoted by this structural repair.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
