### Added

- `trust-dev service install <name>` puts a service of the project under the computer's service
  manager as `trust-service-<name>` and starts it: a systemd unit on Linux, a launchd daemon on
  macOS, a Windows service. The service starts at every boot without a login and again 2 s after
  it ends. It runs the release binary of its package as the user who owns the project folder, or on
  Windows as its own virtual account, never as root or LocalSystem. The command refuses a service
  that could not connect or has no built binary. Run without root or an administrator, it changes
  nothing and prints the command to run with those rights. `trust-dev service uninstall <name>`
  stops and removes the service (specification 48 RU-64).
