# Communication Example: Multi-Driver Composition (`io.drivers`)

This example shows how to run multiple I/O transports in one runtime via
`io.drivers = [...]`.

## What you learn

- how composed drivers are declared and validated
- why production projects often need mixed protocols
- why `io.driver` and `io.drivers` are mutually exclusive

## Files in this folder

- `src/main.st`: minimal `%IX -> %QX` logic (`DO0 := DI0`)
- `src/config.st`: `%IX0.0` / `%QX0.0` symbol mapping and task binding
- `io.toml`: composed Modbus/TCP + MQTT profile
- `runtime.toml`: runtime profile defaults
- `trust-lsp.toml`: project profile

## Step 1: Build PLC logic

Why: isolate source correctness from integration concerns.

```bash
cd examples/communication/multi_driver
trust-runtime build --project . --sources src
```

## Step 2: Inspect composed driver config

Why: each transport has its own failure modes; composition should be explicit.

```toml
[io]
drivers = [
  { name = "modbus-tcp", params = { address = "127.0.0.1:1502", unit_id = 1, input_start = 0, output_start = 0, timeout_ms = 500, on_error = "fault" } },
  { name = "mqtt", params = { broker = "127.0.0.1:1883", topic_in = "trust/examples/multi/in", topic_out = "trust/examples/multi/out", reconnect_ms = 500, keep_alive_s = 5, allow_insecure_remote = false } }
]
```

Why this pattern:

- Modbus handles deterministic register exchange.
- MQTT handles broker/event style exchange.
- One runtime can compose both process-image contributors.

## Step 3: Validate composed form

Why: schema catches malformed driver entries and invalid mutually-exclusive
configuration.

```bash
trust-runtime validate --project .
```

## Step 4: Enforce mutual-exclusion rule

Why: mixed forms are ambiguous and rejected.

Do not mix these in one file:

```toml
[io]
driver = "simulated"
drivers = [{ name = "mqtt", params = {} }]
```

Use exactly one form:

- single driver: `io.driver` + `io.params`
- multi-driver: `io.drivers = [...]`

## Step 5: Commission transport-by-transport

Why: debugging all transports simultaneously obscures root cause.

Recommended sequence:

1. bring up Modbus endpoint and verify first,
2. bring up MQTT broker and verify second,
3. run both together and verify combined cycle behavior.

## Common mistakes

- composing drivers before validating each one independently
- inconsistent timeout/reconnect settings across transports
- forgetting safe-state policy while expanding transport scope
- mixing `io.driver` and `io.drivers`
