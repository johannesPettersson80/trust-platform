# PLCopen Motion OOP Warehouse Shuttle

Docs category: `docs/public/examples/libraries-and-motion.md`

Storage/retrieval shuttle example using `itfAxis`, returned `itfCommand`, and
returned `itfAxisCommand` objects from the PLCopen OOP motion package.

Build it from the repository root:

```bash
trust-runtime build --project examples/plcopen_motion_oop_warehouse_shuttle --sources src
```

The program powers a simulated shuttle axis, homes it, moves to a storage bay,
releases the active command, then returns to the load station.
