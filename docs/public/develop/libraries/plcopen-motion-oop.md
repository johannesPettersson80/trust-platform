# PLCopen Motion OOP

The PLCopen Motion OOP package is the object-oriented companion to the classic
PLCopen function-block packages. It exposes `itfAxis`, command interfaces, and
`MC_OopAxis` for projects that prefer command objects and properties.

## Worked Example

The example [Indexing Table](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/plcopen_motion_oop_indexing_table)
turns a rotary table from station to station: it moves the table's axis to
90.0, 180.0, 270.0 and 360.0 in turn, and then to 90.0 again. Its program uses
this package's object surface:

- `MC_OopAxis.Bind` ties the axis object to a truST axis slot on the first
  cycle, and the program then keeps the object behind the PLCopen interface
  `itfAxis` (`AxisObject := TableAxis`).
- `Refresh`, called every cycle, reads the axis's actual values, its status and
  its errors into the object's properties.
- `Power` returns the power command, an `itfCommand`, and the program waits for
  its `Done`.
- `MoveAbsolute` starts the move to a station and returns its command object,
  an `itfAxisCommand`, whose `Busy` and `Done` follow the move. A command starts
  on the rising edge of its `Execute`, as the classic `MC_MoveAbsolute` does.
- `Release`, called every cycle while the move runs, runs the active command's
  block with `Execute` FALSE and copies its outputs into the command object, so
  that `Busy` and `Done` follow the move. With `Execute` back at FALSE, the next
  `MoveAbsolute` starts with a new rising edge. A program that calls neither
  `Release` nor the command's method again sees no `Done`: with step 21's
  `Release` taken out, the table never counts an index.

This is the program as the example ships it:

```iecst
--8<-- "examples/plcopen_motion_oop_indexing_table/src/Main.st"
```

The example's machine tests, in `machine-tests/tests/table.rs`, run this program
with the package's simulated axis on a clock that only the test moves, and check
that the table stops at each station in turn; CI runs them. In a clone of the
repository, run them with `cargo test` in the example's `machine-tests` folder:
they take `trust-sim` from the repository, and run the `trust-runtime` that the
environment variable `TRUST_RUNTIME` names, else the one on your `PATH`, else
your truST extension's, which must be of the clone's version.

## Library Guide

--8<-- "docs/guides/PLCOPEN_MOTION_OOP_LIBRARY_GUIDE.md:3"

## Related

- [PLCopen Motion](plcopen-motion.md)
- [Libraries overview](index.md)
- [Libraries and Motion examples](../../examples/libraries-and-motion.md)
