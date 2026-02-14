# Tutorial 13: Bootstrap a PLC Project from Zero to First Running App

This tutorial starts from an empty folder and takes you to a running PLC with
one real Structured Text program, mapped I/O, and a live runtime.

## Why this tutorial exists

Most examples in this repository already have files in place. Real users often
start from nothing. This tutorial covers the exact sequence from zero files to a
working project so you understand what each file is for and why it is required.

## What you will learn

- minimal project structure (`runtime.toml`, `io.toml`, `src/*.st`)
- how ST logic, `CONFIGURATION`, and `%I/%Q` mappings connect
- why `build` and `validate` are separate checks
- how to verify behavior from runtime I/O

## Prerequisites

- `trust-runtime` installed and available on `PATH`
- optional: VS Code with truST extension
- shell with write access to `/tmp`

## Step 1: Create an empty workspace

Why: starting from an empty directory makes file purpose explicit.

```bash
mkdir -p /tmp/trust-tutorial-13
cd /tmp/trust-tutorial-13
```

Expected result:
- directory exists and is empty

## Step 2: Add the PLC logic (`src/Main.st`)

Why: runtime executes bytecode generated from ST logic. We start with a minimal,
observable rule: copy one input to one output.

```bash
mkdir -p src
cat > src/Main.st <<'ST'
PROGRAM FirstApp
VAR
    StartCmd : BOOL;
    LampOut : BOOL;
END_VAR

LampOut := StartCmd;
END_PROGRAM
ST
```

Expected result:
- `src/Main.st` exists
- logic is simple and testable (`%IX0.0` controls `%QX0.0` through mapping)

## Step 3: Add task and I/O mapping (`src/Configuration.st`)

Why: `PROGRAM` defines logic, but `CONFIGURATION` defines when it runs and how
variables bind to physical/process-image addresses.

```bash
cat > src/Configuration.st <<'ST'
CONFIGURATION FirstConfig
TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH Fast : FirstApp;
VAR_CONFIG
    P1.StartCmd AT %IX0.0 : BOOL;
    P1.LampOut AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
ST
```

Expected result:
- scan cycle every 100 ms
- input and output addresses are explicitly mapped

## Step 4: Add runtime config (`runtime.toml`)

Why: runtime needs resource timing and control/web endpoints.

```bash
cat > runtime.toml <<'TOML'
[bundle]
version = 1

[resource]
name = "Tutorial13Resource"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-runtime-tutorial-13.sock"
mode = "production"
debug_enabled = false

[runtime.web]
enabled = true
listen = "127.0.0.1:18083"
auth = "local"

[runtime.discovery]
enabled = false
service_name = "trust-tutorial-13"
advertise = false
interfaces = []

[runtime.mesh]
enabled = false
listen = "127.0.0.1:15283"
auth_token = ""
publish = []
subscribe = {}

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
TOML
```

Expected result:
- runtime can start deterministically on known local ports/socket

## Step 5: Add I/O driver config (`io.toml`)

Why: without `io.toml`, `%I/%Q` addresses may not be connected to a useful test
driver. `loopback` gives safe local behavior for first validation.

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

Expected result:
- runtime has an explicit driver and safe-state fallback

## Step 6: Build bytecode

Why: `build` compiles ST into executable bytecode (`program.stbc`).

```bash
trust-runtime build --project . --sources src
```

Expected result:
- `program.stbc` is generated
- no compile errors

## Step 7: Validate project

Why: `validate` checks configuration + bundle contract. It catches issues that
can be missed if you only compile sources.

```bash
trust-runtime validate --project .
```

Expected result:
- validation succeeds

## Step 8: Run and verify behavior

Why: final proof is runtime behavior, not just successful compilation.

Terminal A:

```bash
trust-runtime run --project .
```

Terminal B (optional status check):

```bash
trust-runtime ctl --project . status
```

Open:

- `http://127.0.0.1:18083`

Then use runtime I/O controls (Web UI or VS Code Runtime Panel):

1. set `%IX0.0 = TRUE`
2. verify `%QX0.0 = TRUE`
3. set `%IX0.0 = FALSE`
4. verify `%QX0.0 = FALSE`

Expected result:
- output follows input every scan cycle

## Step 9: Make one intentional change and re-run

Why: this confirms your edit/build/run loop is working.

Edit `src/Main.st`:

```st
LampOut := NOT StartCmd;
```

Rebuild and run again:

```bash
trust-runtime build --project . --sources src
trust-runtime run --project .
```

Expected result:
- behavior is inverted (`TRUE` input drives `FALSE` output)

## Common mistakes

- missing `CONFIGURATION` file:
  runtime starts but cannot schedule your logic correctly
- editing ST but forgetting `build`:
  runtime still runs old `program.stbc`
- missing `io.toml`:
  you may get confusing I/O behavior or empty mappings

## Completion checklist

- [ ] project created from empty folder
- [ ] ST + configuration compile into bytecode
- [ ] runtime starts and serves Web UI
- [ ] `%IX0.0` and `%QX0.0` behavior verified
- [ ] one change applied and re-validated
