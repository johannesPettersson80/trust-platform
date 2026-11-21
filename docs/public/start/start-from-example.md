# Start From An Example

The fastest way to learn truST is to start from a bundled example — an editable working copy you can run
immediately, no config to write.

## Open the example gallery

With no project open, the truST view offers **Start from example** first, then **+ Create project** and
**Open project**.

![truST with no project open: Start from example, Create project and Open project](../assets/images/vscode/vscode-create-no-project.png)

*With no project open, **Start from example** copies a ready-made starter.*

Click it to open the searchable example gallery. Hardware and category filters can be combined, and each
card says how its example runs out of the box: **Runs in simulator** for an example whose I/O is simulated,
and what hardware it can connect to later. Pick one, choose a destination folder, and truST copies an
**editable** working copy there and opens its source. You never edit a `.toml` to get going: every starter
runs in the simulator right away.

## The bundled starters

Each of these cards says **Runs in simulator**:

| Example | Hardware | What it is |
| --- | --- | --- |
| **Empty simulator** | No hardware needed | A minimal runnable project — press **Start** to simulate immediately. |
| **Conveyor demo** | No hardware needed | A small, realistic program with live values to watch it run. |
| **Rust function block** | No hardware needed | A Structured Text program that calls a function block written in Rust. |
| **TwinCAT ADS** | Hardware optional: TwinCAT | Starter for a Beckhoff TwinCAT (ADS) device — runs in the simulator; wire the real PLC in Devices & Connections. |
| **Raspberry Pi (EtherCAT / GPIO)** | Hardware optional: Raspberry Pi | Starter for a Raspberry Pi field-IO target — runs in the simulator; wire real I/O in Devices & Connections. |
| **PLCopen Motion single axis** | No hardware needed | Portable single-axis motion starter using the bundled PLCopen Motion library. |

The hardware badge tells you what you can add: a starter with **Hardware optional** opens and runs in the
simulator like the others, and you connect the real device later from
[Devices & Connections](../connect/communication-panel.md). A card that says **Requires** a device needs it
before it runs.

## Next

- Run it: [Run & Check](../operate/debugging-and-runtime-panel.md)
- Watch it: [Live Values](../operate/live-values.md)
- Wire real hardware: [Add A Device](../connect/add-a-device.md)
- Start from scratch instead: [Create A New Project](create-new-project.md)
