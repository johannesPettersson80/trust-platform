# truST PLC Verification Program

Status: reviewed. External review returned `clear-with-edits` (Fable,
2026-07-08); required edits are folded and verified, and the review gate in
`implementation-board.md` is cleared. Phase 1 may start under the policy stop
gates.

Created: 2026-07-07. Split into focused documents: 2026-07-08.

## Purpose

The goal is not "more tests." The goal is that every important truST product
claim is traceable to:

1. a written specification or explicit spec gap,
2. an invariant,
3. a test or proof plan,
4. a suite/gate,
5. durable evidence,
6. a release/public-claim status.

If a claim creates or changes a user-facing workflow, the user story or public
workflow is part of the written specification. It must describe the actor,
preconditions, visible steps, success state, failure/status behavior, and
acceptance evidence before implementation starts.

This is a SQLite-style proof discipline adapted to PLC risk:

- Wrong output is worse than a crash.
- False healthy status is worse than an honest fault.
- Silent source/codegen corruption is worse than a refused build.
- Hardware behavior cannot be proven by mocks alone.
- IEC semantics need IEC/spec or documented-deviation oracles, not parity with
  another truST engine that may share the same bug.

## Document Map

- `policy.md`: stop gates, vocabulary, code-change discipline, spec-gap rules,
  test-refactor policy, suite tiers, metrics, and final definition of done.
- `metadata-model.md`: machine-readable record shapes, schema files, status
  vocabulary, authority precedence, and schema versioning.
- `metadata-evidence-traceability.md`: evidence record rules, traceability
  report requirements, and verification-tooling self-test fixtures.
- `spec-matrix-model.md`: canonical area values and required-specification
  matrix semantics.
- `test-taxonomy.md`: code-area matrix, test classes, coverage dimensions,
  malformed-input taxonomy, supply-chain/security tests, and platform tests.
- `verification-areas.md`: domain ownership, invariant classes, harnesses, and
  initial high-risk invariant seeds.
- `implementation-board.md`: staged implementation checklist.
- `execution-slice-001.md`: Phase 16 readiness and the queued first
  spec-to-green product vertical.
- `fable-review-brief.md`: prompt and acceptance criteria for external review.
- `../plc-verification-program-checklist.md`: short entrypoint that points here.

## Implementation Order

The program must start with specifications, not tests.

1. Create the verification skeleton.
2. Inventory specification/oracle sources.
3. Create the required-specification matrix so each canonical area has explicit
   required spec tags, owners, authority expectations, and source/gap refs.
4. Add structured invariants with `contract_kind` and behavior rows before new
   feature or bug-fix tests are authored. If the work creates a user story or
   public workflow, specify that workflow first or record a spec gap.
5. Let the planner derive required test classes and case families from metadata;
   missing behavior rows become spec gaps.
6. Inventory existing tests.
7. Classify spec gaps separately from test gaps.
8. Run the bytecode/VM pilot first.
9. Broaden to runtime safety, IEC/HIR, protocols, editor/UI, release, and
   hardware lab only after the pilot produces useful reports.

Most important safety area:

- `trust-runtime`, especially scan-cycle safety and the HIR -> bytecode -> VM
  seam.

First implementation pilot:

- `crates/trust-runtime/src/bytecode/**`
- `crates/trust-runtime/src/runtime/vm/**`
- `crates/trust-runtime-core/src/bytecode/**`
- `crates/trust-runtime-core/src/vm/**`
- `crates/trust-runtime-core/src/value/**`
- existing bytecode/VM tests and malformed bytecode fixtures

Why this pilot:

- safety-critical,
- deterministic,
- malformed-input-heavy,
- does not require hardware, browser, VS Code, or network proof,
- exercises spec-source inventory, invariant mapping, coverage gaps, ignored
  protective tests, and validator/report self-tests.

Spec-first planner pilot:

- `plan_tests.py` reports required proof from committed metadata and fails
  closed on unmapped files, uninventoried areas, unknown risk, or missing
  behavior rows.
- `gen_cases.py` derives hostile data-shaped cases from decision-table behavior
  rows or deterministic bytecode transforms; it never invents expected behavior.
- `prove.py red|green|lock` runs cataloged commands and writes evidence records
  itself, using per-case artifacts emitted by the dev-only
  `verification-cases` helper. Phase 16 first closes clean/durable proof,
  state-machine trace provenance, and evidence-bound promotion contracts before
  any product execution.
- Scenario/fault families such as SIGTERM, worker down, slow handshakes, and
  hardware reconnect stay out of v1 case generation until the Phase 8
  fault-injection harness exists.

## Current Status

