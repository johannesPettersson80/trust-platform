# Breakpoint After Launch - Bug Checklist

Goal
- Pinpoint and prevent the failure mode: breakpoint set while already running does not stop on the next cycle.

## 1. Reproduction Steps
1. Start a debug session with no breakpoints.
2. Let runtime run for at least one full cycle.
3. Set breakpoint on a statement that executes every cycle.
4. Wait 2+ cycles (no manual pause).
5. Expected: debugger stops at that breakpoint with reason `breakpoint`.

Failure pattern:

- breakpoint resolves in logs, but no stop and no DAP `stopped` event.

## 2. Runtime Instrumentation Markers
Enable:
```bash
export ST_DEBUG_TRACE=1
export ST_DEBUG_TRACE_LOG=/tmp/trust-debug-dap.log
```

Must see, in order:

- `hook.entry ...`
- `hook.decision effective_mode=Running ...`
- `hook.breakpoint.check ... matched_generation=Some(...)`
- `hook.pause.enter reason=Breakpoint ...`
- `stop reason=Breakpoint ...`

Interpretation:

- No `hook.entry`: statement hook boundary not reached.
- `hook.entry` present + `matched_generation=None`: breakpoint match/resolution issue.
- `hook.pause.enter` present + no adapter stop event: adapter stop coordination/emission issue.

## 3. Adapter/DAP Markers
Enable:
```bash
export ST_DEBUG_DAP_VERBOSE=1
export ST_DEBUG_DAP_LOG=/tmp/trust-debug-dap.log
```

Must see:

- `[trust-debug][stop] action=recv reason=breakpoint ...`
- `[trust-debug][stop] action=emit reason=breakpoint ...`
- outbound DAP payload with `"event":"stopped","reason":"breakpoint"`.

## 4. Regression Tests (must pass)
- `cargo test -p trust-runtime --test debug_stepping breakpoint_set_after_launch_hits_next_cycle`
- `cargo test -p trust-runtime --test debug_stepping breakpoint_set_while_running_hits_on_subsequent_cycle`
- `cargo test -p trust-debug adapter::stop::tests`

## 5. Exit Criteria
- Manual repro in `examples/filling_line` stops on late-set breakpoint without Pause.
- Runtime trace proves boundary path reaches `hook.pause.enter`.
- Adapter trace proves `recv -> emit` for breakpoint stop.
- Regression tests above are green.
