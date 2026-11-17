### Added

- Each service that a project's `trust.toml` declares (`[services.<name>]`) now has its heartbeat
  global, `<name>_heartbeat : UDINT`, for a program to gate on with `SERVICE_FRESH`.
  `trust-dev rust declarations` writes the globals into the generated `src/generated/services.st`
  (`src/generated/trust/services.st` in a Rust-first project), whether the project has a Rust crate
  or not, and the build refuses a project whose committed file is out of date, naming the command
  (specification 48 RU-63).
