# Verification Control Plane

Status: initial seed for the PLC verification program.

This directory holds machine-readable verification metadata. It does not move
or replace executable tests. Native tests stay in `crates/*/tests`,
`crates/*/src`, `editors/vscode/src/test`, `conformance`, `fuzz`, and gate
scripts.

Current seed scope:

- source-build/OpenOT public-build truth for GitHub issue #93,
- bytecode/VM specification sources,
- runtime safety specification sources,
- first public-docs claim scan with explicit gaps,
- bytecode/VM-only planning matrix pilot for `plan_tests.py`.
- bytecode/VM-only decision-table case generator pilot for `gen_cases.py`.
- bytecode/VM-only transform seed artifacts under `verification/seeds/**`.

TOML shape convention:

- invariants are one record per file under
  `verification/invariants/<area>/<INVARIANT_ID>.toml`;
- flat registries use plural wrapper arrays, for example `[[spec_sources]]`,
  `[[spec_gaps]]`, and `[[evidence]]`;
- committed case tables live under `verification/cases/<area>/**` and are
  referenced from planned test-catalog rows by path plus SHA-256 digest.
  These rows pin planning artifacts only; they do not count as runnable proof.
- committed transform seeds live under `verification/seeds/<area>/**`. They are
  deterministic input artifacts for generators and must not be treated as
  executable proof by themselves.
- `verification/spec-matrix.toml` will define required specification tags per
  canonical area. A tag is satisfied by an active spec source whose `covers`
  list includes the tag, or by an open spec gap with an owner.
- `verification/matrix.toml` defines planning globs, risk defaults, required
  test classes, and case families. In Phase 1B it is intentionally scoped to
  `bytecode_vm`; every other area remains uninventoried and must fail closed.
- `scripts/gen_cases.py --invariant <ID>` derives case records from invariant
  behavior rows. Rows with `spec_gap_ref` become blocked cases. Rows with
  `oracle_ref` copy expected outcomes from the behavior row; the tool never
  invents expected behavior. In the bytecode/VM pilot it can also derive
  blocked transform cases from committed seed artifacts.
- `crates/verification-cases` is the dev-helper crate for tests that consume
  committed case tables. It records blocked cases without execution, wraps
  runnable cases with `StateProbe` snapshots, and writes JSON artifacts under
  the workspace-root `target/gate-artifacts/cases/`. It requires the cataloged
  case-file digest before execution. It does not create durable evidence;
  `prove.py` owns that later phase.
- `scripts/verification_report_gate.py` is the report-only CI front door during
  pilot burn-in. It runs the metadata/case-file gate, re-derives planner output
  for changed files, and reports uncataloged changed tests without enforcing
  outside the pilot.
- `scripts/verification/adversarial_selftest_tests.py` is the pilot's
  bypass-resistance fixture suite. It checks that shortcut attempts become
  failed validation, non-red proof, or report-only findings.
Do not mark records `validated` until the validators, suite definitions, and
evidence index checks described in
`docs/internal/testing/checklists/plc-verification-program/metadata-model.md`
exist and pass.
