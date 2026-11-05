### Fixed

- An HMI agent's cancellation that reaches VS Code before its tool call no longer gets lost: the call then starts
  cancelled and its tool answers at once, instead of running to its end (specification 43 §3.1).