External review (Fable, 2026-07-08) returned `clear-with-edits`. Required
edits are folded and verified; the verdict, fold verification, baseline
counts, and snapshot live under
`docs/internal/testing/evidence/plc-verification-program/2026-07-08/`.
The later spec-first planner amendment review is folded in the same evidence
root. Phase 1 implementation is unblocked after these document-only fixes, but
the policy stop gates still control every implementation row.

Still binding during implementation:

- the stop gates in `policy.md`,
- no test moves before the catalog proves destination and runner
  (`VERIF-STOP-002`),
- nothing is marked `validated` without tests, evidence, and closed safety
  coverage cells,
- no runtime/compiler/LSP behavior changes in control-plane Phases 0-15;
  Phase 16 product work remains blocked until its readiness rows close and then
  follows the spec/red/fix/green/gate discipline in `execution-slice-001.md`.

## Storage Summary

Executable tests stay where their native tooling expects them:

```text
crates/*/src/**              In-source unit tests.
crates/*/tests/**            Crate integration and regression tests.
editors/vscode/src/test/**   VS Code extension tests.
conformance/**               Public deterministic conformance tests.
fuzz/**                      Fuzz targets and minimized corpora.
scripts/**                   Gate runners, report renderers, validators.
```

Verification metadata lives under the future `verification/` control plane:

```text
verification/
  invariants/
  suites/
  cases/
  schemas/
  invariant-seeds.toml
  gate-inventory.toml
  matrix.toml
  spec-matrix.toml
  test-catalog.toml
  ignored-tests.toml
  risk-register.toml
  evidence-index.toml
  spec-sources.toml
  spec-gaps.toml
  runtime-anomaly-taxonomy.toml
  mutation-program.toml
```

Generated reports are artifacts under `target/gate-artifacts/**` or CI
workspace artifact paths unless a reviewed summary is intentionally committed.

Phase 8 keeps the runtime-anomaly audit report-only. Its 19-class taxonomy and
explicit Rust scanner associations create planned routing and visible test
debt, not expected behavior, invariant coverage, fault execution, or proof.
The exhaustive mapping row, fault-toggle row, and production-hook guard row
remain open.

Phase 10 defines six exact focused mutation shards and seven selected mutants.
Only the existing bytecode-validator pilot is measured; runtime conversion,
HIR diagnostics, parser recovery, retain/restart, and connector projection are
planned definitions with empty result arrays. Exact live test associations do
not claim execution or mutant causality. Mutation and coverage are adequacy
signals only, and a future delivered-binary run must carry artifact identity
and direct-execution confirmation before it may be measured.

The Phase 2 existing-test scanner is:

```text
python3 scripts/scan_test_catalog.py \
  --json-out target/gate-artifacts/verification/existing-test-catalog.json \
  --markdown-out docs/internal/testing/evidence/plc-verification-program/<date>/p2-existing-test-catalog.md \
  --timestamp <fixed-iso-8601-time>
python3 scripts/validate_generated_test_catalog.py \
  --json target/gate-artifacts/verification/existing-test-catalog.json \
  --markdown docs/internal/testing/evidence/plc-verification-program/<date>/p2-existing-test-catalog.md
python3 scripts/check_test_catalog_staleness.py
python3 scripts/check_vscode_test_registration.py
python3 scripts/report_test_class_completeness.py \
  --json-out target/gate-artifacts/verification/test-class-completeness.json \
  --markdown-out docs/internal/testing/evidence/plc-verification-program/<date>/p2-test-class-completeness.md \
  --timestamp <fixed-iso-8601-time>
python3 scripts/validate_test_class_completeness_report.py \
  --json target/gate-artifacts/verification/test-class-completeness.json \
  --markdown docs/internal/testing/evidence/plc-verification-program/<date>/p2-test-class-completeness.md
```

It inventories only source-derived facts, including runnable Structured Text
test declarations under crate test projects. Package values may be null,
command hints carry an authority label, ignore state distinguishes
unconditional from conditional attributes, and obvious references remain
lexical candidates.
The generated JSON explicitly excludes the hand-owned intent fields belonging
to `verification/test-catalog.toml`. Unmapped or unsupported declarations are
report debt in this slice; scanner corruption and invalid report structure are
errors.

Beginning with catalog schema v2, each committed record declares a closed
subject kind. Scanner-backed native tests bind a generated discovery ID, source
kind, path, and name; line movement is ignored, while rename, delete, move, or
source-kind drift fails. The Phase 1 case-table and bytecode mutation-runner
rows use narrow artifact subject kinds because they are outside the scanner
surface. They cannot be used to exempt ordinary native tests.

The VS Code registration checker requires every
`editors/vscode/src/test/suite/**/*.test.ts` file to have one direct literal
`require("./...test")` between Mocha's pre-require and run boundaries in
`suite/index.ts`. Orphans, missing targets, duplicates, dynamic/conditional
loads, and path escapes fail. It also scans all supported
`editors/vscode/src/test/**` sources and rejects any discovered VS Code fact
whose file is outside that registered set; the file inventory alone is not the
completeness assertion.

