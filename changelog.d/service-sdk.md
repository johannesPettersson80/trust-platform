### Added

- `trust-service`, the SDK of services: a Rust program beside a runtime connects by its name with
  the settings of `[services.<name>]` in the project's `trust.toml` (the service token's file, the
  heartbeat and, when the runtime is elsewhere, its endpoint; without one the service connects to
  the endpoint its project's running runtime publishes, so the runtime keeps its default endpoint),
  reads the program's variables one completed scan at a time as Rust values, writes the variables
  its project allows, reads at a period it chooses, and sends its heartbeat on a connection and a
  thread of its own. `trust-service` and `trust-runtime-client` are published on crates.io with each
  release (specification 48 RU-62).
