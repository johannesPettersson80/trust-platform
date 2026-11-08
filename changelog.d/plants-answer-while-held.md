### Fixed

- While the debugger holds the scan at a breakpoint, a pause or a step, `plant.status`,
  `plant.trace.read` and `plant.trace.clear` answer from the simulation, and a finite search
  (Control Studio's automatic Tune) can start; before, each waited 250 ms for the held scan thread
  and gave up (`runtime_evidence_unavailable`, "native simulation host status timed out").
