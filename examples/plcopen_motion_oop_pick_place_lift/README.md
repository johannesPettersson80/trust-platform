# PLCopen Motion OOP Pick-And-Place Lift

Docs category: `docs/public/examples/libraries-and-motion.md`

Vertical lift example for a pick-and-place station. It homes the lift, moves to
pick height, uses `SetPosition` to re-zero after a simulated tooling offset,
then moves to place height.

Build it from the repository root:

```bash
trust-runtime build --project examples/plcopen_motion_oop_pick_place_lift --sources src
```
