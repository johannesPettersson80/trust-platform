# Phase 5 Suite, Gate, And Routing Closure Validation

Date: 2026-07-10
Implementation commits: `24bb3685e`, `8511e3e2c`
Mutation source commit: `8511e3e2ca14e8c575f4442a44fe0051af4b522c`
Mutation evidence commit: `f58b9c141`
Report and board checkpoint: `c5e8e09ace5c6232433fbf939dd97fc17f5ff514`

## Scope

This closure covers `VERIF-P5-000`, `VERIF-P5-000A`, and
`VERIF-P5-001` through `VERIF-P5-010`. `VERIF-P5-000B` remains open until
Phase 11 builds the reviewed hardware-lab program on the existing
device-in-loop sources.

No runtime, VM, compiler, IDE, extension, CI workflow, skill, agent instruction,
version, release, or product-test behavior changed. CI remains report-only.
No specification gap closed and no proof was created.

## Review Fix

The canonical focused runner now discovers all verification `*_tests.py`
modules instead of maintaining a hand-written module list. It finds 36 modules
and includes the six Phase 3 review-fix tests in
`ignored_test_js_skip_lexical_tests` and
`ignored_test_source_contract_tests` that the prior recorded command omitted.

## Implemented Contracts

- The exhaustive gate inventory contains 62 records: 59 live scanner records,
  one nested workflow template, one exact just recipe, and one catalog-bound
  mutation command.
- Six concrete suites bind 33 direct commands. Command ownership, duration,
  environment, artifacts, enforcement, exact recipe bodies, workflow outputs,
  release evidence, and hardware strictness are revalidated from source.
- `hardware_lab` accepts only the strict
  `TRUST_DIT_REQUIRE_HARDWARE=1` script as an entrypoint. The skip-capable
  hosted workflow remains a helper.
- The matrix contains all 11 canonical areas and 29 ordered routes. Specific
  routes precede area fallbacks; unsafe and unmatched paths default-deny;
  deletions and both rename endpoints are retained; conditional suites remain
  separate from direct requirements.
- `just verification-veryquick` is a bounded trust-builder recipe and is not
  wired into CI by this slice.
- New validation logic is split by responsibility across focused-suite,
  inventory, suite-contract, area-routing, report, live-state, and at-rest
  modules. No new god file was created.

## Generated Reports

All eight reports were generated independently from pristine detached
checkouts at `f58b9c1412b5002e26ea0b05de54d8b10b0bca46` with timestamp
`2026-07-10T21:10:00Z`, then validated at rest.

| Report JSON | SHA-256 |
| --- | --- |
| `test-class-completeness.json` | `52a269ba9059a9c81ec6b465015a63e21bfab093639f0d6c004a7dabb7bae812` |
| `coverage-matrix-gaps.json` | `4b63966835b4a86dde82eb88c06ef6c385d51f086260a2b7a535f870b7564d67` |
| `malformed-input-coverage.json` | `25e566b812f8935beb6a6f6afdac39f3097dcb9d5a4b4daf1b047eb9da66b540` |
| `unmapped-test-debt.json` | `139ff754fd7a372d8b5917219a9f8ba0cc1cbc86bf8c17b3a59af6c18f598005` |
| `test-refactor-assessment.json` | `f26e9d65d31d1a71b44c2adb08515852d228b62a4d399f1d0aa1eb7132e139ed` |
| `invariant-seed-audit.json` | `68e008badbac90b2435b347f88bc24a3e5edc9e2554c96067e38cf18bba594b9` |
| `spec-completeness.json` | `7c908f30968c82d1092afc735b1026a8b0f285185623fae1a6963bfcde5e6938` |
| `phase5-suite-audit.json` | `4400a2c4d157107e21d54d6291ed84d71cf9dc0de85def212535ed5f7c99651f` |

The bytecode-validator mutation shard was rerun on `trust-builder` against
clean implementation commit `8511e3e2ca14e8c575f4442a44fe0051af4b522c`.
Report SHA-256:
`fd8b7a7ab1f73b639b198678782072dee19dd5c2f0f9b19fb945dedf22069d4a`;
2 caught, 0 survived, 0 unviable, 0 timeout, and 0 error.

