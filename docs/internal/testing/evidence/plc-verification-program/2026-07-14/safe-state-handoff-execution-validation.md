# Runtime Safe-State Handoff Execution Validation

Date: 2026-07-14

Validated report commit: `57f08a73e180936ed4336e32cf22219faa3fe19c`

## Outcome

This vertical found and fixed a product defect. `IoSubsystem::apply_safe_state`
treated a successful driver write call as confirmed delivery even when the
driver immediately reported degraded or faulted health. A stopped resource
could therefore overstate safe-state delivery while a Modbus or MQTT worker was
still pending, reconnecting, or down. An early write failure could also prevent
later configured drivers from receiving the safe-state image.

The contract is now written in `docs/specs/11-runtime-engine.md`. The runtime
attempts every configured driver and accepts the handoff only when the write
succeeds and the immediately observed driver health is `Ok`. Any unconfirmed
handoff faults deliberate stop with a named error.

The hand-authored case file
`verification/cases/runtime_safety/RT_SAFE_STOP_001.toml` contains three traces
and is executed by `TEST_RUNTIME_SAFE_STATE_HANDOFF_001`. Separate supporting
regressions exercise generic degraded health, a pending Modbus worker, and a
connecting MQTT worker.

`SPEC_GAP_RUNTIME_SAFE_STATE_001` is closed. `RT_SAFE_STOP_001` and
`RT_SAFE_IO_WORKER_001` are implemented at G2. Real output-module delivery and
worker-disconnect hardware proof remain explicitly missing and are not claimed
by this slice.

## Causal Chain

| Step | Commit or evidence |
| --- | --- |
| Specify confirmed safe-state handoff | `195c1add7f05801269975a993d809c3991cadfdb` |
| Reproduce unconfirmed delivery | `5be87c43e8c5a5d54c7792f411a4a4974214c9db` |
| Apply the minimal runtime fix | `48d90c11d5873c5945e702a87f310d0ba7440def` |
| Bind case file and cataloged runner | `db33adcf59218452e9f4ba1ff21dfdffeb808016` |
| Producer-authentic red proof | `EVID_TEST_RUNTIME_SAFE_STATE_HANDOFF_001_RED` at `7c8d3c00a7a5c6e1702aee75762373be6d73c9c4` |
| Producer-authentic green proof | `EVID_TEST_RUNTIME_SAFE_STATE_HANDOFF_001_GREEN` at `d9222d3128b72fbdf99cf751f5c5c263499f6279` |
| Close the specification gap | `c5268f90aff4e2ac7272c66c53fcb8c051fb34b3` |
| Broad PR gate evidence | `EVID_BROAD_REMOTE_PR_20260714_30F1B77C35BE` |
| Promote both invariants to G2 | `374c949273b353ddb5cc953eb97f29235517f01b` |
| Final report source checkpoint | `1b9eed363c47a97c5da2ced75562456c58a8cea2` |
| Report rebind commit | `57f08a73e180936ed4336e32cf22219faa3fe19c` |

The red artifact failed all three committed cases. The paired green artifact
used the same case-file, trace-definition, and execution-contract digests and
passed all three.

## Validation

On `trust-builder`, the broad producer ran from clean commit
`b18002106162188c4721a070c01eb91a8035ed3c`:

- `just fmt`: passed.
- `just clippy`: passed.
- `just test-all`: passed.
- Cataloged safe-state trace: 3/3 passed.
- Gate duration: 947,787 ms.
- Disk preflight: 99,471,356 KiB under the home filesystem and 3,287,812 KiB
  under `/tmp`.

The required runtime verticals passed at the G2 commit:

- `cargo test -p trust-runtime --test api_smoke`: 3/3.
- `cargo test -p trust-runtime --test debug_control`: 20/20.
- `cargo test -p trust-runtime --test complete_program`: 1/1.
- `cargo test -p trust-runtime --test runtime_reliability`: 4/4.

All 15 report generators and their at-rest validators passed on
`trust-builder` from clean source commit
`1b9eed363c47a97c5da2ced75562456c58a8cea2` with timestamp
`2026-07-14T12:31:26+02:00`. The imported JSON artifacts were checked against
the builder manifest, metadata validation passed with 398 records, and
`git diff --check` passed before the report rebind commit.

## Report Digests

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `4fc4088a95a0071348085d40e23dbac23128b6a51b6def7bb3a42aa402151b94` |
| Coverage-matrix gaps | `d87f871d83247d8dae1be5573e6f7d17e4b6dff10e742e3c74eb8dadc9485d0b` |
| Malformed-input coverage | `6cc1f94ffadd894d2cc27c1ce1da7e10e31d330740f6e51e915b0559d7ed05c1` |
| Unmapped-test debt | `5175199c8c436277dfde8f7fba8056d2e1225d7d542527691e919b443775e205` |
| Test-refactor assessment | `7d4a1c3e9b0d4bbc1e1c2b8bcba0dc810b5cc31be3c7f0dee1320c19288b7d65` |
| Ignored-test inventory | `26cdd713254d060d0f667626e309fb9480f042f8523d3170ad8b087ce61875ed` |
| Invariant-seed audit | `635a082a655a388b6e29e2143d0da430a764feab110cf369208f434506789cbf` |
| Specification completeness | `9da83c17bc47afc665f39d6a42c73c9814d37ce87c2a9eb5b05ee6128502b8bc` |
| Phase 5 suite audit | `1149e37f1aae50339376841085ab3f20c145d89513d38d48945b2b59c0034c9a` |
| Requirement/oracle audit | `f6418337bf149fdce6d15c2d7528c4dacdbdf8816f21bf890946cbfb4049bdbe` |
| Conformance alignment | `47971d181b863556bd9976a86980529f7c343f18cec16f3576162b8c67119772` |
| Runtime-anomaly audit | `e31cb0365d655980164bb2e453215e4d2248f70a095a1605ae1de0187b1905f3` |
| Fuzz-program audit | `6da878df2098909f115831b1ea3dc27d95e171325c344ceae2faa35e9ce77fa0` |
| Mutation program | `9f5ec65a158bd6de5028697662ede472455f19bc579c73de4804ed7932b5db80` |
| Specification-source audit | `15adfb37f19a4fea85a0e5c13f71c03b133cb368bd9c294d16c2077118dd44f6` |

## Remaining Posture

- Specification gaps: 34 total, 30 open, 4 closed.
- Invariants: 53 total, 46 at S0 and 7 at G2.
- `RT_SAFE_STOP_001` still needs a real-output-module hardware run.
- `RT_SAFE_IO_WORKER_001` still needs a worker-disconnect hardware run.

These debts remain visible and are not closed by the safe-state software proof.
