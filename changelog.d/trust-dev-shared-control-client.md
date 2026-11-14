### Fixed

- The agent methods `runtime.reload` and `runtime.compile_reload` of `trust-dev agent serve` send the
  program to a runtime through the shared control client, as `trust-runtime ctl` sends its requests.
  They wait at most 70 seconds for the runtime's answer, where before they could wait without end,
  and they refuse an answer that ends before its line end or carries another request's id
  (specification 11 §6.8.1).
