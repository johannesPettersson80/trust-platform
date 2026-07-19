# Phase 16 Execution Slice 001 Timer Gap Closeout

Date: 2026-07-12
Source checkpoint: `c56dc050a`
Proof posture: specification closeout, not an additional behavior proof

## Written Contract

`docs/specs/08-standard-function-blocks.md` now defines the executed-scan timer
contract and the Figure 15(c) TOF `ET = PT` plateau. `docs/IEC_DECISIONS.md`
records the implementation-owned PT, call, clock, restart, TP, and TIME/LTIME
boundaries. The existing active source
`SPEC_IEC_STANDARD_FBS_CANDIDATE_001` owns the resolved contract.

## Runnable Disposition

`TEST_IEC_TIMER_TRACE_001` is the exact aggregate affected-test set. Its
hand-authored case file is bound to four real-ST traces:

- `IEC_TIMER_001_TOF_TIME_POST_EXPIRY_HOLD`
- `IEC_TIMER_001_TOF_LTIME_POST_EXPIRY_HOLD`
- `IEC_TIMER_001_TON_TIME_BASIC_DELAY`
- `IEC_TIMER_001_TP_TIME_BASIC_PULSE`

`prove.py v1` recorded red evidence at clean commit `05a328fd1` with only the
two TOF hold cases failing, then paired green evidence at clean descendant
`bea129001` with all four cases passing and the same proof-contract, case-file,
and trace-definition digests.

## Atomic Migration

The Phase 8 restart/time-base review now uses its existing `resolved_source`
variant and binds the same source that closes
`SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001`. The timer invariant and risk no
longer carry the closed gap as a live dependency.

The invariant advances only to `implemented/G1`. The tested
`ordering_or_lifecycle` cell is covered; `time_or_clock_fault` remains
`gap_open` because restart, PT-change, skipped-call, nonmonotonic-clock, and TP
short-input/retrigger branches were deliberately not asserted. This closeout
does not claim broad remote execution, G2, validation, release evidence, or IEC
conformance.
