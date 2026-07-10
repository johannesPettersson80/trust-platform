# Test-Class Completeness Report

Generator: `test-class-completeness v1`
Source revision: `c23ebe993c1e2bfa4cec2e865fc0cdebcfed3fd2`
Generated: `2026-07-10T14:00:00Z`
Platform: `linux-aarch64`
Generated JSON SHA-256: `02efbd5f847d3f84a4c881acdac6235d0f9b8ecc972f046e2c47da35ae9f7bd7`
Input SHA-256: `sha256:2de81013c52665704526c0c858e8cd8a0455ba48d12b7e8e6f66973f3c034048`

`complete` means the report was generated and bound successfully. It does not
mean every scanner fact or required test class is mapped.

## Summary

- Scanner facts: 3816
- Classified scanner facts: 1
- Unmapped scanner facts: 3815
- Catalog records: 6
- Runnable catalog records: 2
- Non-runnable catalog records: 4
- Mapped areas: 1
- Required class slots: 5
- Complete required class slots: 2
- Missing required class slots: 3

## Scanner Classification

| Source kind | Facts | Classified | Unmapped |
| --- | ---: | ---: | ---: |
| `conformance_case` | 21 | 0 | 21 |
| `fuzz_target` | 2 | 0 | 2 |
| `gate_script` | 29 | 0 | 29 |
| `github_workflow_job` | 30 | 0 | 30 |
| `rust_integration_test` | 1369 | 1 | 1368 |
| `rust_unit_test` | 1652 | 0 | 1652 |
| `structured_text_test` | 257 | 0 | 257 |
| `vscode_test` | 456 | 0 | 456 |

Classified mappings:

- `DISC_88F921D24D3708CEF3E1` -> `TEST_BYTECODE_CONTAINER_INVALID_MAGIC`

## Area: `bytecode_vm`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `failing_regression` | none | none | no |
| `iec_conformance` | none | none | no |
| `metadata_validation` | none | `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001` (planned; catalog_status:planned), `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001` (planned; catalog_status:planned), `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001` (planned; catalog_status:planned), `TEST_CASE_TABLE_VM_SEAM_VALID_001` (planned; catalog_status:planned) | no |
| `mutation` | `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001` | none | yes |
| `negative_malformed_input` | `TEST_BYTECODE_CONTAINER_INVALID_MAGIC` | none | yes |

## Limitations

- A scanner fact is classified only by an exact discovery_id on a generated_test catalog row.
- Case-table and mutation-runner artifacts never enter the scanner-fact classification denominator.
- A required class is complete only when the mapped area has at least one catalog row in a runnable status.
- Planned and other non-runnable rows remain visible but do not satisfy required-class completeness.
- An unmapped fact or missing class is report debt, not proof that no relevant executable test exists.
- The report never infers area, class, invariant, oracle, or expected behavior from names or lexical references.
- Generated tests marked ignored or conditional by the scanner do not satisfy runnable class completeness.
- Scanner exclusions remain those documented by the generated existing-test catalog.
- Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.

## Review Fixes Folded In

- The VS Code registration audit now performs the scanner-fact join itself. It
  rejects a discovered test fact outside `suite/**` and a discovered JavaScript
  test file that the TypeScript file inventory cannot register. The live result
  is 456 discovered facts, 38 files, and 38 registrations.
- Catalog `subject_kind`, `discovery_source_kind`, and `test_class` enums are
  checked from a dedicated schema-contract module. This reduced
  `metadata_validator/core.py` from 995 to 980 lines.
- A structurally complete report can contain debt, but cannot relabel it:
  summary, per-source counts, mapped-area classes, non-runnable reasons, input
  digest, canonical command, ISO timestamp, clean source inputs, and generated
  Markdown are revalidated at rest.
- Generated facts with `ignore_state = "ignored"` or `"conditional"` remain
  classified when review-owned identity exists, but cannot satisfy an
  effectively runnable required-class slot. Phase 3 still owns their reviewed
  ignore classification.

## Tests First

