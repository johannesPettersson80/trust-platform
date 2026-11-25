### Added

- A runtime keeps a flight recorder when `runtime.toml` sets `[runtime.trace] flight_recorder =
  <n>`: it always keeps the last `n` to `2n` cycles, and when the resource faults it keeps them, up
  to the faulted cycle, as the fault's trace, which `trace.read {"trace": "fault"}` reads until the
  next fault. Its memory is reserved when the resource starts; when the computer cannot give it,
  the resource starts without a flight recorder and a `runtime_trace_memory` event names the bytes
  needed (specification 48 RU-59; specification 11, "Cycle traces").
