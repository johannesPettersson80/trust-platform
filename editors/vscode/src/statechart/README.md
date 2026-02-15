# StateChart Editor - Usage Guide

Visual UML StateChart editor with simulation and real-hardware execution.

## Table of Contents

- [Implemented Features](#implemented-features)
- [Quick Start](#quick-start)
- [Execution Modes](#execution-modes)
- [Editor Layout](#editor-layout)
- [Example: Traffic Light](#example-traffic-light)
- [Development with Real Hardware](#development-with-real-hardware)
- [Troubleshooting](#troubleshooting)
- [Hardware Action Mappings](#hardware-action-mappings)
- [Related Docs](#related-docs)
- [References](#references)

---

## Implemented Features

### Visual Editor

- Create/edit states (`normal`, `initial`, `final`, `compound`)
- Add transitions and events
- Edit entry/exit actions
- Full properties panel
- Visual Action Mappings panel for hardware configuration
- Validation and warnings for unmapped actions
- Auto layout and zoom controls

### Execution System

- Simulation mode (no hardware)
- Hardware mode (`trust-runtime`, EtherCAT/GPIO/etc.)
- Start/Stop controls
- Live current-state indicator
- Available event list from current state
- Event dispatch buttons + custom event input
- Active-state visual highlight
- Auto-transitions with timers (`after` in ms)

## Quick Start

### 1. Start the editor

1. Open VS Code at `editors/vscode` (from repository root).
2. Press `F5` to launch the Extension Development Host.
3. In the dev window, open:

```text
examples/statecharts/traffic-light.statechart.json
```

### 2. Edit a statechart

- Add states: `➕ State`, `🟢 Initial`, `🔴 Final`
- Connect transitions: drag from one state to another
- Edit properties: select a node and use the lower-right panel
- Add actions: use `➕` in Entry/Exit actions

### 3. Run it

1. Click `▶ Run` in the upper-right execution panel.
2. The initial state is highlighted.
3. Available events appear as buttons.
4. Click events to transition.
5. The diagram updates to the active state.

### 4. Simulate

- Use event buttons to trigger transitions.
- Or enter a custom event in `Send Custom Event`.
- Watch the state update in real time.

## Editor Layout

```text
┌────────────────────────────────────────┬──────────────────┐
│                                        │  Execution Panel │
│         Visual Diagram                 │  • Run/Stop      │
│         (ReactFlow)                    │  • Current State │
│         • States                       │  • Events        │
│         • Transitions                  │  • Custom Event  │
│         • Toolbar                      │                  │
│                                        ├──────────────────┤
│                                        │ Properties Panel │
│                                        │  • Label         │
│                                        │  • Type          │
│                                        │  • Entry Actions │
│                                        │  • Exit Actions  │
└────────────────────────────────────────┴──────────────────┘
```

## Execution Modes

### Simulation

- In-memory statechart execution (TypeScript)
- Event-driven transitions
- Entry/exit action logging
- Configurable automatic timers (`after`)
- Guard evaluation (always `true` in simulation)
- Final-state detection
- No hardware required

### Hardware

- Connects to `trust-runtime` control endpoint
- Supports Unix socket (`/tmp/trust-debug.sock`) and TCP
- Direct output forcing (`io.force`)
- Input reading for guards (`io.read`)
- Real I/O guard expressions, for example:
  - `%IX0.0 == TRUE`
  - `%IW0 > 100`
- Action mappings from logical actions to physical addresses
- Automatic cleanup/unforce on stop
- Boolean conversion to `"TRUE"` / `"FALSE"` for runtime protocol

## Example: Traffic Light

This example demonstrates:

- 3 states: `Red`, `Green`, `Yellow`
- 1 event: `TIMER`
- Entry actions: switch corresponding light ON
- Exit actions: switch light OFF
- Full cycle: `Red -> Green -> Yellow -> Red`

Quick check:

1. Open `traffic-light.statechart.json`
2. Press `Run`
3. Click `TIMER` repeatedly
4. Confirm active-state transitions in the diagram

---

## Development with Real Hardware

This section covers end-to-end development and testing with physical hardware.

### Architecture

```text
┌─────────────────────┐         ┌──────────────────────┐
│  VS Code Extension  │         │   trust-runtime      │
│  (Dev Host)         │◄───────►│   + Hardware Driver  │
│                     │  Socket │   (EtherCAT/GPIO)    │
│  • StateChart Editor│         │                      │
│  • RuntimeClient    │         │  • Control Endpoint  │
│  • Hardware Mode    │         │  • I/O Forcing       │
└─────────────────────┘         └──────────────────────┘
                                          │
                                          ▼
                                   ┌──────────────┐
                                   │   Hardware   │
                                   │  EK1100 +    │
                                   │  EL2008      │
                                   └──────────────┘
```

### Prerequisites

1. Hardware configured (for example EK1100 + EL2008)
2. `trust-runtime` built (preferably from source)
3. Backend project at `examples/statechart_backend/`
4. Permissions for hardware access (sudo/network)

### End-to-end workflow

#### 1. Start backend runtime

```bash
# Terminal 1
cd examples/statechart_backend
sudo ./start.sh
```

Expected output includes:

```text
✅ Build complete
🚀 Starting runtime...
   Control endpoint: /tmp/trust-debug.sock
✅ Control endpoint ready: /tmp/trust-debug.sock (rw-rw----)
✅ Backend is running!
```

Verify socket:

```bash
ls -l /tmp/trust-debug.sock
# srw-rw---- 1 root <your-group> ... /tmp/trust-debug.sock
```

#### 2. Open VS Code and launch extension host

```bash
# Terminal 2
cd editors/vscode
code .
```

Then in VS Code:

1. Press `F5` (`Run > Start Debugging`)
2. Wait for `[Extension Development Host]`

#### 3. Open a statechart example

Options:

- `File > Open File...` -> `examples/statecharts/ethercat-snake.statechart.json`
- `Ctrl+P` -> `ethercat-snake.statechart.json`
- `File > Open Folder...` -> `examples/statecharts/`

#### 4. Run in hardware mode

In execution panel:

1. Select `🔌 Hardware`
2. Confirm connection to `/tmp/trust-debug.sock`
3. Click `▶ Start Hardware`
4. Trigger events (`START`, `TIMER`) or rely on `after` timers

Hardware logs look like:

```text
✅ Connected to trust-runtime via Unix socket: /tmp/trust-debug.sock
🎯 StateMachine initialized in hardware mode
🔌 [HW] turnOn_DO0 -> FORCE true to %QX0.0
✅ Forced true to %QX0.0
```

Stop execution with `⏹ Stop` to release forced addresses.

### Iteration loop

1. Edit JSON or visual diagram
2. Save (`Ctrl+S`)
3. Reload webviews (`Developer: Reload Webviews`) or reopen file
4. Start again

You only need `npm run compile` after extension TypeScript changes.

## Troubleshooting

### Cannot connect to `/tmp/trust-debug.sock`

```bash
ps aux | grep trust-runtime
ls -l /tmp/trust-debug.sock
sudo pkill -f trust-runtime
cd examples/statechart_backend
sudo ./start.sh
```

### `EACCES: Permission denied /tmp/trust-debug.sock`

```bash
sudo chgrp <your-group> /tmp/trust-debug.sock
sudo chmod 660 /tmp/trust-debug.sock
```

`start.sh` should apply group + `660` automatically.

### Outputs do not change on hardware

1. Confirm hardware logs show force operations.
2. Run runtime manually with verbose logs.
3. Validate `actionMappings` addresses and values.

### Extension changes not reflected

```bash
cd editors/vscode
npm run compile
# In VS Code: Ctrl+Shift+F5 (Restart Debugging)
```

## Hardware Action Mappings

Action mappings bind statechart action names to runtime I/O operations.

### Visual mappings editor

Use the Action Mappings panel in the right sidebar:

- Warns about unmapped actions
- Add/edit/delete mappings
- Address dropdown for `%QX0.0` to `%QX0.7` (EL2008)
- Toggle ON/OFF for booleans
- Marks unused mappings

### JSON format

```json
{
  "actionMappings": {
    "turnOn_LED": {
      "action": "WRITE_OUTPUT",
      "address": "%QX0.0",
      "value": true
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
      "message": "Entering Safe State"
    }
  }
}
```

Supported action types:

- `WRITE_OUTPUT`: write one digital output
- `WRITE_VARIABLE`: write one runtime/ST variable
- `SET_MULTIPLE`: write multiple outputs atomically
- `LOG`: write diagnostic log message

### IEC address patterns

- Digital outputs: `%QX0.0` to `%QX0.7`
- Digital inputs: `%IX0.0` to `%IX0.7`
- Analog outputs: `%QW0`, `%QW1`, ...
- Analog inputs: `%IW0`, `%IW1`, ...

### Guards with hardware inputs

Supported guard examples:

```json
{
  "on": {
    "START": { "target": "Running", "guard": "%IX0.0 == TRUE" },
    "STOP": { "target": "Idle", "guard": "%IX0.1" },
    "OVERHEAT": { "target": "Emergency", "guard": "%IW0 > 100" }
  }
}
```

## Related Docs

- [examples/statecharts/README.md](../../../examples/statecharts/README.md)
- [examples/statecharts/HARDWARE_EXECUTION.md](../../../examples/statecharts/HARDWARE_EXECUTION.md)
- [examples/statechart_backend/README.md](../../../examples/statechart_backend/README.md)

## References

- XState JSON format: <https://xstate.js.org/>
- ReactFlow docs: <https://reactflow.dev/>
- Runtime sources: `crates/trust-runtime/`
- Control project reference: `<control-project-root>`
