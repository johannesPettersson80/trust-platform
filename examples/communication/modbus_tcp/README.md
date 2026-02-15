# Communication Example: Modbus/TCP

This example shows how to wire a minimal PLC project to `io.driver = "modbus-tcp"`.

## What you learn

- how `%IX/%QX` bits map through Modbus register space
- why `unit_id`, `input_start`, and `output_start` must be explicit
- how timeout and `on_error` policy affect runtime behavior

## Files in this folder

- `src/main.st`: simple input-to-output logic (`DO0 := DI0`)
- `src/config.st`: task binding plus `VAR_CONFIG` mapping (`P1.DI0`/`P1.DO0`)
- `io.toml`: Modbus/TCP backend profile
- `runtime.toml`: runtime defaults
- `trust-lsp.toml`: project settings

## Step 1: Build the project

Why: prove ST compile path is valid before protocol troubleshooting.

```bash
cd examples/communication/modbus_tcp
trust-runtime build --project . --sources src
```

## Step 2: Inspect `io.toml`

Why: every field controls a concrete transport or mapping decision.

```toml
[io]
driver = "modbus-tcp"

[io.params]
address = "127.0.0.1:1502"
unit_id = 1
input_start = 0
output_start = 0
timeout_ms = 500
on_error = "fault"
```

Field intent:

- `address`: Modbus server endpoint.
- `unit_id`: target slave/unit address.
- `input_start`: first register offset read into `%I` image.
- `output_start`: first register offset written from `%Q` image.
- `timeout_ms`: upper bound per exchange.
- `on_error = "fault"`: fail closed during commissioning.

## Step 3: Validate configuration

Why: catches schema/mode errors before runtime boot.

```bash
trust-runtime validate --project .
```

## Step 4: Run with a local test server

Why: isolate mapping/timeout behavior before connecting to plant hardware.

1. Start a Modbus test endpoint on `127.0.0.1:1502`.
2. Start runtime:

```bash
trust-runtime run --project .
```

3. In another terminal, inspect image:

```bash
trust-runtime ctl --project . io-read
```

## Step 5: Harden for production

Why: lab defaults often become hidden failure points in production.

- replace loopback endpoint with real server address
- keep `on_error = "fault"` until commissioning sign-off
- verify safe state outputs match actuator-safe values

## Common mistakes

- using wrong `unit_id` for gateway/slave topology
- shifting `input_start`/`output_start` by one register block
- setting `on_error = "warn"` too early in bring-up
- validating config but not testing real timeout behavior
