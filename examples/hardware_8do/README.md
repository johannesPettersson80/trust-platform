# Hardware Backend: 8 Digital Outputs

Docs category: `docs/public/examples/capstones.md`

This is a **hardware backend** that provides the infrastructure to run programs on physical hardware with 8 digital outputs.

## Purpose

This backend is **NOT** tied to any specific programming language or editor. It provides:
- Hardware I/O drivers (EtherCAT or GPIO)
- PLC runtime environment
- Control endpoint for visual editors
- HMI configuration

## Supported Programs

Any program that uses 8 digital outputs can use this backend:
- **Ladder Logic programs** (`examples/ladder/*.ladder.json`)
- **Blockly visual programs** (`examples/blockly/*.blockly.json`)
- **UML Statechart programs** (`examples/statecharts/*.statechart.json`)
- **Structured Text programs** (`*.st` files)

## Architecture

```
┌─────────────────────────┐
│   Your Program          │
│ (Ladder/Blockly/        │
│  Statechart/ST)         │
└───────────┬─────────────┘
            │
            │ Uses addresses: %QX0.0 - %QX0.7
            │
┌───────────▼─────────────┐
│   hardware_8do          │
│   (This Backend)        │
│                         │
│   ├── io.toml           │ ◄── Defines hardware mapping
│   ├── runtime.toml      │ ◄── Runtime settings
│   └── start.sh          │ ◄── Launch script
└───────────┬─────────────┘
            │
            │ EtherCAT/GPIO
            │
┌───────────▼─────────────┐
│   Physical Hardware     │
│   EK1100 + EL2008       │
│   8 LEDs/Relays/etc     │
└─────────────────────────┘
```

## Hardware Configuration

### Option 1: EtherCAT (Default)
- **Bus Coupler**: Beckhoff EK1100
- **Output Module**: Beckhoff EL2008 (8-channel 24V digital output)
- **Network**: Requires EtherCAT network adapter (check with `ip link show`)

### Option 2: GPIO (Raspberry Pi)
- **Platform**: Raspberry Pi with 8 GPIO pins
- **Pins**: BCM 17, 18, 27, 22, 23, 24, 25, 4
- **Enable**: Uncomment GPIO section in `io.toml`

## Address Mapping

All programs using this backend must use these addresses:

| Address | Description | Physical Pin |
|---------|-------------|--------------|
| %QX0.0  | Digital Output 0 | EL2008 Ch.0 |
| %QX0.1  | Digital Output 1 | EL2008 Ch.1 |
| %QX0.2  | Digital Output 2 | EL2008 Ch.2 |
| %QX0.3  | Digital Output 3 | EL2008 Ch.3 |
| %QX0.4  | Digital Output 4 | EL2008 Ch.4 |
| %QX0.5  | Digital Output 5 | EL2008 Ch.5 |
| %QX0.6  | Digital Output 6 | EL2008 Ch.6 |
| %QX0.7  | Digital Output 7 | EL2008 Ch.7 |

## Usage

### 1. Configure Hardware

Edit `io.toml` to match your network adapter:
```toml
[io.params]
adapter = "enp111s0"  # Change to your adapter (ip link show)
```

### 2. Start the Runtime

```bash
cd examples/hardware_8do
./start.sh
```

Use `sudo` if your EtherCAT/GPIO environment requires elevated permissions.

This will:
- Compile the ST code (if any)
- Start the PLC runtime
- Expose control endpoint at `/tmp/trust-debug.sock`
- Start HMI web interface at `http://localhost:9090`

### 3. Run Your Program

**For Ladder/Blockly/Statechart programs:**
- Open your `.ladder.json`, `.blockly.json` or `.statechart.json` file in VS Code
- Choose hardware mode in the editor toolbar and click run
- The visual editor will connect to the control endpoint and control I/O directly

**For ST programs:**
- Edit `src/Main.st`
- Runtime will automatically reload on file changes

## Directory Structure

```
hardware_8do/
├── io.toml              # Hardware I/O configuration (EtherCAT/GPIO)
├── runtime.toml         # Runtime settings (cycle time, logging)
├── trust-lsp.toml       # LSP configuration
├── start.sh             # Launch script
├── src/
│   ├── Main.st          # Main ST program (placeholder for generated code)
│   └── config.st        # Global configuration
└── hmi/
    ├── _config.toml     # HMI server settings
    ├── overview.toml    # Main dashboard
    ├── trends.toml      # Trend charts
    ├── alarms.toml      # Alarm configuration
    └── process.toml     # Process view
```

## Example Programs Using This Backend

### Ladder Logic
- `examples/ladder/ethercat-snake.ladder.json` - Knight Rider LED pattern

### Statecharts
- `examples/statecharts/ethercat-snake-bidirectional.statechart.json` - Knight Rider
- `examples/statecharts/ethercat-snake.statechart.json` - LED chase

### Blockly
- `examples/blockly/ethercat-snake-bidirectional.blockly.json` - Knight Rider
- `examples/blockly/simple-led-blink.blockly.json` - Simple LED blink

## Control Endpoint

The runtime exposes a Unix socket at `/tmp/trust-debug.sock` that allows:
- Direct I/O manipulation (`writeIo`, `forceIo`, `unforceIo`)
- Runtime inspection
- Variable monitoring

Visual editors (Ladder, Blockly, Statechart) connect to this endpoint to control hardware without generating ST code.

## HMI Dashboard

Access the HMI at `http://localhost:9090` to:
- Monitor digital outputs (DO0-DO7)
- View trends
- Check alarms
- Inspect runtime status

## Troubleshooting

### EtherCAT not working
```bash
# Check network adapter
ip link show

# Verify EtherCAT master loaded
sudo ethercat master

# Check module detection
sudo ethercat slaves
```

### Permission denied on /tmp/trust-debug.sock
```bash
# Check socket exists
ls -la /tmp/trust-debug.sock

# Restart runtime
./start.sh
```

### Runtime not starting
```bash
# Check logs
journalctl -u trust-runtime -f

# Verify compilation
trust-lsp compile src/Main.st
```

## See Also

- [Ladder Logic Examples](../ladder/README.md)
- [Blockly Examples](../blockly/README.md)
- [Statechart Examples](../statecharts/README.md)
- [EtherCAT Setup Guide](../ethercat_ek1100_elx008_v2/README.md)
