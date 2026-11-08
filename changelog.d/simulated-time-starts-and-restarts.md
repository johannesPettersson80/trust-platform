### Fixed

- A simulated runtime's first cycle runs at simulation time zero, however long the runtime took to
  start. At a high time scale the start had counted as missed task intervals, one overrun warning
  (TRUST-RT-OVERRUN-001) right after every start.
- After a restart, simulated plants start at the time the restarted runtime continues from, so its
  first cycle finds them in their initial state, as after a start. Before, they integrated all the
  time since the start in that first cycle, so a tank drained, and past 100 s the plant faulted.
