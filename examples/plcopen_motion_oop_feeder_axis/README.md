# PLCopen Motion OOP Feeder Axis

Docs category: `docs/public/examples/libraries-and-motion.md`

Feeder/unwind axis example using OOP parameter methods and continuous velocity
control. The example writes a maximum velocity parameter, powers the axis, then
starts a feed velocity command and checks `InVelocity`.

Build it from the repository root:

```bash
trust-runtime build --project examples/plcopen_motion_oop_feeder_axis --sources src
```
