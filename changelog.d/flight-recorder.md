### Added

- A runtime keeps a flight recorder when `runtime.toml` sets `[runtime.trace] flight_recorder =
  <n>`: it always keeps the last `n` to `2n` cycles, and when the resource faults it keeps them, up
  to the faulted cycle, as the fault's trace, which `trace.read {"trace": "fault"}` reads until the
  next fault. Its memory is reserved when the resource starts; when the runtime cannot give it,
  the resource starts without a flight recorder and a `runtime_trace_memory` event names the bytes
  needed (specification 48 RU-59; specification 11, "Cycle traces"). A chip keeps one too: it takes
  `flight_recorder` from its deployment record, gives each of the two recordings half the memory
  it has for a recording, and keeps the fault's trace in its memory until the next fault or a
  reset.
- `trust-runtime ctl fault-trace [--out <file>]` saves the flight recorder's trace of the last fault
  as a `.trusttrace` file, by default `fault-<UTC date and time>.trusttrace`, and a runtime started
  on a project also writes it into the project folder as `traces/fault-<UTC date and
  time>.trusttrace`; a copy it cannot write is reported as a `runtime_trace_file_unavailable` event
  (specification 11, "Cycle traces").
