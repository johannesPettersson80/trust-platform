# Communication Examples Index

Docs category: `docs/public/examples/connectivity.md`

This folder groups protocol-focused communication examples so they are easy to find and compare.

Included protocols:

- `ads_line1` (Beckhoff ADS TwinCAT symbol import with offline generation and
  `[runtime.ads]` live runtime activation)
- `ads_server_basic` (truST as an ADS target exposing selected runtime globals
  to pyads/TwinCAT clients through `[runtime.ads_server]`)
- `modbus-tcp`
- `mqtt`
- `mqtt_traffic_light` (named PLC tags published to MQTT topics)
- `opcua`
- `ethercat` (mock-first + hardware handoff)
- `ethercat_field_validated_es` (field-tested EK1100 + EL2008 profile)
- `gpio`
- composed `multi_driver` (`io.drivers = [...]`)

## Why this folder exists

Most projects start with `simulated` or `loopback`, then fail during integration because protocol assumptions were never validated early. These examples isolate each protocol so teams can commission communication one layer at a time.

## Recommended execution order

1. `ads_line1/README.md`
   - learn deterministic TwinCAT symbol import with cached snapshots and generated ST globals.
2. `ads_server_basic/README.md`
   - learn ADS server exposure, source-pinned client allowlists, pyads smoke
     proof, and the remaining real TwinCAT merge gate.
3. `modbus_tcp/README.md`
   - learn deterministic request/response register mapping and timeout policy.
4. `mqtt/README.md`
   - learn broker/topic boundaries and reconnect behavior.
5. `mqtt_traffic_light/README.md`
   - publish program-instance variables without manual `VAR_CONFIG` addresses.
6. `opcua/README.md`
   - learn runtime wire exposure and feature-gated build behavior.
7. `ethercat/README.md`
   - learn mock-first module chain validation, then hardware handoff.
8. `ethercat_field_validated_es/README.md`
   - apply a previously field-tested real-adapter profile for EK1100 + EL2008 output commissioning.
9. `gpio/README.md`
   - learn IEC bit mapping to GPIO lines, debounce, and safe-state defaults.
10. `multi_driver/README.md`
   - learn composed-driver commissioning and mutual-exclusion rules.

## Common base layout in each example

- `trust-lsp.toml`: project + runtime endpoint defaults
- `src/main.st`: minimal IEC program logic
- `src/config.st`: task/resource binding + `VAR_CONFIG` `%I/%Q` mapping
- `io.toml`: protocol-specific I/O backend profile
- `runtime.toml`: runtime profile (OPC UA example uses this directly)
- ADS examples also include `ads.toml`, cached symbol snapshots under `ads/`,
  reviewed generated ST under `src/generated/`, and `[runtime.ads]` in
  `runtime.toml`.
- ADS server examples instead use `[runtime.ads_server]` in `runtime.toml` and
  expose existing declared runtime globals directly; they do not use
  `ads.toml`.

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
- ADS wire client:
  - live ADS wire access requires build feature `ads-wire`
  - `[runtime.ads] enabled = true` loads `ads.toml` and starts ADS workers at
    runtime startup
  - the offline import/validate workflow works from cached snapshots without a PLC connection
- ADS server:
  - serving truST globals to external ADS clients requires build feature
    `ads-server`
  - `[runtime.ads_server] enabled = true` starts the server listener on ADS
    router TCP port `48898` and exposes the configured logical AMS port
  - loopback/pyads proof is useful, but real TwinCAT browse/read/notification
    validation remains the merge gate for TwinCAT compatibility

These notes are repeated in the protocol READMEs and in `docs/guides/PLC_IO_BINDING_GUIDE.md`.
