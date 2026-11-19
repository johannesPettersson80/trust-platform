### Added

- `trust-service`, the SDK of services: a Rust program beside a runtime connects by its name with the
  settings of `[services.<name>]` in the project's `trust.toml` (the endpoint, the service token's file
  and the heartbeat), reads the program's variables one completed scan at a time as Rust values,
  writes the variables its project allows, reads at a period it chooses, and sends its heartbeat on a
  connection and a thread of its own. `trust-service` and `trust-runtime-client` are published on
  crates.io with each release (specification 48 RU-62).
