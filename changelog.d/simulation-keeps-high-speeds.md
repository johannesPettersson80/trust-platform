### Fixed

- A simulation at a high time scale keeps its speed. Where cycles are due less than 2 ms apart, the
  scheduler runs the due cycles back to back instead of waiting for each, and a wait that wakes late
  by less than 2 ms costs no speed. Before, every wait woke late by up to about a millisecond, on
  Windows especially, and each late wake restarted the schedule, so a 10 ms task ran at most about
  ten times faster than real time whatever the time scale. Simulation time and results are
  unchanged.
