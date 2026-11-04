### Fixed

- The debugger, the console (`trust-runtime ui`) and `trust-dev ctl` send each control request to the runtime in
  one piece. They wrote the request's newline as a write of its own, which the TCP stack held until the runtime
  acknowledged the request: about 40 ms a request from Linux, and up to 200 ms from Windows, where every control
  connection is TCP. A debug stop makes several such requests (specification 11).
