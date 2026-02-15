# Communication Examples Index

This folder groups protocol-focused communication examples so they are easy to find and compare.

Included protocols:

- `modbus-tcp`
- `mqtt`
- `opcua`
- `ethercat` (mock-first + hardware handoff)
- `ethercat_field_validated_es` (field-tested EK1100 + EL2008 profile)
- `gpio`
- composed `multi_driver` (`io.drivers = [...]`)

## Why this folder exists

Most projects start with `simulated` or `loopback`, then fail during integration because protocol assumptions were never validated early. These examples isolate each protocol so teams can commission communication one layer at a time.

## Recommended execution order

1. `modbus_tcp/README.md`
   - learn deterministic request/response register mapping and timeout policy.
2. `mqtt/README.md`
   - learn broker/topic boundaries and reconnect behavior.
3. `opcua/README.md`
   - learn runtime wire exposure and feature-gated build behavior.
4. `ethercat/README.md`
   - learn mock-first module chain validation, then hardware handoff.
5. `ethercat_field_validated_es/README.md`
   - apply a previously field-tested real-adapter profile for EK1100 + EL2008 output commissioning.
6. `gpio/README.md`
   - learn IEC bit mapping to GPIO lines, debounce, and safe-state defaults.
7. `multi_driver/README.md`
   - learn composed-driver commissioning and mutual-exclusion rules.

## Common base layout in each example

- `trust-lsp.toml`: project + runtime endpoint defaults
- `src/main.st`: minimal IEC program logic
- `src/config.st`: task/resource binding + `VAR_CONFIG` `%I/%Q` mapping
- `io.toml`: protocol-specific I/O backend profile
- `runtime.toml`: runtime profile (OPC UA example uses this directly)

## Validation loop (all protocols)

Run from each protocol folder:

```bash
trust-runtime build --project . --sources src
trust-runtime validate --project .
trust-runtime ctl --project . io-read
```

Why this loop matters:

- `build` confirms ST parses/type-checks and bytecode generation succeeds.
- `validate` checks runtime + I/O schema before launch.
- `io-read` confirms the control plane can read process image state.

## Transport-gating notes (important)

- EtherCAT hardware transport (non-`mock` adapter):
  - requires build feature `ethercat-wire`
  - is only supported on unix targets in this build
- OPC UA wire server:
  - requires build feature `opcua-wire`
  - if `runtime.opcua.enabled = true` without that feature, startup fails with a feature-disabled error

These notes are repeated in the protocol READMEs and in `docs/guides/PLC_IO_BINDING_GUIDE.md`.
