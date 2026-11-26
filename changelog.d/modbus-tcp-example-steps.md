### Fixed

- The Modbus/TCP example's README shows its whole `io.toml`, the safe state of its output among it, and walks through a
  run against a Modbus/TCP server step by step: the input bit that register value 256 sets, the output the server
  receives, and the safe state the runtime writes when it stops. CI runs the steps against a test server.
