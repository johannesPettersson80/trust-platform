# PLCopen Motion OOP Indexing Table

Docs category: `docs/public/examples/libraries-and-motion.md`

Rotary indexing-table example. It moves the table to four stations in turn with
`MoveAbsolute` through the OOP axis interface, and calls `Release` every cycle
while a move runs, which keeps the command's `Busy` and `Done` up to date and lets
the next move start with a new rising edge.

Build it from the repository root:

```bash
trust-runtime build --project examples/plcopen_motion_oop_indexing_table --sources src
```

Its machine tests run the program with the library's simulated axis and check
that the table stops at 90.0, 180.0, 270.0 and 360.0, then at 90.0 again. In a
clone of the repository, run them from `machine-tests` with `cargo test`; CI runs
them too.
