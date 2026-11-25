# Cycle Traces After A Fault

When a program faults on the machine, the runtime's flight recorder has kept the cycles before the
fault. Save that trace, then replay it on the engineering computer to see the fault again, without
the machine and without the conditions that caused it.

## Keep a flight recorder

Add the flight recorder to the project's `runtime.toml`:

```toml
[runtime.trace]
flight_recorder = 300
```

The runtime then always keeps the last 300 to 600 cycles: at a cycle interval of 10 ms, the last 3 to
6 seconds. Choose a number of cycles that covers the time a fault takes to build up, such as a level
that falls for a few seconds before a trip. When a cycle faults, the runtime keeps the cycles up to the
faulted one as the fault's trace.

The runtime reserves the flight recorder's memory when the resource starts, and again at each restart
and online change. When the computer or chip cannot give that memory, the resource starts without a
flight recorder, and the event `runtime_trace_memory` names the bytes needed and the bytes free:

```text
trust-runtime ctl --project ./my-plc events
```

A chip takes `flight_recorder` from the project when the program is built and installed. Its two
recordings each take the share of half the memory the chip has for a recording, so that a recording on
demand still fits beside them.

## Save the fault's trace

1. Make the machine safe as the site's procedure says, before anything else
   ([Field Fault Procedures](field-fault-procedures.md)).
2. Save the trace. A computer's runtime has already written it into the project's folder, as
   `traces/fault-<UTC date and time>.trusttrace`. To save it from a running runtime, or from a chip:

    ```text
    trust-runtime ctl --project ./my-plc fault-trace --out pump-fault.trusttrace
    trust-runtime ctl --endpoint serial:///dev/ttyACM0 fault-trace --out pump-fault.trusttrace
    ```

    The first saves it from the runtime of the project `./my-plc`, the second from a chip on its USB
    link. Each checks every part's CRC-32 and the whole file's SHA-256, then prints the trace's cycles,
    why it ended and its size, such as
    `saved the fault's trace of 501 cycles (faulted) to pump-fault.trusttrace, 53584 bytes`.

3. Keep the file with the fault's report. A computer's runtime keeps the fault's trace until the next
   fault; a chip keeps it until the next fault or a reset.

## Replay it on the engineering computer

```text
trust-dev replay pump-fault.trusttrace --project ./my-plc
```

`trust-dev replay` builds the project as it is now and runs it in a simulator, on a clock that only the
replay moves. It gives each cycle the recorded inputs, changes from outside and clock readings, and
compares the cycle's outputs, and its fault, with the trace's.

- **Every cycle matches.** It prints, for example,
  `Replayed 501 cycles of the same program: every cycle's outputs match the trace.` and ends with 0.
  The program does on the computer exactly what it did on the machine, the fault included: the fault
  comes from the program and the inputs the trace holds.
- **A cycle differs.** It prints the first cycle that differs and its logical time, each output that
  differs with the trace's value and the build's, and how many later cycles differ, and ends with 1. A
  changed program is named as one. For the same program, every difference is a determinism fault or an
  input the trace does not hold (specification 44 NX-04).
- **The trace or the build cannot be replayed.** It ends with 2 and the reason.

A trace from a chip replays the same way: the report then names the build "the same program for
another device".

## Record on demand

To keep a behaviour that is not a fault, such as a start-up, record it while it happens:

```text
trust-runtime ctl --project ./my-plc record --duration 30s --out start-up.trusttrace
```

`trust-dev replay` replays it like a fault's trace.

## Make the trace a test

A machine test replays a trace under `cargo test`, so that every later change of the program is
checked against what the machine did: see [Machine Tests](../develop/machine-tests.md). The example
**Fault trace** on [Test And Debug](../examples/test-and-debug.md) goes through all of it: a fault,
its trace saved and replayed, and a fix checked against it.

## Related

- [Field Fault Procedures](field-fault-procedures.md)
- [Errors And Recovery](errors-and-recovery.md)
- [`runtime.toml`: `[runtime.trace]`](../reference/config/runtime-toml.md)
- [`trust-runtime ctl`](../reference/cli/trust-runtime.md) and [`trust-dev replay`](../reference/cli/trust-dev.md)
