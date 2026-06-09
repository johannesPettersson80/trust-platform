# OpenOT Multi-PROGRAM Logging

This project demonstrates the truST OpenOT authoring path with two `PROGRAM`
blocks publishing into one OpenOT shared-memory ring.

- `Filler` tags a message, state enum, and level value.
- `Quality` tags a message, integer value, audited setpoint value, and alarm.
- The source has no logging calls, no `Op :=`, and no user-visible OpenOT ids.
- truST generates `Filler.OotProducer` and `Quality.OotProducer`.
- `runtime.toml` drains those generated producers with `producer_instances`.
- `trust-lsp.toml` pulls in the sibling OpenOT ST producer sources from
  `open-ot-ref/st/iec61131`.

## Build

From the repository root:

```bash
trust-runtime build --project examples/openot_multi_program --sources src
```

This generates `program.stbc` in the example directory.

## Run

```bash
trust-runtime run --project examples/openot_multi_program
```

The runtime publishes OpenOT records to `openot-multi-program.shm` using:

```toml
[runtime.openot]
enabled = true
source = "st-fb"
producer_instances = ["Filler.OotProducer", "Quality.OotProducer"]
```

The runtime is still the single writer to the ring. The per-program ST
producers are drained sequentially, so consumers separate the stream by
`sourceId` and per-source `seq`.

The sibling OpenOT workbench keeps the reference copy of this source shape in
`open-ot-ref/examples/multi-program/`.
