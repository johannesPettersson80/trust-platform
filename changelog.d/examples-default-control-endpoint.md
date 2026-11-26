### Fixed

- Ten examples start on Windows: the five PLCopen Motion OOP examples, the single-axis motion demo, tutorials 9 and 12,
  the MQTT traffic light and the ADS server example no longer pin a Unix control socket, which a Windows runtime
  refuses. Each takes its platform's default endpoint, which `trust-runtime ctl --project` finds. The ADS example's
  pyads smoke script reaches the runtime by its project folder with `--doctor-project`.
