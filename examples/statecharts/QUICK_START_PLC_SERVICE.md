# Quick Start: Automatic PLC Service with StateCharts

## Install in 3 Steps

### 1. Prepare the project

```bash
# Move to your project (or use the example project)
cd examples/statechart_backend

# Build bytecode
sudo ../../target/release/trust-runtime build --project .

# Confirm output exists
ls -lh program.stbc
```

### 2. Install the systemd service

```bash
# Run installer from the project directory
sudo ./install-plc-service.sh

# Or run installer against another project path
sudo ../statecharts/install-plc-service.sh /path/to/your/project
```

### 3. Done: PLC auto-start is enabled

```bash
# Check service
sudo systemctl status trust-plc.service

# Follow logs
sudo journalctl -u trust-plc.service -f

# Test I/O command
trust-runtime ctl --project . io-write %QX0.0 TRUE
```

## Full Flow

```text
┌─────────────────────────────────────────────────────┐
│  1. DEVELOPMENT (VS Code)                           │
│  • Edit ST in src/                                  │
│  • Edit .statechart.json files                      │
│  • Configure io.toml for hardware                   │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│  2. BUILD                                            │
│  $ trust-runtime build --project .                  │
│  → Generates program.stbc                           │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│  3. SERVICE INSTALL                                  │
│  $ sudo ./install-plc-service.sh                    │
│  → Creates /etc/systemd/system/trust-plc.service   │
│  → Enables startup on boot                          │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│  4. AUTO START                                       │
│  On Linux boot:                                     │
│  → systemd starts trust-plc.service                │
│  → trust-runtime loads program.stbc                │
│  → EtherCAT master initializes                      │
│  → PLC cycle runs (10ms default)                    │
│  → StateCharts connect over control socket          │
└─────────────────────────────────────────────────────┘
```

## Monitoring

### Status dashboard commands

```bash
sudo systemctl status trust-plc.service
sudo journalctl -u trust-plc.service --since "5 min ago"
trust-runtime ctl --project /opt/trust/production status
trust-runtime ctl --project /opt/trust/production config-get
```

### Real-time logs

```bash
sudo journalctl -u trust-plc.service -f
sudo journalctl -u trust-plc.service -p err -f
sudo journalctl -u trust-plc.service -f -o short-iso
```

## Manual Control

```bash
sudo systemctl stop trust-plc.service
sudo systemctl start trust-plc.service
sudo systemctl restart trust-plc.service
sudo systemctl disable trust-plc.service
sudo systemctl enable trust-plc.service
```

## Production I/O Commands

```bash
trust-runtime ctl --project /opt/trust/production io-read %IX0.0
trust-runtime ctl --project /opt/trust/production io-write %QX0.0 TRUE
trust-runtime ctl --project /opt/trust/production io-read %IW0
trust-runtime ctl --project /opt/trust/production io-force %QX0.1 TRUE
trust-runtime ctl --project /opt/trust/production io-unforce %QX0.1
```

## Software Update Paths

### Method 1: Versioned deploy (recommended)

```bash
cd /path/to/new-version
trust-runtime build --project .
trust-runtime deploy --project . --root /opt/trust
sudo systemctl restart trust-plc.service

# rollback if needed
trust-runtime rollback --root /opt/trust
sudo systemctl restart trust-plc.service
```

### Method 2: In-place update

```bash
sudo systemctl stop trust-plc.service
cd /opt/trust/production
trust-runtime build --project .
sudo systemctl start trust-plc.service
```

## Safety and Watchdog

The default service profile should include:

- `Restart=always`
- `RestartSec=5`
- Runtime watchdog enabled in `runtime.toml`
- Safe output state definitions

```toml
[runtime.watchdog]
enabled = true
timeout_ms = 5000
action = "SafeHalt"  # or Restart/Continue
```

## Production Debugging

```bash
trust-runtime ctl --project /opt/trust/production status
trust-runtime ctl --project /opt/trust/production vars
trust-runtime ctl --project /opt/trust/production io-read %MW100
```

## Apply Config Updates

```bash
trust-runtime ctl --project /opt/trust/production \
  config-set resource.cycle_interval_ms 20

trust-runtime ctl --project /opt/trust/production \
  config-set control.auth_token "new-secure-token"
```

## Remote Access (SSH tunnel)

```bash
ssh -L 9000:127.0.0.1:9000 user@plc-ip
trust-runtime ctl --endpoint tcp://127.0.0.1:9000 status
```

## Test Before Production

### Simulation profile

```bash
# in io.toml
[io.params]
adapter = "mock"

sudo systemctl restart trust-plc.service
```

### Manual dry run

```bash
sudo systemctl stop trust-plc.service
cd /opt/trust/production
sudo trust-runtime --project .
# Ctrl+C to stop
sudo systemctl start trust-plc.service
```
