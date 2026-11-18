### Added

- The control request `variables.read` reads a set of the program's variables by their paths: the
  values one completed scan left, never half a scan, with the scan's number, its logical time, the
  resource's state and whether the values are current (`good`) or the last scan's while the
  resource does not run (`stale`). A Viewer may send it and it works with the debug surface off,
  so a service can read a production deployment; computers and chips answer it alike.
  `trust-runtime ctl read <path>...` prints the values with their types (specification 11 §6.8.1,
  specification 48 RU-62).
