# Bytecode Validator Contract Validation

- Date: 2026-07-15
- Test implementation revision: `38b6d09592913e373441511ccc8b5411938dc63f`
- Specification closeout revision: `988ccb47942a20dc99127faabac29eed48016365`
- Final report source revision: `de94ec228fe9ff07f015e9395a39d7282c37371b`
- Final report evidence revision: `3778447b604cf0e5ccabcc2eeb5329d869440991`
- Evidence posture: specification, characterization, mutation adequacy, and
  broad regression validation; no producer-authentic red/green proof.

## Outcome

The bytecode validator contract is specified and its focused tests are active.
The product-path case runner, direct decoder and validator tests, and existing
Phase 11 seam tests all rejected the reviewed malformed inputs. No product
acceptance defect reproduced, so this batch contains no runtime product fix and
does not manufacture a red/green proof pair.

`SPEC_GAP_BYTECODE_VALIDATOR_001` is closed. The stable public error-identity
contract remains open under `SPEC_GAP_VM_ERROR_MODEL_001`, and numeric resource
budgets remain open under `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001`.

The clean bytecode-validator mutation rerun at implementation revision
`38b6d09592913e373441511ccc8b5411938dc63f` used `cargo-mutants 27.0.0` and
reported 2 caught, 0 survived, 0 unviable, 0 timeout, and 0 infrastructure
error. Its committed report SHA-256 is
`329a2d46a14eb0c1f29c9dfac6f94ba112529a0daecb3b48f47038f429db9dc8`.

## Focused Validation

Local validation on Linux aarch64:

- `python3 scripts/run_verification_focused_tests.py`: 772 tests passed in
  756.661 seconds;
- `scripts/verification_metadata_gate.sh`: 515 records before this evidence
  row was indexed, with the Phase 16 fence green for zero changed paths;
- `python3 scripts/check_verification_tooling_selftests.py`: 33/33 fixtures;
- ignored-test join: 29 discovered, 29 registered, 5 unknown, 0 catalog-mapped;
- catalog staleness join: 179 committed records against 3,948 scanner facts;
- VS Code registration: 456 facts, 38 files, 38 registrations; and
- refactor proposals: 1 proposal, 0 redirects, 179 catalog records, 3,948
  scanner facts.

The first complete focused run failed five reviewed census tripwires after the
new tests and specification state changed the live denominators. The expected
values were updated only after the live reports established the new counts; 72
targeted tests then passed before the complete 772-test rerun.

Broad validation ran once on the clean trust-builder checkout at
`5202fdb27304d5e30880cdc608427a09b934ba2c`:

- `just fmt`: passed in 2.368 seconds;
- `just clippy`: passed in 79.861 seconds; and
- `just test-all`: passed in 625.598 seconds.

The later changes through the report source revision are Python census-test
baselines and generated verification evidence only; no compiled product or Rust
test source changed after the broad checkpoint. The generated builder target
was removed after the run, restoring 68 GiB free space, and the validation
worktree remained clean.

## Commit-Bound Reports

All 15 report pairs were generated and validated at rest on trust-builder from
the clean source revision `de94ec228fe9ff07f015e9395a39d7282c37371b`
with timestamp `2026-07-15T20:06:12+02:00`. The staged artifact archive SHA-256
was `1d5231531901402c25174d97eb46e66cb57a470e5681eaf6bd4a395b23e68d15`.

| Report JSON | SHA-256 |
|---|---|
| `test-class-completeness.json` | `62bd189dde82e09513ec6dfa76ace6e09b817270c1d894156aabd2ba1bc6902c` |
| `coverage-matrix-gaps.json` | `a0aff61ce384217985e1f04a4ac71e74c0b9b7fd5a69362ea4a7e768622e4e41` |
| `malformed-input-coverage.json` | `530a4fb33f55526372f6dfb47fe96756bcfdf9afb971f11e21b91e2ec4caa4f7` |
| `unmapped-test-debt.json` | `6d9a8a12c39b004396cc0a26480757e01b2af18045ac389b40f3dbd64fc86fb5` |
| `test-refactor-assessment.json` | `a030862ebe5ca2423e05e7ecc5877cf5d3705b8275832b0a1de499adbbc5e466` |
| `ignored-test-inventory.json` | `87674e80dd09e9a317f05f4e4938ab1f5d9a187d49a6c41e004fed8c2b13d82e` |
| `phase5-suite-audit.json` | `3c9d544904e2ab24be3c758bc93e43be0a0f3bf4198784e05219f6b61e01f945` |
| `invariant-seed-audit.json` | `0fda3ce067355b5169a9a55797e7f93f86c658d36e39659d023c4a6772d6ba37` |
| `spec-completeness.json` | `6ceb04d40caad91a18419ebd4c99417f64f8fe190a0ecb2dffdeea8d7986373d` |
| `requirement-oracle-audit.json` | `388b9a9413237f9cb5116d487861406a3e1f729f46b449935ec0b81bb848db77` |
| `conformance-alignment.json` | `5079347a6a63dcfc0e861ba3846305519d5bdabba79b5a01fa0b272d2c197c49` |
| `runtime-anomaly-audit.json` | `cf4c61d60b5bb6d70f7576db23909bb94148cd3ff44261b2f916799e56887060` |
| `fuzz-program-audit.json` | `ac50fc84cd198cac5eceaf7d87e37e54b86c91baa01cff3631e94243daa8460f` |
| `p10-mutation-survivor-report.json` | `9903193c160f9014895160c3ba7c8ab158b7c90662d21a964660fb9e251a4157` |
| `spec-source-audit.json` | `780f0d811425618a67d2ff87a8209423f4f29568247092ad8c0f26c7c5e613b3` |

## Preserved Boundaries

The closeout creates no proof, promotes no invariant, changes no CI or suite
enforcement, and changes no approved proof producer, skill, or agent
instruction. `VM_SEAM_VALID_001`, `VM_SEAM_OWNER_001`, and
`VM_SEAM_REF_001` remain at `S0`. The five resource-limit malformed-input
classes and the public error-model gap remain visible debt.
