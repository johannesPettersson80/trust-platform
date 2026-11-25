### Added

- The example `fault_trace`: a dosing pump's controller faults when its stroke sensor breaks. Its README
  saves the flight recorder's trace of the fault with `trust-runtime ctl fault-trace`, replays it with
  `trust-dev replay` to see the fault again, and checks a fix against it. Its machine test faults the
  program on purpose, keeps the trace with trust-sim's `fault_trace` and replays it, and CI runs it. The
  documentation's examples page "Test And Debug" shows it.
