### Fixed

- Tutorial 23 (historian and Prometheus metrics) walks through its steps with the commands and the output of a real
  run, from the allowlist in `runtime.toml` to the history file and the `/metrics` endpoint, and sets its input with
  `trust-runtime ctl io-write` instead of leaving that to the web interface. CI runs its steps on tutorial 13's project.
