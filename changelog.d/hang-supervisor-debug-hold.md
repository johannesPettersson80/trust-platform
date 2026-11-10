### Fixed

- The hang supervisor no longer faults a resource with a watchdog timeout when a debugger pause
  begins while the supervisor checks the scan. On a busy computer, the supervisor could count the
  pause as execution time and fault a resource that the debugger was holding.
