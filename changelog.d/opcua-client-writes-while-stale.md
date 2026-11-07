### Fixed

- An OPC UA client connection whose read values had not changed for its timeout (2 seconds by
  default) was marked stale and then held back the program's outputs until a read value changed on
  the server, which a server with steady values never sends. The client now writes the outputs
  while its session is up, whether the connection is connected or stale (specification 11).
