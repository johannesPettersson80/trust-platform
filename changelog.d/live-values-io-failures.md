### Fixed

- When Live Values' I/O state request times out, the panel clears its values and says that it lost the connection to
  the runtime. Before, the old values stayed under `I/O state request failed: A stale runtime or debug session blocked
  startup.`, a start's message; other failed requests could be told as a start's failure too, such as a missing runtime
  binary or a workspace that is not writable. A failed request now names its own reason, and the debug adapter's
  `no program runs: launch or attach first` asks to Start or Connect like any missing session (specification 25 §4.4).
