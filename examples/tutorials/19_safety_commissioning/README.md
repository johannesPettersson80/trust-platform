# Tutorial 19: Safety Commissioning (Safe State, Watchdog, Fault Policy, Restart)

This tutorial is a practical commissioning checklist you can run in a bench
environment before production start-up.

## Safety note

Run this tutorial only in a non-production environment. You will intentionally
trigger a runtime fault to verify fail-safe behavior.

## Why this tutorial exists

Safety settings are often configured but not proven. This tutorial validates
that fault handling actually drives outputs to safe state and that restart
procedures are understood by operators.

## What you will learn

- how to configure safe-state output policy
- how to enable watchdog and fault halt policy
- how to intentionally trigger a benign fault for verification
- how cold restart recovery behaves after fault

## Prerequisites

- complete Tutorial 13 first

## Step 1: Prepare an isolated safety test project

Why: fault-injection testing should never run against your live project.

```bash
rm -rf /tmp/trust-safety-commissioning
cp -R /tmp/trust-tutorial-13 /tmp/trust-safety-commissioning
cd /tmp/trust-safety-commissioning
```

## Step 2: Add a controlled fault trigger in logic

Why: you need a deterministic way to induce a known fault and verify runtime
reaction.

Replace `src/Main.st` with:

```bash
cat > src/Main.st <<'ST'
PROGRAM FirstApp
VAR
    StartCmd : BOOL;
    FaultTrigger : BOOL;
    LampOut : BOOL;
    Divisor : INT := 1;
    ProbeValue : INT;
END_VAR

IF FaultTrigger THEN
    Divisor := 0;
ELSE
    Divisor := 1;
END_IF;

ProbeValue := 10 / Divisor;
LampOut := StartCmd;
END_PROGRAM
ST
```

Update `src/Configuration.st` mapping:

```bash
cat > src/Configuration.st <<'ST'
CONFIGURATION FirstConfig
TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH Fast : FirstApp;
VAR_CONFIG
    P1.StartCmd AT %IX0.0 : BOOL;
    P1.FaultTrigger AT %IX0.1 : BOOL;
    P1.LampOut AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
ST
```

Expected result:
- `%IX0.1 = TRUE` causes divide-by-zero fault deterministically

## Step 3: Configure safety policy in runtime and I/O

Why: fault behavior must be explicit, not implicit.

`io.toml`:

```bash
cat > io.toml <<'TOML'
[io]
driver = "loopback"
params = {}

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
TOML
```

In `runtime.toml`, set:

```toml
[runtime.watchdog]
enabled = true
timeout_ms = 2000
action = "safe_halt"

[runtime.fault]
policy = "halt"
```

Expected result:
- runtime halts on fault
- outputs move to configured safe state

## Step 4: Build and validate

Why: syntax/config mistakes in safety settings invalidate the test.

```bash
trust-runtime build --project . --sources src
trust-runtime validate --project .
```

## Step 5: Start runtime and verify normal behavior first

Why: establish baseline before inducing any fault.

```bash
trust-runtime run --project .
```

In UI/Runtime Panel:
- set `%IX0.0 = TRUE`
- verify `%QX0.0 = TRUE`

## Step 6: Trigger fault and verify safe-state reaction

Why: this is the core commissioning evidence.

- set `%IX0.1 = TRUE` (`FaultTrigger`)
- observe runtime transitions to fault/halt
- verify `%QX0.0` is forced to `FALSE`

Expected result:
- runtime stops executing logic
- safe-state output is applied immediately

## Step 7: Recover with cold restart

Why: operators need deterministic recovery procedure.

```bash
trust-runtime ctl --project . restart --mode cold
```

After restart:
- clear `%IX0.1` back to `FALSE`
- verify runtime returns to normal running state

## Step 8: Record commissioning evidence

Why: safety sign-off requires proof, not memory.

Capture and archive:
- runtime status before/after fault
- safe-state output observation
- restart command and post-restart status

## Common mistakes

- testing fault policy without defining `io.safe_state`
- running fault-injection on production-connected hardware
- enabling watchdog but never testing timeout/fault path

## Completion checklist

- [ ] safe-state output policy configured
- [ ] watchdog and fault policy enabled
- [ ] controlled fault triggered and observed
- [ ] output forced to safe state verified
- [ ] cold restart recovery validated
