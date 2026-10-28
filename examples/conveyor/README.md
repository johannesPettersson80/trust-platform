# Conveyor demo

This starter shows a small cyclic program and its mapped process-image values.

- **First action:** Open this example folder in VS Code and press **Start** in the truST project view.
- **Expected result:** The Simulator runs `MainProgram`; in **Live Values**, `running` is true, `conveyor_speed` ramps to 100, then `part_count` increments and the speed resets. `%IX0.0` (`e_stop`) stops the line.
- **Simulation and hardware:** `io.toml` uses simulated I/O and the safe state drives `%QX2.0` false on stop. No hardware is required or claimed by this example.
- **truST surface:** Open **Live Values** after the simulator starts to watch `%QX2.0`, `%QW0`, and `%MW0`.
