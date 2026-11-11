### Fixed

- A function block or class instance, or a structure or an array that holds one, is now copied whole by an assignment
  from a variable of its type and by an output of its type bound with `=>`, as in WAGO's `fbDevice(oStatus =>
  fbResult)`: every variable of it, the internal ones and a standard block's state included. The build refused both
  with "... that holds a function block or class instance is not compiled yet" (#451).
