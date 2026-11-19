### Added

- `trust-dev service install <name>` puts a service of the project under the computer's service
  manager as `trust-service-<name>` and starts it: a systemd unit on Linux, a launchd daemon on
  macOS. The service starts at every boot without a login and again 2 s after it ends, and runs
  the release binary of its package as the user who owns the project folder, never as root. The
  command refuses a service that could not connect or has no built binary. Run without root, it
  changes nothing and prints the `sudo` command to run. `trust-dev service uninstall <name>` stops
  and removes the service (specification 48 RU-64).
