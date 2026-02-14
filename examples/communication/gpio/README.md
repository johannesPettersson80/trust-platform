# Communication Example: GPIO (Edge I/O Mapping)

This example shows how to wire IEC `%IX/%QX` symbols to GPIO lines using
`io.driver = "gpio"`.

## What you learn

- how GPIO line mapping works for input/output process image bits
- why debounce and initial output state are explicit commissioning decisions
- how to validate GPIO configuration before touching hardware permissions

## Files in this folder

- `src/main.st`: minimal `%IX -> %QX` logic (`DO0 := DI0`)
- `src/config.st`: `%IX0.0` / `%QX0.0` symbol mapping and task binding
- `io.toml`: GPIO driver configuration
- `runtime.toml`: runtime profile defaults
- `trust-lsp.toml`: project profile

## Step 1: Build the PLC logic

Why: prove source/type consistency before I/O/backend troubleshooting.

```bash
cd examples/communication/gpio
trust-runtime build --project . --sources src
```

## Step 2: Review `io.toml` line by line

Why: GPIO errors usually come from mismatched addresses/lines, not parser issues.

```toml
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
```

Field intent:

- `backend`: GPIO access method (`sysfs` in this profile).
- `inputs[].address`: IEC input bit updated from line state.
- `inputs[].line`: physical BCM line index.
- `inputs[].debounce_ms`: minimum stable period before accepting edge change.
- `outputs[].address`: IEC output bit mapped to line write.
- `outputs[].initial`: line value applied on output configure.
- `io.safe_state`: fail-safe output fallback value.

## Step 3: Validate config before runtime boot

Why: catches structural issues early (`inputs`/`outputs` shape, address class,
required fields).

```bash
trust-runtime validate --project .
```

## Step 4: Understand runtime vs validation boundary

Why: validation checks schema + mapping semantics; runtime still depends on OS and
hardware access.

At runtime with real GPIO, ensure:

- the target host exposes configured lines,
- runtime process has permission to access GPIO,
- line ownership conflicts are resolved.

## Step 5: Commission in safe order

Why: output misconfiguration can energize hardware unexpectedly.

1. Start with disconnected or simulated load on output lines.
2. Verify `%IX0.0` and `%QX0.0` behavior in runtime panel.
3. Confirm safe-state `%QX0.0 = FALSE` on fault/stop paths.

## Common mistakes

- mapping output address into `inputs` (or inverse)
- using wrong BCM line numbers for target board
- omitting `io.safe_state` for energized outputs
- assuming successful `validate` implies runtime permission/hardware readiness
