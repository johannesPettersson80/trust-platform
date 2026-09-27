# TwinCAT ADS

This is a Beckhoff ADS starter that is deliberately runnable before a PLC is connected.

- **First action:** Open the folder in VS Code, select the Simulator in the truST project view and press **Start**. Use **Devices & Connections** to inspect or configure connections.
- **Expected result:** The Simulator runs `Main`; `Main.heartbeat` increments each cycle and can be inspected through **Live Values**.
- **Simulation and hardware:** `io.toml` uses simulated I/O out of the box. A real TwinCAT target is optional; configure its ADS connection in **Devices & Connections**, which writes `ads.toml`, before connecting.
- **truST surface:** Use **Devices & Connections** for the ADS connection and **Live Values** for the running program value.
