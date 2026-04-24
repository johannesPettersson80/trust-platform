# truST PLC Safety Guide

This guide explains how the runtime handles faults, watchdogs, and safe-state outputs.
It is not a substitute for a site-specific safety assessment.

## 1) Safe State Outputs

Define safe outputs in the Web UI (**I/O → I/O configuration → Safe‑state outputs**),
or directly in `io.toml`, so the runtime can force them on fault/watchdog:

```
[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
```

When a fault occurs, outputs are driven to these values before halting.

## 2) Watchdog Behavior

Watchdog monitors cycle/task execution time. If the timeout is exceeded:

- Resource transitions to **FAULT**
- Outputs go to safe state
- Execution halts until restart

Example:
```
[runtime.watchdog]
enabled = true
timeout_ms = 5000
action = "safe_halt"
```

## 3) Fault Policy

Faults include divide-by-zero, out-of-bounds access, invalid type conversion,
FOR step of 0, and deadline overruns.

Set policy in `runtime.toml`:
```
[runtime.fault]
policy = "halt"
```

## 4) Retain + Restart

Warm restart restores RETAIN variables. Cold restart resets all values.

Use warm restarts for controlled recovery. Use cold restarts after wiring changes
or if state is uncertain.

## 5) Debug in Production

Debug attach is disabled by default in production. Only enable in controlled
maintenance windows.

```
runtime.control.mode = "production"
runtime.control.debug_enabled = false
```

## 6) Operator Checklist

Before commissioning:

- Verify safe-state outputs.
- Trigger a test fault and confirm outputs go safe.
- Confirm watchdog timeout and restart behavior.
- Confirm retain persistence for required values.

During operation:

- Monitor status and fault events.
- Restart cold if safety is uncertain.