The test-class completeness report keeps two denominators separate. Scanner
classification counts only exact `discovery_id` joins from `generated_test`
catalog rows; case-table and mutation-runner artifacts cannot classify source
facts. Matrix completeness compares each mapped area's
`required_test_classes` with catalog rows in runnable statuses. Planned and
other non-runnable rows are listed but do not satisfy a required class.
Scanner-backed rows marked `ignored` or `conditional` also do not satisfy a
required class; Phase 3 still owns their reviewed classification. A
structurally `complete` report may therefore contain unmapped facts and missing
classes. Those are report debt, not an enforcement failure or permission to
infer area, class, invariant, oracle, or expected behavior from names. These
Phase 2 commands remain standalone; the report-only CI posture is unchanged.

The coverage-matrix gap report keeps declared coverage state separate from
structural absence. A missing invariant/family slot has no implied state, and
cataloged cases remain planning observations rather than proof. The malformed
input report uses only reviewed `malformed_input_class_ids` from the
bytecode/VM pilot taxonomy; it never infers a class from test text or location.
The unmapped-test debt report emits every remaining scanner identity rather
than only aggregate counts. All three artifacts bind clean source inputs,
reject symlinks and noncanonical/tampered output, and return success for honest
debt without enabling CI enforcement.

The P2-001 board surface is deliberately narrower than the whole workspace:
Rust tests under `xtask/**` and fuzz targets in crate-local fuzz workspaces such
as `crates/trust-ads-server/fuzz/**` are not included in generated totals. They
remain explicit completeness debt until a later reviewed scope row.
The live-repository census assertions pin the reviewed evidence baseline; an
organic test addition or removal requires an intentional count and evidence
refresh rather than a silent baseline move. These assertions are not a CI
enforcement ratchet in this slice.

Phase 3 overlays reviewed ignore intent without changing the Phase 2 catalog.
Its dedicated inventory keeps mechanical source identity, raw reason, state,
and mechanism separate from `verification/ignored-tests.toml`. The live checker
requires every ignored or conditional observation to resolve to exactly one
registry record and rejects stale or invented identities; line movement is not
semantic. An optional `test_id` is permitted only when it resolves to a catalog
row with the same discovery identity, so the initial uncataloged population does
not fabricate catalog mappings.

The initial register may use `unknown` where current behavior or quarantine
evidence is not established. That debt is visible and report-only until the
reviewed grace duration or milestone required by `VERIF-P14-000` exists.
Historical source text, paths, names, and lexical references never create a
`red_protective` or `flaky_quarantined` claim. Ignored and conditional tests do
not become runnable proof or coverage through this registry.

Phase 4 maps the 44 seed obligations written in `verification-areas.md` through
the schema-v2 `verification/invariant-seeds.toml`. The manifest uses explicit
aliases only where an existing canonical invariant already owns the same
obligation; names and prose similarity cannot create aliases. Lifecycle v1
keeps baseline seeds at their reviewed `gap_open`/`spec_gap`, S0 posture and
retains the original empty-association rule for Phase 4-created records. The
only reviewed `execution_ready` seed is `IEC_TIMER_001`; that authorization may
reference known, back-linked tests and evidence and may use only an
evidence-supported promotion. It permits `validated` only after the canonical
metadata validator accepts a specified active oracle, no current gaps or
missing obligations remain, every coverage cell is closed, linked tests and
evidence are non-empty and bidirectional, and promotion evidence supports the
claimed level. Earlier lifecycle states must keep every current gap explicit.
Lifecycle authorization itself creates no proof or gap closure. Five
review-derived risks retain their tracked source provenance without treating
historical fix evidence as proof of the registry claim.

The Phase 4A specification-completeness report keeps four debt surfaces
separate: unspecified/ambiguous invariant specs, expected-result tests without
an oracle or gap, `spec_gap` coverage cells, and the bytecode pilot's disjoint
spec-gap/test-gap denominator. The Phase 1A source audit now inventories every
rendered public-prose block in its reviewed surface closure, but 14,098 blocks
still lack hand-owned semantic dispositions. Public claims remain
registry-only context in the Phase 4A report, so `VERIF-P4A-005` stays open.

```text
python3 scripts/report_invariant_seed_audit.py --json-out <json> --markdown-out <md> --timestamp <time>
python3 scripts/validate_invariant_seed_audit_report.py --json <json> --markdown <md>
python3 scripts/report_spec_completeness.py --json-out <json> --markdown-out <md> --timestamp <time>
python3 scripts/validate_spec_completeness_report.py --json <json> --markdown <md>
```
