### Fixed

- The systemd units that the runtime archives carry start the PLC again. They ran `trust-runtime --project …`, which
  the command line refuses, so systemd restarted the service every 2 s and the PLC never ran. They now run
  `trust-runtime run --project /opt/trust/current --restart warm`: the deployed project starts warm, so that its
  retained values come back after a power loss, a crash or a restart by systemd. The deployment notes show `run` too.
