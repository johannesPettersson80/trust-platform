### Fixed

- `trust-runtime fleet start`, `stop` and `status` send their control requests through the shared control
  client, as `trust-runtime ctl` does, and refuse an answer that ends before its line end, where they took
  it as complete. A stop still asks again when the runtime breaks off the status read while it stops
  (specification 11 §6.8.3).
