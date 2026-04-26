# PLCopen Motion OOP Labeling Conveyor

Docs category: `docs/public/examples/libraries-and-motion.md`

Conveyor pacing example using the OOP axis facade. The program powers the axis,
starts a velocity command, watches `itfContinuousAxisCommand.InVelocity`, then
halts the conveyor for a label placement window.

Build it from the repository root:

```bash
trust-runtime build --project examples/plcopen_motion_oop_labeling_conveyor --sources src
```
