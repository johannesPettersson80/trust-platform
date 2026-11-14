### Fixed

- The `trust-runtime ui` console sends its requests to a runtime through the shared control client, as
  `trust-runtime ctl` does. It waits at most 750 milliseconds for a reply, and 70 seconds for the reply
  to its `reload`, where before it could wait without end for a runtime that stopped answering. It
  refuses an answer that ends before its line end or carries another request's id (specification 11
  §6.8.1).
