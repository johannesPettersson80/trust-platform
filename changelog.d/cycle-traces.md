### Added

- A computer runtime records cycle traces. `trace.record` starts a recording at the resource's next
  cycle, for a number of cycles, a task's runs or a span of logical time, with all of its memory
  reserved first. Each cycle keeps its task activations, the control link's writes as the bytes the
  variables then hold, its `CURRENT_DT` readings, the input image as the scan sampled it, the output
  image it wrote, and the fault that ended it. `trace.read` gives the recording's state and then its
  `.trusttrace` file in parts, each with its CRC-32, and `trace.stop` ends a recording at once. A
  recording ends early when its resource stops, pauses, restarts or faults, and when its program
  changes online (specification 48 RU-59; specification 11, "Cycle traces").
