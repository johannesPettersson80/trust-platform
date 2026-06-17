# VS Code Communication Panel

Use **Structured Text: Communication** in VS Code when you need to connect a
truST project to another runtime, plant system, broker, PLC, fieldbus, or local
I/O driver.

The panel is the primary setup surface for development. The runtime browser UI
is still available for commissioning and field changes when VS Code is not
installed, but normal project setup should start in VS Code.

## What It Does

- Shows every supported communication family in one place.
- Uses plain-language intent rows before protocol names.
- Reads the selected runtime from the Runtime pane. There is no second runtime
  chooser.
- Shows one status vocabulary across protocols: not in build, not configured,
  simulate mode, unreachable, connected, degraded, error, or configured policy.
- Uses runtime-owned schemas for setup fields, defaults, validation, and config
  snippets. VS Code renders the form; the runtime owns the contract.

## Common Choices

| Need | Card |
| --- | --- |
| connect to a Beckhoff/TwinCAT PLC or expose truST to TwinCAT | ADS / TwinCAT |
| expose runtime variables to SCADA, HMI, or historian software | OPC UA |
| read/write register-oriented equipment | Modbus TCP |
| publish or subscribe through a broker | MQTT |
| connect truST runtimes | Discovery, Mesh / Zenoh, Realtime T0, Runtime cloud |
| wire local hardware or test without hardware | EtherCAT, GPIO, Simulated I/O, Loopback I/O |
| publish telemetry/evidence records | OpenOT |

## Setup Behavior

I/O drivers such as Modbus TCP, MQTT, EtherCAT, GPIO, simulated, and loopback
use native VS Code forms backed by `comm.schema` and `comm.apply`.

Protocols whose deep native apply flow is not complete yet still use the same
runtime schema, but return a validated `runtime.toml` snippet. Paste the snippet
into the project config and restart or deploy the runtime.

Secret fields are blocked over untrusted remote plain-TCP control channels.
Use a same-host runtime endpoint or apply the generated snippet on the runtime
host.

## Next

- [Protocol Matrix](protocol-matrix.md)
- [External Systems](external-systems/index.md)
- [Runtime To Runtime](runtime-to-runtime/index.md)
- [Devices And Fieldbus](devices-and-fieldbus/index.md)
