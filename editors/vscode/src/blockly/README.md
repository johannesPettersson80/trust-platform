# Blockly PLC Editor

Visual programming editor for IEC 61131-3 PLC programs using Google Blockly.

## Overview

The Blockly PLC Editor provides a drag-and-drop visual programming interface for creating PLC programs. Blocks are automatically converted to IEC 61131-3 Structured Text (ST) code that can be executed on real hardware or simulated.
The extension also auto-generates a sibling `.st` companion on save/import so Blockly logic can be mixed into standard ST projects without requiring the custom editor at runtime.

## Features

- **Visual Programming**: Drag-and-drop blocks to create PLC logic
- **Real-time Code Generation**: Instant conversion to Structured Text
- **Execution Modes**:
  - **Simulation**: Test programs without hardware
  - **Hardware**: Execute on real PLCs via trust-runtime
- **Block Categories**:
  - Logic (IF/ELSE, comparisons, boolean operations)
  - Loops (FOR, WHILE, REPEAT)
  - Math (arithmetic, functions)
  - Variables (get/set)
  - Functions (custom blocks)
  - PLC I/O (digital/analog read/write)
  - PLC Timers (TON, TOF, TP)
  - PLC Counters (CTU, CTD, CTUD)

## Getting Started

### Creating a New Blockly Program

1. Open Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`)
2. Run: `Structured Text: New Blockly Program`
3. Enter a program name
4. Select target directory

A new `.blockly.json` source file and `<name>.st` companion will be created. By default the generated `.st` file is opened for ST-first workflows.

### Importing an Existing Program

1. Open Command Palette
2. Run: `Structured Text: Import Blockly Program`
3. Select a `.blockly.json` file to import

## Using the Editor

### Workspace Layout

```
┌─────────────────────────────────────────────────────────────┐
│ Toolbar: [Generate Code] [▶ Simulate] [🔧 Hardware] [Stop]  │
├────────┬────────────────────────────────────────┬───────────┤
│        │                                        │           │
│ Blocks │         Visual Workspace               │Properties │
│ Panel  │     (Drag blocks here)                 │  Panel    │
│        │                                        │           │
│        │                                        │           │
└────────┴────────────────────────────────────────┴───────────┘
```

### Building Programs

1. **Add Blocks**: Drag blocks from left panel to workspace
2. **Connect Blocks**: Snap blocks together to form logic
3. **Configure Properties**: Use right panel to set:
   - Program name and description
   - Variables (name, type)
   - Block-specific settings

### Generating Code

Click **Generate Code** button to:
- Convert blocks to Structured Text
- Validate block connections
- Show warnings for incomplete blocks
- Optionally save as `.st` file

### Executing Programs

#### Simulation Mode

1. Click **▶ Simulate** button
2. Program runs in memory (no hardware required)
3. Use properties panel to monitor variables

#### Hardware Mode

1. Configure `trust-runtime` connection in VS Code settings:
   ```json
   {
     "trust.runtime.controlEndpoint": "unix:///tmp/trust-debug.sock"
   }
   ```
2. Ensure trust-runtime is running
3. Click **🔧 Hardware** button
4. Program executes on connected PLC

## Block Categories

### Logic

- **IF/ELSE**: Conditional branching
- **Comparison**: `=`, `<>`, `<`, `>`, `<=`, `>=`
- **Boolean**: `TRUE`, `FALSE`, `AND`, `OR`, `NOT`

### Loops

- **FOR**: Count-controlled loop
- **WHILE**: Condition-controlled loop
- **REPEAT**: Post-test loop

### Math

- **Arithmetic**: `+`, `-`, `*`, `/`, `**` (power)
- **Functions**: `ABS`, `SQRT`, `SIN`, `COS`, etc.

### Variables

- **Set Variable**: Assign value to variable
- **Get Variable**: Read variable value
- **Create Variable**: Define new variables in properties panel

### PLC I/O

- **Digital Write**: Set output (`%QX0.0 := TRUE`)
- **Digital Read**: Read input (`%IX0.0`)
- **Analog Write**: Set analog output (`%QW0 := 1000`)
- **Analog Read**: Read analog input (`%IW0`)

### PLC Timers

- **TON**: On-delay timer
- **TOF**: Off-delay timer
- **TP**: Pulse timer

### PLC Counters

- **CTU**: Count up
- **CTD**: Count down
- **CTUD**: Count up/down

## File Format

Blockly programs are stored as JSON:

```json
{
  "blocks": {
    "languageVersion": 0,
    "blocks": [
      {
        "type": "controls_if",
        "id": "block_id_1",
        "x": 50,
        "y": 50,
        "inputs": {
          "IF": {
            "block": {
              "type": "logic_compare",
              "fields": { "OP": "EQ" }
            }
          }
        }
      }
    ]
  },
  "variables": [
    {
      "id": "var_1",
      "name": "counter",
      "type": "INT"
    }
  ],
  "metadata": {
    "name": "MyProgram",
    "description": "Example PLC program",
    "version": "1.0.0"
  }
}
```

## Code Generation Examples

### Example 1: Simple Logic

**Blocks:**
```
[IF] %IX0.0 = TRUE
  [THEN] %QX0.0 := TRUE
  [ELSE] %QX0.0 := FALSE
