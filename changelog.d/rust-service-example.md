### Added
- The example `rust_service`, **Rust service** in the example gallery: a Rust service beside the
  runtime fetches the electricity price over HTTP and writes a water heater's setpoint through the
  variable its `writes` names and its typed bindings, and the program takes the setpoint only while
  `SERVICE_FRESH` says the service's heartbeat is fresh, falling back to a safe setpoint when the
  service stops. A stand-in price source lets it run with no internet service.
