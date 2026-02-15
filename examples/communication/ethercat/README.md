# Communication Example: EtherCAT (Mock First, Hardware Next)

This example shows safe bring-up for `io.driver = "ethercat"`.

## Critical transport gate

Hardware EtherCAT transport (non-`mock` adapter) is gated by build and platform:

- requires feature `ethercat-wire`
- supported only on unix targets in this build

`adapter = "mock"` remains valid for deterministic local/CI validation.

## What you learn

- how to validate module-chain mapping without hardware
- how to transition from mock profile to real NIC adapter
- where feature/platform gating applies in commissioning

## Files in this folder

- `src/main.st`: `%IX -> %QX` minimal logic (`DO0 := DI0`)
- `src/config.st`: task binding plus `VAR_CONFIG` mapping (`P1.DI0`/`P1.DO0`)
- `io.toml`: deterministic mock profile
- `io.hardware.toml`: hardware profile template (`adapter = "eth0"`)
- `runtime.toml`: runtime defaults
- `trust-lsp.toml`: project settings

## Step 1: Build project

Why: prove IEC project integrity before bus troubleshooting.

```bash
cd examples/communication/ethercat
trust-runtime build --project . --sources src
```

## Step 2: Validate mock profile (`io.toml`)

Why: mock mode verifies mapping and safety policy without field hardware.

```bash
trust-runtime validate --project .
trust-runtime run --project .
```

`io.toml` uses:

- `adapter = "mock"`
- module chain `EK1100 -> EL1008 -> EL2008`
- `on_error = "fault"`
- explicit `io.safe_state`

## Step 3: Prepare hardware profile

Why: hardware transport needs explicit adapter and matching chain declaration.

```bash
cp io.hardware.toml io.toml
```

Then set `adapter` to your EtherCAT NIC name.

## Step 4: Confirm feature/platform requirements

Why: prevents ambiguous startup failures during handoff.

- build runtime with `ethercat-wire` enabled
- run on unix target for hardware adapter mode
- ensure raw socket access permissions are in place

## Step 5: Commission on hardware

Why: final verification must use real bus timing and fault behavior.

```bash
trust-runtime validate --project .
trust-runtime run --project .
trust-runtime ctl --project . io-read
```

## Common mistakes

- expecting non-`mock` adapter to work without `ethercat-wire`
- attempting hardware adapter mode on non-unix target
- mismatching physical module order vs `[[io.params.modules]]`
- downgrading `on_error` policy before validating safe-state outputs
