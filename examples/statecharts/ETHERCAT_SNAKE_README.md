# EtherCAT Snake Pattern - UML StateChart Examples

This directory contains statechart examples for real EtherCAT hardware control, including a visual "snake" / "Knight Rider" output pattern.

## Available Files

### `ethercat-snake.statechart.json`

Full snake pattern with 17 states:

- Phase 1 (states 0-8): turn LEDs on sequentially `0 -> 7`
- Phase 2 (states 9-16): turn LEDs off sequentially `7 -> 0`
- Continuous loop back to state 1
- 16 transitions per full cycle

### `ethercat-snake-bidirectional.statechart.json` (recommended)

Bidirectional pattern:

- Forward (`0 -> 7`)
- Backward (`6 -> 0`)
- Each state turns one LED on in entry and off in exit
- Visual effect: one moving active LED

## Required Hardware

```text
[PC NIC] -> [EK1100 Coupler] -> [EL2008 DO 8ch]
```

- `EK1100`: EtherCAT bus coupler
- `EL2008`: 8-channel digital output module
- LEDs/loads connected to `DO0..DO7`

## Action Mappings

`actionMappings` connect statechart actions to physical I/O.

```json
{
  "turnOn_DO0": {
    "action": "WRITE_OUTPUT",
    "address": "%QX0.0",
    "value": true
  }
}
```

### Supported action types

| Action Type | Description | Example |
|---|---|---|
| `WRITE_OUTPUT` | Write a digital output | `%QX0.0 := TRUE` |
| `WRITE_VARIABLE` | Write an ST variable | `motorSpeed := 1500` |
| `SET_MULTIPLE` | Write multiple outputs | Turn all LEDs off |
| `LOG` | Runtime debug logging | State diagnostics |

## IEC Address Mapping

```text
%QX0.0  -> EL2008 channel 0 (DO0)
%QX0.1  -> EL2008 channel 1 (DO1)
%QX0.2  -> EL2008 channel 2 (DO2)
...
%QX0.7  -> EL2008 channel 7 (DO7)
```

Format: `%QX[byte].[bit]`

- `Q` = output
- `X` = bit/boolean
- `0` = byte index
- `.0-.7` = bit index

## Test in VS Code (Simulation)

### 1. Open the extension project

```bash
cd editors/vscode
code .
# Press F5 to launch Extension Development Host
```

In the dev window open:

```text
examples/statecharts/ethercat-snake-bidirectional.statechart.json
```

### 2. Inspect the diagram

You should see:

- Forward and backward states
- `TICK` transitions
- Entry/exit actions for each state

### 3. Run simulation

1. Click `▶ Run`
2. Send `START`
3. Trigger `TICK` repeatedly (or use timer-based transitions)
4. Confirm active state moves across the graph

### 4. Check logs

```text
Help > Toggle Developer Tools > Console
```

Example logs:

```text
Executing action: turnOn_DO0
Executing action: turnOff_DO0
Executing action: turnOn_DO1
```

## Run with Real Hardware

### Current runtime integration

Use the backend runtime in `examples/statechart_backend` and run statecharts in hardware mode from the VS Code editor.

1. Start backend runtime.
2. Open statechart in extension host.
3. Select `Hardware` mode and start execution.
4. Verify I/O force/unforce logs and physical output behavior.

### Example project/runtime setup

Create a project with:

- `io.toml` configured for EtherCAT (`EK1100`, `EL2008`)
- Minimal ST program (`Main.st`)
- `CONFIGURATION` task file (`config.st`)

Example `io.toml` snippet:

```toml
[io]
driver = "ethercat"

[io.params]
adapter = "enp111s0"
timeout_ms = 250
cycle_warn_ms = 5
on_error = "fault"

[[io.params.modules]]
model = "EK1100"
slot = 0

[[io.params.modules]]
model = "EL2008"
slot = 1
channels = 8
```

Run command example:

```bash
sudo setcap cap_net_raw,cap_net_admin=eip "$(readlink -f "$(which trust-runtime)")"
sudo nmcli dev set enp111s0 managed no
sudo ip link set enp111s0 up

trust-runtime run --project examples/statecharts/ethercat-snake-project \
  --statechart examples/statecharts/ethercat-snake-bidirectional.statechart.json
```

## Pattern Timing

If `TICK` is emitted every 200ms:

| Phase | States | Total Time |
|---|---|---|
| Forward | 8 states | 1.6s |
| Backward | 7 states | 1.4s |
| Full cycle | 15 transitions | 3.0s |

Change speed by adjusting `tick_interval` in `Main.st`.

## Tips

- Start in simulation first, then move to hardware.
- Keep safe-state outputs configured in `io.toml`.
- Ensure action names in states match keys in `actionMappings`.
- Validate socket permissions and runtime connectivity before testing hardware events.
