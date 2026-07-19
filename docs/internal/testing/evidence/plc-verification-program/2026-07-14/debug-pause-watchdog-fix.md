# Debug pause/watchdog product fix

Date: 2026-07-14

## Missing contract and test

`SPEC_GAP_DEBUG_PAUSE_WATCHDOG_001` recorded that debugger pause interaction
with cycle deadlines, watchdogs, I/O, safe state, and resume was unspecified.
The product contract is now written in `docs/specs/11-runtime-engine.md` and
`docs/specs/13-debug-adapter.md`, with the host/debugger extension classified as
DEV-049 rather than an IEC conformance deviation.

Two focused integration tests were added in
`crates/trust-runtime/tests/debug_pause_watchdog.rs`:

- statement-boundary debugger pause longer than the watchdog timeout;
- resource pause between cycles with no scan or watchdog interval while paused.

## Red result

Clean commit `5b7aa66da09d46f770895e230536213bbc4b8c88` was run on
`trust-builder` with:

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --test debug_pause_watchdog -- --nocapture
```

The between-cycle resource-pause case passed. The statement-pause case failed
after a 100 ms operator dwell under a 20 ms watchdog:

```text
operator dwell at a statement pause must not become watchdog execution time;
error=Some(WatchdogTimeout)
left: Faulted
right: Running
```

This is a product defect: the debug hook waited inside an already-armed cycle,
and both the evaluator deadline and the scheduler's post-cycle wall-clock check
counted the paused interval as active execution.

## Green result

The runtime now accumulates only statement-boundary debug wait time, extends
the current execution and output-commit deadlines by that exact amount, and
subtracts the same interval from the scheduler's active-execution measurement.
It does not reset the watchdog or exclude ordinary execution time.

Clean commit `1d08d125298fdf1e8b0ce871c6036dd82431cd00` passed on
`trust-builder`:

```text
cargo test -p trust-runtime --test debug_pause_watchdog
# 2 passed

cargo test -p trust-runtime --test runtime_reliability \
  watchdog_faults_resource_on_overrun -- --exact
# 1 passed

cargo test -p trust-runtime --test runtime_safety_fail_closed \
  watchdog_deadline_breach_before_commit_prevents_output_write -- --exact
# 1 passed

cargo test -p trust-runtime --lib deadline
# 11 passed: scheduler deadline guards plus stack, register, and tier-1 VM paths
```

## Honest posture

This row is `proof_kind = "none"`. The test is an ordinary Rust regression and
does not emit a same-run verification case artifact, so it is not promoted to
producer-authentic red/green proof. The specification gap advances only to
`test_mapped`; `DEBUG_PAUSE_001` remains S0 with targeted proof and the batch
broad gate still explicit debt.
