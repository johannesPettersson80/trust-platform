# PLCopen Motion

This single-axis starter exercises the bundled PLCopen motion blocks on the Simulator.

- **First action:** Open the folder in VS Code and press **Start** in the truST project view.
- **Expected result:** The Simulator powers the virtual axis, commands position 25, and eventually exposes `motion_demo_power_on`, `motion_demo_target_ok`, `motion_demo_standstill`, and completed `motion_demo_cycles` in **Live Values**; `motion_demo_error` remains 0 for a healthy run.
- **Simulation and hardware:** `io.toml` uses simulated I/O. This example does not prove or require a physical drive; connect real motion hardware only through a separately configured target.
- **truST surface:** Use **Live Values** to observe the mapped motion status at `%QX0.0`, `%QX0.1`, `%QX0.2`, `%MW0`, and `%MW2`.
