# Communication Example: EtherCAT (Field-Validated Spanish Profile)

This example restores a previously field-tested EtherCAT configuration submitted
by an operator who validated EK1100 + EL2008 output behavior on real hardware.

## What is different from `../ethercat/`

- `ethercat/`: mock-first onboarding plus hardware handoff template
- `ethercat_field_validated_es/`: real adapter profile (`adapter = "enp111s0"`) and
  an 8-output snake pattern used for physical wiring verification

Use this folder when you want to replicate a known-good commissioning baseline,
then adapt only adapter name, module order, and output mapping.

## Hardware profile used by contributor

```text
[PC NIC] -> [EK1100 Coupler] -> [EL2008 DO 8ch]
```

## Transport gate (critical)

Real EtherCAT adapter mode is available only when:

- runtime is built with feature `ethercat-wire`
- runtime runs on a unix target

Without that, use `adapter = "mock"` for validation only.

## Step 1: Build and validate

Why: prove project and config schema integrity before touching network/hardware.

```bash
cd examples/communication/ethercat_field_validated_es
trust-runtime build --project . --sources src
trust-runtime validate --project .
```

## Step 2: Review `io.toml`

Why: EtherCAT startup issues are usually module-chain or adapter mismatch.

- `adapter = "enp111s0"`: contributor-tested NIC name (change on your host)
- module chain:
  - `EK1100` at `slot = 0`
  - `EL2008` at `slot = 1`, `channels = 8`
- `on_error = "fault"` plus explicit safe-state for `%QX0.0`..`%QX0.7`

## Step 3: Run on hardware

Why: this example is intended for physical output verification.

```bash
trust-runtime run --project .
```

Expected behavior:

- outputs follow a snake pattern (0->7 on, then 7->0 off)
- complete pattern period is based on `step_time` in `src/main.st`

Optional helper scripts from the original field submission:

- `./run-ethercat.sh`: configures NIC/capabilities, builds, and runs
- `./run-simple.sh`: rebuilds and runs without network/capability setup

## Step 4: Adapt safely for your line

1. Update `adapter` to your NIC.
2. Keep module order exactly equal to physical chain.
3. Keep `on_error = "fault"` and safe-state outputs during commissioning.
4. Confirm `%QX` mapping matches the terminal block/channel plan.

## Common mistakes

- running hardware adapter mode without `ethercat-wire`
- expecting `enp111s0` to exist on all hosts
- mismatched module chain vs physical order
- removing safe-state outputs before commissioning is stable
