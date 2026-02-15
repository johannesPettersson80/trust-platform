# StateChart Backend

**Purpose:** Minimal trust-runtime project that provides hardware I/O control for StateChart editor.

## What is this?

This is **NOT** a standalone program. It's a backend service that:

1. Runs trust-runtime with EtherCAT/GPIO drivers
2. Exposes a control endpoint at `/tmp/trust-debug.sock`
3. Receives `io.force` commands from the VS Code StateChart editor
4. Controls real hardware outputs (LEDs, relays, etc.)

## Architecture

```
┌─────────────────────┐         Control Endpoint        ┌─────────────────────┐
│  VS Code Editor     │◄────────────────────────────────►│  statechart_backend │
│                     │   /tmp/trust-debug.sock          │  (trust-runtime)    │
│  .statechart.json   │                                  │                     │
│  ┌───────────────┐  │   io.force %QX0.0 TRUE          │  EtherCAT Driver    │
│  │ ●→○→○         │  │─────────────────────────────────►│  ┌──────────────┐  │
│  │ LED snake     │  │                                  │  │ EK1100+EL2008│  │
│  └───────────────┘  │   Response: OK                   │  └──────┬───────┘  │
└─────────────────────┘◄─────────────────────────────────└─────────┼──────────┘
                                                                    │
                                                          ┌─────────▼─────────┐
                                                          │  🔴🟢🔵 Hardware   │
                                                          └───────────────────┘
```

## Hardware Support

### EtherCAT (Default)

**Hardware:** Beckhoff EK1100 bus coupler + EL2008 (8-channel digital output)

Configure in `io.toml`:
```toml
[io]
driver = "ethercat"
[io.params]
adapter = "enp111s0"    # Change to match your network interface
```

**Find your interface:**
```bash
ip link show
```

### GPIO (Raspberry Pi)

Uncomment GPIO section in `io.toml`:
```toml
[io]
driver = "gpio"
[io.params]
chip = "/dev/gpiochip0"
```

## Quick Start

### 1. Configure Hardware

Edit `io.toml` and update the network interface or GPIO chip.

### 2. Build

```bash
cd examples/statechart_backend
trust-runtime build --project .
```

Or use the latest version:
```bash
../../target/release/trust-runtime build --project .
```

### 3. Start Backend (with sudo for hardware access)

```bash
sudo ./start.sh
```

The runtime will:
- ✅ Create control endpoint: `/tmp/trust-debug.sock`
- ✅ Initialize EtherCAT/GPIO driver
- ✅ Set socket permissions for user access
- ✅ Wait for StateChart commands

### 4. Use with StateChart Editor

In VS Code:
1. Press `F5` to launch Extension Development Host
2. Open any `.statechart.json` file
3. Select **🔌 Hardware** mode
4. Click **▶️ Start Hardware**
5. You should see: `✅ Connected to trust-runtime`
6. Send events (START, TICK) to control hardware

## What's in src/Main.st?

A **minimal program** that:
- Defines output variables (DO0-DO7)
- Maps them to physical addresses (%QX0.0 - %QX0.7)
- Does NOT contain any control logic
- Lets StateChart editor take full control via `io.force`

The program just provides the I/O infrastructure.

## Files

```
statechart_backend/
├── src/
│   ├── Main.st           # Minimal program with I/O variables
│   └── config.st         # Configuration and mappings
├── io.toml              # Hardware driver configuration
├── runtime.toml         # Runtime settings + control endpoint
├── trust-lsp.toml       # Project metadata
├── start.sh             # Startup script with permission handling
└── README.md            # This file
```

## Troubleshooting

### Permission Error: EACCES /tmp/trust-debug.sock

**Cause:** Socket created by root, your user can't connect.

**Solution:** Script handles this automatically, but if manual:
```bash
sudo chmod 666 /tmp/trust-debug.sock
```

### EtherCAT Error: No such device

**Cause:** Wrong network interface name.

**Solution:**
1. Find your interface: `ip link show`
2. Update `io.toml`: `adapter = "your-interface-name"`

### Failed to connect to trust-runtime

**Cause:** Backend not running.

**Solution:**
```bash
# Check if running
ps aux | grep trust-runtime

# Check socket exists
ls -la /tmp/trust-debug.sock

# Restart backend
sudo pkill trust-runtime
sudo ./start.sh
```

## Related Files

- **StateChart examples:** `../statecharts/*.statechart.json`
- **Editor code:** `../../editors/vscode/src/statechart/`
- **Action mappings:** Defined in `.statechart.json` files

## See Also

- [StateChart Examples README](../statecharts/README.md)
- [Hardware Execution Guide](../statecharts/HARDWARE_EXECUTION.md)
- Trust Platform documentation: `../../docs/`
