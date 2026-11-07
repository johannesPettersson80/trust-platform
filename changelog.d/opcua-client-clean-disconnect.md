### Fixed

- Closing an OPC UA client connection while its server was still reachable, as a runtime does when
  it stops and before it reconnects, could abort the runtime process: the session's run loop
  reconnected the session that the client was closing, and the opcua library panicked when it
  dropped the half-reopened session. The client now stops the run loop before it closes the
  session (specification 11).
