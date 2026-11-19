### Added

- A running runtime's fleet topology lists each service that holds a service token as a client of
  the runtime, with the age of its last heartbeat by the runtime's own clock. The service shows as
  connected while its last heartbeat is younger than twice its period, as degraded after that, and
  as configured until its first heartbeat. `service.heartbeat` takes the service's period,
  `period_ms`, which `trust-service` sends (specification 11 §6.8.1a, "Service clients").
