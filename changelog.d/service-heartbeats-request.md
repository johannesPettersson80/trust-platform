### Added

- A service's heartbeat reaches its program: the control request `service.heartbeat`, sent with the
  service's own service token, adds one to the program's global `<name>_heartbeat` at the next cycle's
  start, which `SERVICE_FRESH` watches. Only the service's token can send it, so no other client keeps
  a dead service looking alive, and a service that only reads keeps a Viewer token. Computers and
  boards alike (specification 11 §6.8.1, "Service heartbeats"; specification 48 RU-63).
