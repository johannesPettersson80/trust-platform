# Blockly PLC Examples

Docs category: `docs/public/examples/visual-editors.md`

Visual programming examples for trust-platform using Google Blockly.

## Available Examples

### 1. Simple LED Blink
**File:** `simple-led-blink.blockly.json`  
**Description:** Basic example that toggles first output every cycle  
**Hardware:** Any PLC with at least 1 digital output  
**Outputs:** `%QX0.0`

Simple test to verify Blockly editor and runtime connection.

### 2. EtherCAT Snake Bidirectional
**File:** `ethercat-snake-bidirectional.blockly.json`  
**Description:** Knight Rider-style LED pattern moving back and forth  
**Hardware:** Beckhoff EK1100 + EL2008 (8-channel digital output)  
**Outputs:** `%QX0.0` through `%QX0.7`

Creates a moving LED pattern that:
- Starts at position 0 (first LED)
- Moves forward through all 8 positions
- Reverses direction at ends
- Repeats infinitely

## How to Use

### 1. Open in VS Code

1. Open VS Code in trust-platform workspace
2. Install trust-lsp extension (or run in development mode)
3. Open any `.blockly.json` file
4. Blockly visual editor will open automatically

### 2. Edit in Blockly Editor

- **Toolbox (Left)**: Drag blocks to workspace
- **Workspace (Center)**: Arrange and connect blocks
- **Properties (Right)**: Configure variables and metadata
- **Generate Code**: Click button to see ST output

### 3. Execute

#### Simulation Mode (No Hardware)

```bash
# From VS Code editor
Click: ▶ Simulate
```

Program runs in memory without physical I/O.

#### Hardware Mode (Real PLC)

**Option A: Using hardware_8do backend**

```bash
cd examples/hardware_8do
./start.sh
```

Then in VS Code editor:
```
Click: 🔧 Hardware
```

**Option B: Manual Runtime**

```bash
# Terminal 1: Start runtime with I/O
cd examples/hardware_8do
trust-runtime -c runtime.toml

# Terminal 2: Generate and deploy ST code
# (From Blockly editor: Generate Code → Save as .st)
# Copy generated .st to src/ folder
trust-runtime -c runtime.toml  # Hot reload
```

## Hardware Setup

### EtherCAT (Recommended)

**Required:**
- Beckhoff EK1100 bus coupler
- Beckhoff EL2008 (8-channel 24V digital output)
- EtherCAT-capable network interface

**Wiring:**
```
[PC EtherCAT Port] ──→ [EK1100] ──→ [EL2008] ──→ [8x LEDs/Relays]
                                          │
                                    24V Power Supply
```

**Configuration:**

Edit `examples/hardware_8do/io.toml`:
```toml
[io.params]
adapter = "enp111s0"  # Your EtherCAT interface
```

Find your interface:
```bash
ip link show
```

### GPIO (Raspberry Pi, etc.)

For testing with Raspberry Pi or compatible boards.

Edit `examples/hardware_8do/io.toml`:
```toml
[io]
driver = "gpio"

[io.params]
chip = "/dev/gpiochip0"

[[io.params.output]]
line = 17  # Maps to %QX0.0
```

## Block Categories

### PLC I/O Blocks

**Digital Write**
- Sets digital output
- Example: `%QX0.0 := TRUE`

**Digital Read**
- Reads digital input
- Example: `button := %IX0.0`

**Analog Write**
- Sets analog output (0-32767)
- Example: `%QW0 := 1000`

**Analog Read**
- Reads analog input
- Example: `sensor := %IW0`

### Logic Blocks

- IF/ELSE conditions
- Comparisons (=, <>, <, >, <=, >=)
- Boolean operations (AND, OR, NOT)

### Loop Blocks

- FOR loops
- WHILE loops
- REPEAT loops

### Variable Blocks

- Set variable value
- Get variable value
- Create variables in Properties panel

### Math Blocks

- Arithmetic (+, -, *, /, **)
- Functions (ABS, SQRT, etc.)

## Address Mapping

### EL2008 Digital Outputs

| Output | IEC Address | Description      |
|--------|-------------|------------------|
| DO0    | `%QX0.0`    | First LED/Relay  |
| DO1    | `%QX0.1`    | Second LED/Relay |
| DO2    | `%QX0.2`    | Third LED/Relay  |
| DO3    | `%QX0.3`    | Fourth LED/Relay |
| DO4    | `%QX0.4`    | Fifth LED/Relay  |
| DO5    | `%QX0.5`    | Sixth LED/Relay  |
| DO6    | `%QX0.6`    | Seventh LED/Relay|
| DO7    | `%QX0.7`    | Eighth LED/Relay |

## Creating New Examples

### 1. From VS Code

```
Ctrl/Cmd+Shift+P → Structured Text: New Blockly Program
```

Enter name and save in `examples/blockly/`

### 2. Build Your Program

- Add blocks from toolbox
- Connect blocks logically
- Define variables in Properties panel
- Test with "Generate Code"

### 3. Document

Add entry to this README with:
- Filename
- Description
- Required hardware
- I/O addresses used

## Troubleshooting

### Editor Won't Open

1. Ensure VS Code has trust-lsp extension
2. File must have `.blockly.json` extension
3. Check file is valid JSON

### Hardware Not Responding

1. Check `hardware_8do` backend is running
2. Verify adapter name in `io.toml`
3. Check EtherCAT permissions:
   ```bash
   sudo setcap cap_net_raw+ep $(which trust-runtime)
   ```

### Blocks Don't Generate Code

1. Ensure all blocks are connected
2. Check for incomplete block inputs
3. Review warnings in Code Panel

## References

- [Blockly Developer Guide](https://developers.google.com/blockly)
- [IEC 61131-3 Structured Text](https://en.wikipedia.org/wiki/Structured_text)
- [trust-platform Documentation](../../README.md)
- [Blockly Editor README](../../editors/vscode/src/blockly/README.md)

## License

MIT OR Apache-2.0
