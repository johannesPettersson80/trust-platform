# Tutorial 17: I/O Backends and Multi-Driver Configuration

This tutorial teaches how to configure and validate all major I/O backend forms:

- `loopback`
- `simulated`
- `gpio`
- `modbus-tcp`
- `mqtt`
- composed `io.drivers = [...]`

## Why this tutorial exists

Many users only run `simulated` in early testing, then hit avoidable failures
when moving to hardware or multi-protocol integration. This guide explains each
form and what it is for.

## What you will learn

- when to use each backend
- how to keep safe-state output policy across driver changes
- how to validate `io.toml` before runtime start
- why `io.driver` and `io.drivers` cannot be mixed

## Prerequisites

- complete Tutorial 13 first
- optional hardware/network brokers for full runtime checks

## Step 1: Prepare a sandbox project

Why: swapping backends repeatedly is easier in a disposable copy.

```bash
rm -rf /tmp/trust-io-backends
cp -R /tmp/trust-tutorial-13 /tmp/trust-io-backends
cd /tmp/trust-io-backends
```

Build once:

```bash
trust-runtime build --project . --sources src
```

## Step 2: Start with loopback (local functional sanity)

Why: `loopback` is the fastest no-hardware path to confirm `%Q` writes can feed
`%I` reads for local testing.

```bash
cat > io.toml <<'TOML'
[io]
driver = "loopback"
params = {}

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
TOML

trust-runtime validate --project .
```

Expected result:
- validation passes

## Step 3: Switch to simulated (deterministic software I/O)

Why: `simulated` is useful when you want fake process behavior without physical
pins or remote protocols.

```bash
cat > io.toml <<'TOML'
[io]
driver = "simulated"
params = {}

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
TOML

trust-runtime validate --project .
```

Expected result:
- validation passes

## Step 4: Configure GPIO profile (hardware edge I/O)

Why: GPIO needs explicit IEC-to-pin mapping and debounce/initial state choices.

```bash
cat > io.toml <<'TOML'
[io]
driver = "gpio"

[io.params]
backend = "sysfs"
inputs = [
  { address = "%IX0.0", line = 17, debounce_ms = 5 }
]
outputs = [
  { address = "%QX0.0", line = 27, initial = false }
]

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
TOML

trust-runtime validate --project .
```

Expected result:
- config is schema-valid
- runtime may still require platform permissions/hardware at run time

## Step 5: Configure Modbus/TCP profile

Why: Modbus introduces transport, unit-id, and timeout policy decisions.

```bash
cat > io.toml <<'TOML'
[io]
driver = "modbus-tcp"

[io.params]
address = "192.168.0.10:502"
unit_id = 1
input_start = 0
output_start = 0
timeout_ms = 500
on_error = "fault"

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
TOML

trust-runtime validate --project .
```

Expected result:
- config validates
- actual runtime connectivity depends on reachable Modbus server

## Step 6: Configure MQTT profile

Why: MQTT requires explicit topic boundaries and reconnect behavior.

```bash
cat > io.toml <<'TOML'
[io]
driver = "mqtt"

[io.params]
broker = "127.0.0.1:1883"
topic_in = "line/in"
topic_out = "line/out"
reconnect_ms = 500
keep_alive_s = 5
allow_insecure_remote = false

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
TOML

trust-runtime validate --project .
```

Expected result:
- config validates
- runtime connectivity depends on broker availability and ACLs

## Step 7: Use multi-driver composition (`io.drivers`)

Why: production systems may need multiple protocol drivers in one runtime.

```bash
cat > io.toml <<'TOML'
[io]
drivers = [
  { name = "modbus-tcp", params = { address = "192.168.0.10:502", unit_id = 1, input_start = 0, output_start = 0, timeout_ms = 500, on_error = "fault" } },
  { name = "mqtt", params = { broker = "127.0.0.1:1883", topic_in = "line/in", topic_out = "line/out", reconnect_ms = 500, keep_alive_s = 5, allow_insecure_remote = false } }
]

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
TOML

trust-runtime validate --project .
```

Expected result:
- configuration validates as composed-driver form

## Step 8: Understand the mutual-exclusion rule

Why: mixed driver forms are ambiguous and rejected.

Do not do this in the same file:

```toml
[io]
driver = "simulated"
drivers = [{ name = "mqtt", params = {} }]
```

Use exactly one form:
- `io.driver` + `io.params`
- or `io.drivers = [...]`

## Step 9: Final verification command set

Why: standard validation loop catches most integration mistakes early.

```bash
trust-runtime build --project . --sources src
trust-runtime validate --project .
trust-runtime ctl --project . io-read
```

## Common mistakes

- forgetting `io.safe_state` for critical outputs
- setting `on_error = "ignore"` during commissioning
- validating config but not verifying real network reachability
- mixing `io.driver` and `io.drivers`

## Completion checklist

- [ ] each backend form authored and validated
- [ ] multi-driver form validated
- [ ] safe-state policy preserved across changes
- [ ] mutual-exclusion rule understood and enforced
