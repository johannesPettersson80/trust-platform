# Fourteen-Invariant Specification Closeout

Date: 2026-07-17

Specification and case checkpoint:
`fc651193d0ea662c0f8b061c7d2670e37ad788de`

## Scope

This record reviews the written specification and executable case-table inputs
for the final 14 S0 invariants. It closes no gap by itself and is not proof that
the cases passed. At this checkpoint the 11 affected gaps are
`resolution_status = "spec_updated"`; every affected invariant remains at S0
with its prior gap oracle.

The reviewed normative sources are:

- `SPEC_DEVELOPER_WORKFLOWS_001` for recursive `.st`/`.pou` discovery and
  project-scoped Git commit ownership;
- `SPEC_CONNECTOR_STATUS_001` for the closed connector state, health,
  freshness, confidence, and point-quality vocabulary;
- `SPEC_RELEASE_EVIDENCE_001` for platform, source-build, dependency, artifact,
  version, hardware, conformance, and behavior-lock release claims.

The 14 catalog rows resolve one-to-one to live Rust scanner facts and bind the
14 committed hand-authored case files by SHA-256. No filename, prose, or lexical
candidate creates an invariant mapping.

## Focused Checks

The following focused validation was run while constructing the checkpoint:

- `python3 -m unittest scripts.release_evidence_contract_tests scripts.release_artifact_generators_tests scripts.release_claim_contract_tests` (8 passed);
- `python3 -m unittest scripts.verification.invariant_seed_audit_tests scripts.verification.fuzz_program_source_contract_tests scripts.release_evidence_contract_tests scripts.release_artifact_generators_tests scripts.release_claim_contract_tests` (41 passed);
- `python3 scripts/validate_verification_metadata.py` (666 records);
- `python3 scripts/check_test_catalog_staleness.py` (228 committed records
  against 4,009 scanner facts);
- `cargo test -p trust-dev commit::tests:: -- --nocapture` on `trust-builder`
  (7 passed);
- `cargo test -p trust-dev developer_test_discovery_trace_cases -- --nocapture`
  on `trust-builder` (1 passed);
- `cargo test -p verification-cases --test release_evidence_trace_cases -- --nocapture`
  on `trust-builder` (11 passed);
- `cargo test -p trust-runtime --test protocol_status_trace_cases ui_connector_status_trace_cases -- --nocapture`
  on `trust-builder` (1 passed);
- `cargo check --locked -p trust-lsp -p trust-runtime -p trust-debug -p trust-dev`
  on `trust-builder` (passed without a sibling OpenOT source dependency);
- `npm audit --audit-level=low` under `editors/vscode` on `trust-builder`
  (0 vulnerabilities);
- `npm run compile` plus
  `npx mocha --ui tdd out/test/suite/connector-status-contract.test.js` on
  `trust-builder` (2 passed);
- `git diff --check` (passed).

Broad Rust, extension-host, and documentation gates are intentionally deferred
to the single final batch checkpoint.

## Defects Found

The specification-first cases and audits found three product/toolchain defects:

1. `trust-dev commit` could absorb a caller-owned pre-staged path inside the
   selected project. It now aborts before index or history mutation, while an
   out-of-scope staged path remains untouched.
2. VS Code connector projection accepted arbitrary backend state and health
   strings. It now rejects values outside the shared canonical vocabulary.
3. The committed VS Code lockfile resolved 13 known dependency advisories,
   including two critical and six high. Direct dependency updates plus two
   compatibility-tested transitive overrides reduce the audited count to zero,
   and CI/release packaging now run the audit.

The source-build audit also found and fixed a checker error: it had assumed the
lockfile contained exactly three OpenOT packages, although two additional
support packages are legitimately pinned to the same public Git revision. The
contract now requires the shipped subset and rejects a sibling/path source for
every OpenOT package.

## Boundaries

- No affected invariant is promoted by this record.
- No proof evidence is created by this record.
- No CI verification enforcement mode is flipped.
- No IEC decision or deviation is created because these are developer,
  connector-status, and release contracts rather than IEC execution semantics.
- `VERIF-P16-007` and the standing stop rows remain open.
