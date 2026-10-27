# StateChart Examples

Docs category: `docs/public/examples/visual-editors.md`

This directory contains example UML StateChart files for the truST StateChart Editor.

## Files

### Basic Examples

#### `traffic-light.statechart.json`
A simple traffic light state machine with three states:
- **Red** → TIMER → Green
- **Green** → TIMER → Yellow  
- **Yellow** → TIMER → Red

**Use case:** Basic cyclical state machine demonstration.

#### `motor-control.statechart.json`
A motor control system with safety checks:
- **Stopped** (initial) - Motor disabled
- **CheckingSafety** - Validates safety conditions
- **Starting** - Ramps up motor speed
- **Running** - Normal operation
- **Stopping** - Controlled shutdown
- **Error** - Fault state with alarm

**Use case:** Industrial control with safety guards and error handling.

### EtherCAT Hardware Examples 🔌⚡

#### `ethercat-snake-simple.statechart.json` ⭐ Best for Learning
Simple 3-LED bidirectional pattern:
- **5 states total**: Init → LED_0 → LED_1 → LED_2 → LED_1_Back → LED_0_Back
- **Visual effect**: `●○○ → ○●○ → ○○● → ○●○ → ●○○`
- **Cycle time**: ~1 second (200ms × 5 ticks)
- **Hardware**: EK1100 + EL2008 (uses only 3 outputs)
- **Action mappings**: Maps to %QX0.0, %QX0.1, %QX0.2

**Best for**: Learning action mappings and testing hardware connection.

#### `ethercat-snake-bidirectional.statechart.json` ⭐ Recommended
Full 8-LED Knight Rider pattern:
- **15 states**: Init + Forward (0-7) + Backward (6-0)
- **Visual effect**: Single LED moves left-right-left continuously
- **Entry/Exit actions**: Turns LED ON on entry, OFF on exit
- **Cycle time**: ~3 seconds (200ms × 15 ticks)
- **Hardware**: EK1100 + EL2008 (full 8 outputs)
- **Action mappings**: Complete mapping to %QX0.0 through %QX0.7

**Best for**: Production-ready snake effect, hardware demonstrations.

#### `ethercat-snake.statechart.json`
Sequential turn-on/turn-off pattern:
- **17 states**: Complete phase-based control
- **Phase 1 (0-8)**: Turn ON LEDs sequentially 0→7
- **Phase 2 (9-16)**: Turn OFF LEDs sequentially 7→0
- **Visual effect**: Progressive activation then deactivation
- **Cycle time**: ~3.2 seconds (200ms × 16 ticks)

**Best for**: Testing sequential control patterns.

### 📚 Documentation

- **[ETHERCAT_SNAKE_README.md](ETHERCAT_SNAKE_README.md)**: Complete guide for EtherCAT examples
  - Hardware setup
  - Action mappings explained
  - How to run with real hardware
- **[HARDWARE_EXECUTION.md](HARDWARE_EXECUTION.md)**: How a statechart drives real I/O

## Quick Start

A statechart runs as Structured Text, like every visual program
(`docs/specs/17-visual-editors-runtime-unification.md`):

1. Open a `.statechart.json` file; the StateChart editor opens with the diagram.
2. Save it. Beside the file truST writes the companion `<name>.st`, a
   `FUNCTION_BLOCK FB_<name>_STATECHART`, and the runtime entry
   `<name>.visual.runtime.st`, which runs the companion every 20 ms.
3. Start it from the truST sidebar, like any Structured Text program.
4. Raise events: each event of an untimed transition is a `BOOL` input
   `EV_<EVENT>` of the instance `fb_<name>`. Set it to `TRUE` in the
   debugger's Variables view (Set Value) and back to `FALSE`; every scan in
   which it is `TRUE` takes that event's transition from the active state.
   A transition with `after` fires on its own timer.
5. Watch the outputs in Live Values; the active state is the instance's
   `STATE_NAME` output.

The editor has no simulation or hardware mode of its own: its TypeScript
engine, runtime client and execution panel were removed on 26 October 2026
(`docs/specs/25-vscode-product-contract.md` §11.4.1).

## Action Mappings Reference

### Supported Action Types

