# Raspberry Pi (EtherCAT / GPIO)

This starter demonstrates a simulator-first field-I/O project.

- **First action:** Open the folder in VS Code, select the Simulator in the truST project view and press **Start**. Use **Devices & Connections** to inspect or configure connections.
- **Expected result:** The Simulator runs `Main`; `ticks` increments and `output_on` alternates on and off, visible in **Live Values**.
- **Simulation and hardware:** `io.toml` uses simulated I/O, so no Raspberry Pi, EtherCAT coupler, or GPIO wiring is required for the first run. For real field I/O, configure the target in **Devices & Connections** (or replace the documented `[io]` block) and commission the hardware separately.
- **truST surface:** Use **Devices & Connections** for the target/I/O setup and **Live Values** for the simulated program values.
