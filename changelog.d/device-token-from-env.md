### Fixed

- `trust-dev test --device` takes the device's control token from `TRUST_CTL_TOKEN` when `--token`
  is not given, as its help and specification 22 say. It sent no token then, and a device whose
  settings set one refused the run with `missing auth token`.
