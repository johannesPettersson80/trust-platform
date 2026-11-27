### Fixed

- The MQTT example's README walks through a run against a broker step by step, with the values of a real run: the
  output byte the runtime publishes at every cycle, the input byte 1 that sets `%IX0.0`, the input that keeps the last
  message's value, and the safe state the runtime publishes when it stops. Its Try-its show a byte that sets another
  bit, `stale_after_ms` faulting a stale input, and a broker lost under `warn`. CI runs the steps against a test
  broker.