Before implementation, the registration suite failed twice because out-of-suite
TypeScript and JavaScript test facts incorrectly passed, and errored because the
audit had no fact count. The schema-contract and completeness suites each failed
to import their not-yet-created modules. Later adversarial fixtures were added
for metadata-validator wiring, schema widening, ignored facts, forged command
and timestamp values, nonexistent/pre-feature commits, semantic count edits,
and JSON/Markdown digest tampering.

## Honest Debt Boundary

The report makes no proof claim. Only
`TEST_BYTECODE_CONTAINER_INVALID_MAGIC` classifies a mechanical scanner fact.
The bytecode-validator mutation runner is a runnable catalog artifact but never
classifies a source fact. The four planned case tables remain non-runnable.
No area, test class, invariant, oracle, expected result, or malformed-input
class is inferred from source text.

`VERIF-P2-008` remains open. `VERIF-P2-009` also remains open because the
catalog does not yet have a reviewed machine binding from negative tests to the
surface-specific malformed-input taxonomy. This report contains aggregate and
per-source unmapped counts, not all 3,815 individual debt identities, so
`VERIF-P2-010` remains open as well.

## Validation

Clean-source generation and independent at-rest validation ran locally from
commit `c23ebe993c1e2bfa4cec2e865fc0cdebcfed3fd2`:

```text
python3 scripts/report_test_class_completeness.py \
  --json-out target/gate-artifacts/verification/test-class-completeness.json \
  --markdown-out docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-test-class-completeness.md \
  --timestamp 2026-07-10T14:00:00Z
python3 scripts/validate_test_class_completeness_report.py \
  --json target/gate-artifacts/verification/test-class-completeness.json \
  --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-test-class-completeness.md
```

Both commands passed. The generated JSON SHA-256 is
`02efbd5f847d3f84a4c881acdac6235d0f9b8ecc972f046e2c47da35ae9f7bd7`.
The focused validation command recorded in `verification/evidence-index.toml`
is rerun after this evidence row is indexed.

Remote focused validation ran from a temporary clean worktree at the same
implementation commit on `trust-builder`:

```text
just fmt
python3 -m unittest \
  scripts.verification.adversarial_selftest_tests \
  scripts.verification.report_gate_tests \
  scripts.verification.prover_tests \
  scripts.verification.metadata_validator.evidence_proof_tests \
  scripts.verification.bytecode_transforms_tests \
  scripts.verification.bytecode_validator_mutation_tests \
  scripts.verification.test_catalog_scanner_tests \
  scripts.verification.test_catalog_intent_tests \
  scripts.verification.test_catalog_staleness_tests \
  scripts.verification.test_catalog_vscode_registration_tests \
  scripts.verification.metadata_validator.schema_contracts_tests \
  scripts.verification.test_class_completeness_tests
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
python3 scripts/check_test_catalog_staleness.py
python3 scripts/check_vscode_test_registration.py
python3 scripts/report_test_class_completeness.py \
  --json-out target/gate-artifacts/verification/test-class-completeness.json \
  --markdown-out target/gate-artifacts/verification/test-class-completeness.md \
  --timestamp 2026-07-10T14:00:00Z
python3 scripts/validate_test_class_completeness_report.py \
  --json target/gate-artifacts/verification/test-class-completeness.json \
  --markdown target/gate-artifacts/verification/test-class-completeness.md
git diff --check
git status --short --branch
```

Results: 154/154 tests passed in 20.319 seconds; both metadata commands
validated 101 implementation-commit records; staleness was 6/3,816;
registration was 456 discovered facts, 38 files, and 38 registrations; report
generation and at-rest validation passed; the worktree stayed clean. Broad
remote clippy/test/test-all were not run: the pre-run builder had 13 GB free
after an unrelated active `test-all`, below the documented thresholds for those
gates. The temporary worktree and transfer bundle were removed after validation;
the final disk check showed 60 GB free on `/home/johannes` and 3.6 GB on `/tmp`.

No product/runtime source, extension product source, workflow, suite,
specification gap, invariant status, skill, or agent instruction changed. CI
remains report-only; `VERIF-P1B-012`, `VERIF-P1B-014`, `VERIF-P2-008` onward,
`VERIF-P10-001`, and `VERIF-P10-003` remain open.
