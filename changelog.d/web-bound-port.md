### Fixed

- With the web interface on port 0 (`listen = "127.0.0.1:0"`), the system chooses a free port. The
  `Open:` line that `trust-runtime play` prints and the runtime's web endpoint in the fleet topology
  (`fleet.topology`, Devices & Connections) now show that port instead of `:0`. The configured
  value stays in the endpoint's `params.listen` (specification 11, fleet topology).
