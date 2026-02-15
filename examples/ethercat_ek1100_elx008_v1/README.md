# EtherCAT EK1100 + EL1008 + EL2008: Definitive Bring-Up Tutorial

This tutorial is the reference onboarding path for EtherCAT backend v1.

Start in deterministic mock mode, verify behavior in VS Code, then transition
to real NIC/hardware.

## Hardware Chain Model

Physical order must match configured module order:

```text
[PLC Host NIC] -> [EK1100 Coupler] -> [EL1008 DI 8ch] -> [EL2008 DO 8ch]
```

- `EK1100`: EtherCAT coupler/root
- `EL1008`: 8 digital inputs
- `EL2008`: 8 digital outputs

Slot numbers in `io.toml` represent expected module order on the chain.

## Step 1: Understand io.toml Line by Line

Open `io.toml` and map each field:

- `[io]`
  - `driver = "ethercat"`: selects EtherCAT backend
- `[io.params]`
  - `adapter = "mock"`: deterministic software adapter (no hardware required)
  - `timeout_ms = 250`: communication timeout threshold
  - `cycle_warn_ms = 5`: cycle overrun warning threshold
  - `on_error = "fault"`: runtime fault policy on driver error
  - `mock_inputs = ["01", "00"]`: deterministic input pattern for mock driver
- `[[io.params.modules]]`
  - declares expected module chain, model, slot, channel count
- `[[io.safe_state]]`
  - defines fallback output values on fault (`%QX0.0 = FALSE`)

## Step 2: Validate Source Mapping

Open `src/config.st` and `src/Main.st`.

- `%IX0.0` mapped to `DI0` via `VAR_CONFIG`
- `%QX0.0` mapped to `DO0` via `VAR_CONFIG`
- `DO0 := DI0 OR RisingSeen`

This provides a minimal deterministic signal path for bring-up checks.

## Step 3: Build + Validate in Mock Mode

From repository root:

```bash
code examples/ethercat_ek1100_elx008_v1
trust-runtime build --project examples/ethercat_ek1100_elx008_v1 --sources src
trust-runtime validate --project examples/ethercat_ek1100_elx008_v1
```

## Step 4: Run + Verify in Runtime Panel

1. `Ctrl+Shift+P` -> `Structured Text: Open Runtime Panel`
2. Start runtime in Local mode.
3. Observe `%IX0.0` behavior (driven by `mock_inputs`).
4. Confirm `%QX0.0` response follows logic in `Main.st`.

If needed, adjust `mock_inputs` and re-run to test edge conditions.

## Step 5: Switch to Real Hardware

In `io.toml`:

- change `adapter = "mock"` -> actual NIC (for example `"eth0"`)

How to find NIC names:

```bash
ip -br link
```

Permission note:

- EtherCAT raw socket access may require elevated privileges/capabilities.
- If permission errors occur, run with appropriate OS/network permissions.

## Step 6: Debug from VS Code (F5)

1. Set breakpoint in `src/Main.st` on `DO0 := DI0 OR RisingSeen;`
2. Press `F5` using `.vscode/launch.json`
3. Observe `DI0`, `RisingSeen`, `DO0` transitions in Variables panel

## Troubleshooting Matrix

- Timeout errors:
  - increase `timeout_ms` and verify physical link state.
- Module not found / wrong slot:
  - verify physical order vs `[[io.params.modules]]` slots.
- Wrong NIC selected:
  - re-check `ip -br link`; ensure adapter is EtherCAT-capable link.
- Permission denied/raw socket errors:
  - run with proper capabilities/privileges.
- Cycle overrun warnings:
  - inspect `cycle_warn_ms`, host load, and task interval.

## Pre-Go-Live Checklist

- [ ] Module order physically matches `io.toml`
- [ ] Correct NIC selected (`adapter`)
- [ ] Safe-state outputs defined for all critical outputs
- [ ] Fault policy reviewed (`on_error`)
- [ ] Runtime panel checks passed in mock mode
- [ ] Runtime panel checks repeated on real hardware
