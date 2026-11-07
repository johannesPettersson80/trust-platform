### Fixed

- An OPC UA client connection whose read values did not change for its timeout was reported stale,
  and its points' values were no longer applied, though the server was healthy. Its subscription now
  also watches the server's current time as a heartbeat, so a steady value stays live and a stale
  connection means that the subscription stopped; when notifications resume, the connection and its
  points are live again with their last values. The subscription publishes at the connection's
  `poll_interval_ms`, which it ignored before (it published every 250 ms), and a connection is
  stale only after at least three publishing intervals without a notification. A point whose server
  reports a bad status no longer makes the whole connection stale or faulted (specification 11).
