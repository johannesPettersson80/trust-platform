### Fixed

- The debugger's session with a running runtime (attach) sends its requests through the shared control
  client, as `trust-runtime ctl` does. It waits at most 750 milliseconds for a reply, and 70 seconds for
  the reply to an online change of the program, where before it could wait without end for a runtime
  that stopped answering. It refuses an answer that ends before its line end or carries another
  request's id (specification 11 §6.8.1).
