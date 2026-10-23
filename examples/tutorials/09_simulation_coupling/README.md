# Tutorial 09: Simulation Coupling

This project connects a simulated `%IW0` tank-level input to a `%QX0.0` high
level alarm. It is complete and runnable without external hardware.

Open this folder as the active truST project, or open `src/main.st` from a
workspace that contains the folder. Then use **Structured Text: Compile** and
open the Runtime Panel.

You can also verify the project from a terminal:

```bash
trust-runtime check --project examples/tutorials/09_simulation_coupling
trust-runtime build --project examples/tutorials/09_simulation_coupling
trust-runtime play --project examples/tutorials/09_simulation_coupling --simulation
```

While the runtime is active, write a `WORD` value below 500 to `%IW0` and
observe `%QX0.0` remain false. Write 500 or greater and observe the alarm turn
true.

Challenge: add a second warning threshold and map it to `%QX0.1`.