## Local Validation

The canonical focused command discovered 36 modules and passed 414/414 tests
in 103.525 seconds after this closure row was indexed:

```text
python3 scripts/run_verification_focused_tests.py
```

Additional passing checks:

```text
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
python3 scripts/check_ignored_test_staleness.py
python3 scripts/check_test_catalog_staleness.py
python3 scripts/check_vscode_test_registration.py
python3 scripts/validate_test_refactor_proposals.py
python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_DECLARED_TYPE_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_STRING_BOUND_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_VALID_001 --check
python3 scripts/validate_test_class_completeness_report.py --json target/gate-artifacts/verification/test-class-completeness.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-test-class-completeness.md
python3 scripts/validate_coverage_matrix_gap_report.py --json target/gate-artifacts/verification/coverage-matrix-gaps.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-coverage-matrix-gaps.md
python3 scripts/validate_malformed_input_coverage_report.py --json target/gate-artifacts/verification/malformed-input-coverage.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-malformed-input-coverage.md
python3 scripts/validate_unmapped_test_debt_report.py --json target/gate-artifacts/verification/unmapped-test-debt.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2-unmapped-test-debt.md
python3 scripts/validate_test_refactor_assessment_report.py --json target/gate-artifacts/verification/test-refactor-assessment.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p2a-test-refactor-assessment.md
python3 scripts/validate_invariant_seed_audit_report.py --json target/gate-artifacts/verification/invariant-seed-audit.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p4-invariant-seed-audit.md
python3 scripts/validate_spec_completeness_report.py --json target/gate-artifacts/verification/spec-completeness.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p4a-specification-completeness.md
python3 scripts/validate_phase5_suite_audit_report.py --json target/gate-artifacts/verification/phase5-suite-audit.json --markdown docs/internal/testing/evidence/plc-verification-program/2026-07-10/p5-suite-gate-routing-audit.md
git diff --check
```

Before this closure row was indexed, both metadata entrypoints validated 321
records. After indexing it, both validated 322 records.

Observed live joins: 88 ignored facts / 88 records / 63 unknown / 0
catalog-mapped; six catalog rows against 3,816 scanner facts; 456 VS Code facts
in 38 files and 38 registrations; one refactor proposal and zero redirects.

Planner probes preserved the intended routing posture: the bytecode-validator
path required `pr` with conditional `nightly` and retained five open gaps; the
Neovim path default-denied; the VS Code documentation path remained
uninventoried rather than inventing product-test requirements.

## Remote Gates

The retained isolated clone on `trust-builder` was clean at
`c5e8e09ace5c6232433fbf939dd97fc17f5ff514`. Rust and Cargo were 1.95.0 on
`x86_64-unknown-linux-gnu`.

```text
cd "$HOME/projects/trust-platform-p5-validation-8511e3e2c"
export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-p5-final"
export TMPDIR="$HOME/.cache/codex-targets/trust-platform-p5-final-tmp"
just fmt
just clippy
just verification-veryquick
just test-all
```

`just fmt` passed in 2.43 seconds. `just clippy` passed in 175.30 seconds.
`just verification-veryquick` passed in 225.27 seconds. `just test-all`
passed in 580.44 seconds with no failed test target. The clone remained clean
after all gates.

Disk preflight found 56G free under `/home/johannes` and 2.4G under `/tmp`.
A home-backed `TMPDIR` was used. Only generated targets owned by this
validation were eligible for cleanup; unrelated worktrees and targets were
left untouched. The isolated target reached 49G and left 6.8G free after the
successful gates. It and its temporary directory were then deleted, restoring
56G free under `/home/johannes`; the retained source clone remained clean.

## Preserved Boundaries

- `VERIF-P1B-012`, `VERIF-P1B-014`, `VERIF-P3-006`, `VERIF-P4A-005`,
  `VERIF-P5-000B`, `VERIF-P10-001`, and `VERIF-P10-003` remain open.
- All 34 specification gaps remain open.
- All 52 invariants remain unvalidated at S0.
- CI remains report-only and no skill or agent instruction changed.
- This evidence uses `proof_kind = "none"` with no linked tests, invariants, or
  specification gaps.