```json
{
  "actionMappings": {
    "turnOn_LED": {
      "action": "WRITE_OUTPUT",
      "address": "%QX0.0",
      "value": true
    },
    "setSpeed": {
      "action": "WRITE_VARIABLE",
      "variable": "motorSpeed",
      "value": 1500
    },
    "resetAll": {
      "action": "SET_MULTIPLE",
      "targets": [
        { "address": "%QX0.0", "value": false },
        { "address": "%QX0.1", "value": false }
      ]
    },
    "logStatus": {
      "action": "LOG",
      "message": "Status message"
    }
  }
}
```

In the companion, `WRITE_OUTPUT` and `SET_MULTIPLE` write their addresses (a
direct address such as `%QX0.0` becomes the global `ld_io_QX0_0`, which the
runtime entry declares `AT %QX0.0`), `WRITE_VARIABLE` writes the variable, and
`LOG` becomes a comment: it has no effect when the program runs.

### IEC 61131-3 Address Format

| Format | Type | Example | Description |
|--------|------|---------|-------------|
| `%QX0.0` | Digital Output Bit | `%QX0.0` | Byte 0, Bit 0 |
| `%QB0` | Digital Output Byte | `%QB0` | Byte 0 (8 bits) |
| `%IX0.0` | Digital Input Bit | `%IX0.0` | Input byte 0, bit 0 |
| `%MW0` | Memory Word | `%MW0` | Memory word 0 |

**EtherCAT EL2008 Mapping:**
- `%QX0.0` → Channel 0 (DO0)
- `%QX0.1` → Channel 1 (DO1)
- ... up to `%QX0.7` → Channel 7 (DO7)

## Hardware Requirements (For Real Execution)

### Minimum Setup
```
[PC] → [EtherCAT NIC] → [EK1100] → [EL2008]
```

### Tested Hardware
- **EK1100**: EtherCAT Bus Coupler
- **EL2008**: 8-channel 24V DC Digital Output
- **Network**: Dedicated Ethernet port for EtherCAT

### Software Requirements
- `trust-runtime` with EtherCAT support
- Network interface with raw socket capability
- Linux kernel with EtherCAT drivers (or ethercrab userspace driver)
- A project `io.toml` that maps `%QX0.0`–`%QX0.7` to the EL2008
  (`examples/hardware_8do/io.toml` has an EK1100 + EL2008 and a Raspberry Pi
  GPIO setup)

## Running on Hardware 🔧

The runtime entry declares every address the chart writes as a global `AT`
that address, so the project's I/O configuration drives the hardware with the
values the companion writes. See [HARDWARE_EXECUTION.md](HARDWARE_EXECUTION.md).

---

## Comparison: StateChart vs Traditional ST

### Traditional ST Approach (ethercat_ek1100_elx008_v2)
```structured-text
PROGRAM Main
VAR
    position : INT := 0;
    step_timer : TON;
END_VAR

step_timer(IN := NOT step_timer.Q, PT := T#200MS);

IF step_timer.Q THEN
    position := position + 1;
    (* Complex IF-ELSE logic for each position *)
END_IF
END_PROGRAM
```

**Pros:** Flexible timing, direct control
**Cons:** Hard to visualize, difficult to maintain

### StateChart Approach
```json
{
  "states": {
    "LED_0": {
      "entry": ["turnOn_DO0"],
      "exit": ["turnOff_DO0"],
      "on": { "TICK": "LED_1" }
    }
  }
}
```

**Pros:** Visual representation, clear state transitions, easy to debug
**Cons:** Actions are limited to the mapped types (`WRITE_OUTPUT`, `WRITE_VARIABLE`, `SET_MULTIPLE`, `LOG`)

## Creating New StateCharts

Create a new file with `.statechart.json` extension:

```json
{
  "id": "myMachine",
  "initial": "Idle",
  "states": {
    "Idle": {
      "on": {
        "START": "Running"
      }
    },
    "Running": {
      "on": {
        "STOP": "Idle"
      }
    }
  }
}
```

## Features

- ✨ Visual drag-and-drop editor
- 🎨 State types: normal, initial, final, compound
- ⚡ Entry/exit actions
- 🔀 Transition events and guards
- 💾 XState-compatible JSON format
- 🚀 Auto-layout for organizing diagrams
