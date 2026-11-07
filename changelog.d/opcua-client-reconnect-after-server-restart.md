### Fixed

- An OPC UA client connection whose server came back within about two seconds of a loss, as after
  a quick server restart or a short network outage, stayed `reconnecting` until the runtime
  restarted: its recovery reopened the lost session in place, which panicked inside the opcua
  library and ended the connection's worker. The client now closes the lost session and connects a
  new one with its own subscription (specification 11).