```

**Generated ST:**
```st
PROGRAM MyProgram

VAR
END_VAR

IF %IX0.0 = TRUE THEN
  %QX0.0 := TRUE;
ELSE
  %QX0.0 := FALSE;
END_IF;

END_PROGRAM
```

### Example 2: Counter

**Blocks:**
```
[SET] counter := counter + 1
[IF] counter >= 10
  [THEN] [SET] counter := 0
```

**Generated ST:**
```st
PROGRAM CounterProgram

VAR
  counter : INT;
END_VAR

counter := counter + 1;
IF counter >= 10 THEN
  counter := 0;
END_IF;

END_PROGRAM
```

## Integration with trust-runtime

### Hardware Execution

When running in hardware mode, the editor:

1. Generates ST code from blocks
2. Sends code to trust-runtime for compilation
3. Executes compiled program on PLC
4. Streams I/O updates back to editor

### Configuration

Set runtime connection in `.vscode/settings.json`:

```json
{
  "trust.runtime.controlEndpoint": "unix:///tmp/trust-debug.sock",
  "trust.runtime.authTokenFallback": "optional-auth-token",
  "trust.runtime.requestTimeoutMs": 5000
}
```

### Supported Protocols

- **Unix Socket**: `unix:///tmp/trust-debug.sock`
- **TCP**: `tcp://127.0.0.1:9000`

## Architecture

### Components

```
┌─────────────────────────────────────────────────────────┐
│ VS Code Extension (extension.ts)                        │
│  ├─ BlocklyEditorProvider (blocklyEditor.ts)           │
│  ├─ Command Handlers (newBlockly.ts, importBlockly.ts) │
│  └─ Runtime Client (runtimeClient.ts)                   │
└──────────────────┬──────────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────────┐
│ Webview (React)                                         │
│  ├─ BlocklyEditor.tsx (main component)                 │
│  ├─ ToolboxPanel.tsx (block categories)                │
│  ├─ PropertiesPanel.tsx (settings, variables)          │
│  ├─ CodePanel.tsx (generated ST display)               │
│  └─ useBlockly.ts (state management hook)              │
└──────────────────┬──────────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────────┐
│ Code Generator (blocklyEngine.ts)                       │
│  └─ Converts Blockly blocks → Structured Text          │
└─────────────────────────────────────────────────────────┘
```

### Data Flow

```
User Action → Webview → Extension → Runtime
    ↓           ↓          ↓          ↓
  Blocks    Save JSON   Generate   Execute
            to File      ST Code    on PLC
```

## Development

### Building

```bash
# Build once
npm run build:blockly

# Watch mode
npm run watch
```

### File Structure

```
editors/vscode/src/blockly/
├── blocklyEditor.ts          # Custom editor provider
├── blocklyEngine.ts          # ST code generator
├── runtimeClient.ts          # Runtime communication
├── newBlockly.ts             # New program command
├── importBlockly.ts          # Import program command
├── uriUtils.ts               # URI helpers
├── webview/
│   ├── BlocklyEditor.tsx     # Main React component
│   ├── ToolboxPanel.tsx      # Block toolbox
│   ├── PropertiesPanel.tsx   # Properties editor
│   ├── CodePanel.tsx         # Generated code view
│   ├── hooks/
│   │   └── useBlockly.ts     # React hook
│   ├── types.ts              # TypeScript types
│   ├── main.tsx              # Entry point
│   ├── index.html            # HTML template
│   └── styles.css            # Styles
└── README.md                 # This file
```

## Roadmap

- [ ] Custom block creation
- [ ] Blockly workspace themes
- [ ] Step-by-step debugging
- [ ] Block library import/export
- [ ] PLC function block support
- [ ] IEC 61131-3 standard blocks
- [ ] Simulation visualization
- [ ] Multi-file programs

## Contributing

Contributions welcome! See main [CONTRIBUTING.md](../../../CONTRIBUTING.md).

## License

MIT OR Apache-2.0
