# StateChart Examples

Docs category: `docs/public/examples/visual-editors.md`

This directory contains example UML StateChart files for the truST LSP StateChart Editor.

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
  - Troubleshooting

## Quick Start

### Test in VS Code (Simulation)

**Option 1: Quick Script**
```bash
cd examples/statecharts
./test-snake.sh
```

**Option 2: Manual**
1. Open VS Code: `cd editors/vscode && code .`
2. Press **F5** to start Extension Development Host
3. In dev window: **Ctrl+O** → Browse to `.statechart.json` file
4. Editor opens automatically with visual diagram

### Run the Snake Animation

1. **Select Mode**: Choose **🖥️ Simulation** (default) or **🔌 Hardware**
2. **Click ▶️ Start** in Execution Panel (top right)
3. **Click START** button (appears in Available Events)
4. **Click TICK** repeatedly to step through animation
5. Watch the active state **light up green** as it becomes active!

### Execution Modes

#### 🖥️ Simulation Mode (Default)
- **No hardware required** - perfect for testing
- Actions logged to console only
- Great for learning and debugging state logic
- Works immediately, no setup needed

#### 🔌 Hardware Mode  
- **Requires trust-runtime backend** running with your hardware
- Actions execute on **real I/O** (EtherCAT, GPIO, etc.)
- LEDs actually turn on/off following state transitions
- Backend project: `../hardware_8do/`
- See [HARDWARE_EXECUTION.md](HARDWARE_EXECUTION.md) for complete setup guide

**Quick Hardware Setup:**
```bash
# Terminal 1: Start the backend
cd examples/hardware_8do
sudo ./start.sh

# Terminal 2: Open VS Code and test
cd editors/vscode
code .
# Press F5, open a .statechart.json, select 🔌 Hardware mode
```

### View Logs
- **Help > Toggle Developer Tools > Console**
- **Simulation mode:** `🖥️  [SIM] Executing action: turnOn_DO0`
- **Hardware mode:** `🔌 [HW] turnOn_DO0 → WRITE true to %QX0.0`

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

## Hardware Test Project 🔧

Ready-to-use trust project for hardware mode testing:

```bash
cd hardware-project

# 1. Build project
./build.sh

# 2. Start runtime (Terminal 1)
./run.sh

# 3. Test connection (Terminal 2)
./test-connection.sh

# 4. Open StateChart editor and select 🔌 Hardware mode!
```

**What's included:**
- ✅ Pre-configured trust-lsp.toml with control endpoint
- ✅ runtime.toml with fast cycle (10ms)
- ✅ io.toml with EtherCAT (EK1100 + EL2008) and GPIO examples
- ✅ Minimal ST program (required but does nothing)
- ✅ Build & run scripts
- ✅ Connection test utility

See [hardware-project/README.md](hardware-project/README.md) for complete documentation.

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
**Cons:** Requires StateChart runtime integration

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
