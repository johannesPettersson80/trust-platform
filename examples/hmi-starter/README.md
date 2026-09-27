# HMI starter

This starter pairs a simulator program with the bundled `overview` HMI view.

- **First action:** Open the folder in VS Code, select the Simulator in the truST project view and press **Start**. Then choose **Open HMI**.
- **Expected result:** The HMI opens without hardware and shows the **Tank level** value, which cycles from 0 through 99 in the Simulator.
- **Simulation and hardware:** `io.toml` selects simulated I/O and the HMI has no hardware prerequisite. Hardware integration is outside this starter's scope.
- **truST surface:** Use **HMI** for the `overview` view; use **Live Values** if you also want to inspect `Main.level` and `Main.running` directly.
