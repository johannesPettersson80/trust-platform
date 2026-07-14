# IEC timer duration-overflow batch validation

Date: 2026-07-15

Final source checkpoint: `d1c5ec7bc1d70770a079969bf4d5c443a55daf1e`

## Outcome

This batch found and fixed a product defect in the shared TON, TOF, and TP
elapsed-time arithmetic. Advancing a timer from `i64::MAX - 1` nanoseconds by
two nanoseconds overflowed the signed duration representation. Debug builds
panicked; an optimized build could wrap the elapsed value instead of reaching
the preset.

The existing written contract already requires TP elapsed time to stop at PT,
TON to transition after PT, TOF to reach and hold ET at PT, and TIME/LTIME
variants to use the same state machines. The applicable basis remains IEC
61131-3 Ed.3 section 6.6.3.5.5, Table 46, and Figure 15 as recorded in
`docs/specs/08-standard-function-blocks.md`. This is not an IEC deviation and
did not require a new behavior decision.

## Tests first and product fix

Commit `61baa413c5187ba47a936487e4aba6c5a210dd49` added three focused regression
tests:

- `TEST_IEC_TIMER_TON_DURATION_OVERFLOW_001`
- `TEST_IEC_TIMER_TOF_DURATION_OVERFLOW_001`
- `TEST_IEC_TIMER_TP_DURATION_OVERFLOW_001`

On the clean trust-builder checkout at that commit, the focused command ran all
three tests and all three failed at the raw elapsed-time addition with
`attempt to add with overflow`.

Commit `de860c373596d06ec05abcfc9fd4536fd4cd71ee` introduced one shared
`accumulate_elapsed` helper. It uses signed saturating addition and then clamps
the result to the normalized PT. TON, TOF, and TP all use that helper; no timer
keeps a separate overflow policy.

The same commit expanded the real Structured Text `timer_variants` integration
test to execute TON, TOF, TP, TON_LTIME, TOF_LTIME, and TP_LTIME. The cataloged
row is `TEST_IEC_TIMER_RUNTIME_VARIANTS_001`.

The initial focused verification run correctly exposed two stale census
tripwires caused by the three new Rust facts and four new catalog rows. Commit
`d1c5ec7bc1d70770a079969bf4d5c443a55daf1e` refreshed only those expected
counts. No production behavior changed in that follow-up.

## Validation

On `trust-builder`, using the isolated clean checkout
`$HOME/projects/trust-platform-timer-overflow` and shared target
`$HOME/.cache/codex-targets/trust-platform-gate`:

- The red focused run failed 3/3 before the product fix.
- The debug focused run passed 3/3 after the fix.
- The release-profile focused run passed 3/3 in 622.30 seconds.
- The real-ST TIME/LTIME timer-variant test passed 1/1.
- `just fmt` passed in 2.06 seconds.
- `just clippy` passed in 189.11 seconds.
- `cargo test -p trust-runtime --test api_smoke` passed 3/3.
- `cargo test -p trust-runtime --test debug_control` passed 20/20.
- `cargo test -p trust-runtime --test complete_program` passed 1/1.
- `cargo test -p trust-runtime --test runtime_reliability` passed 4/4.
- The canonical focused Python verification suite passed 767/767.
- `just test-all` passed at the final source checkpoint in 629.05 seconds.

The final catalog staleness join reported 137 committed rows against 3,917
scanner facts. The refreshed runtime-anomaly audit reports 19 classes, 41
explicit associations, and 8 remaining test-gap classes. The
`timer_duration_overflow` class moved from missing to three runnable direct
associations. Each association explicitly excludes host-clock arithmetic,
restart, PT-change, and skipped-call claims.

## Generated report refresh

All 15 installed report pairs declared at least one changed batch input. They
were regenerated and passed their production at-rest validators from the clean
source checkpoint with timestamp `2026-07-15T01:22:00+02:00`.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `7e31b3ab45bb15748c73fda4b78939fba51074d2c39cbd4b8297745ac42df5f1` |
| Coverage-matrix gaps | `c3dbeab3b3d7b4ee13003daf1ceece5f754af1058f7c3a43c9ce07fb6eff7618` |
| Malformed-input coverage | `c393d8bdec5afb561cdd2c0f29f02d0396165c6a0e17f623bd5656ee366a4c60` |
| Unmapped-test debt | `febeac98efc68ae6ce444542b69f0a20a06dfd86781b81123c5584fc03942042` |
| Test-refactor assessment | `05ebadfddf1383e88a8f8018e9f41ad55def1f18794105e982441eca16a4aa73` |
| Ignored-test inventory | `cbcf62610e6b473806dabd83a25771ef10719dd4078eef592a1b02c854d26e2a` |
| Phase 5 suite audit | `086aa2ae5d21b17579566f6b1cf734c0517034d4db257c01c3316de71d5b64c3` |
| Invariant-seed audit | `da0809282b565f45adda4139907992a3a9a5f294d47efdd146c2ee4fc46e6831` |
| Specification completeness | `f35ff09e2fa52304aa62b0b1f5dc623cbb411e33079e72a46632eafa41697cb4` |
| Requirement/oracle audit | `9e1df96145da88b1ccfd80be2ec3a6d4cbccf7314fe86142d70a3eae7f337e18` |
| Conformance alignment | `d4244b7f0fa2ffccbe075c21c30718c5ad54f7536555f2bb91e8f9d40909d47b` |
| Runtime-anomaly audit | `f03c720534e664dd471c86f1f7c47adbc75e2b09cbcacfcfba4b07ce431deaa8` |
| Fuzz-program audit | `93415cc250f6ddcd8e4f2b3fb33d171f3f1b96cb3459bcf881edad95261e06c0` |
| Mutation program | `75ca006e09cd8aa23981cc4f395cc127ca790c032a6db01bc59627adbd269b8d` |
| Specification-source audit | `2287c998c1408499354d018ffd16196e0f4d4e94c561f5d5dba8ffdefa390339` |

The mutation program report was rebound because its declared input closure was
stale. The measured bytecode-validator shard was not rerun: no mutation
selector, mutation runner, validator implementation, or measured outcome
changed.

## Honest remaining posture

- `IEC_TIMER_001` remains at G2; this manual regression record creates no new
  proof and does not alter the previously reviewed proof contract.
- The invariant's `time_or_clock_fault` cell remains `gap_open`. Restart,
  PT-change, skipped-call, and nonmonotonic-clock adequacy are not claimed by
  these state-machine overflow tests.
- No specification gap is closed by this batch.
- The refreshed program still reports 28/53 invariants not specified, 29/53
  without an eligible oracle, 63/80 required coverage slots missing, and 3,785
  of 3,917 scanner facts unmapped.
- CI remains report-only. No suite or workflow changed.
- The release metadata is synchronized at version 0.24.45. Tagging and public
  release remain deferred until the change reaches `main`.
