### Fixed

- `trust-dev test --device` reaches a device whose settings set a control token: every request of
  its link carries the token, from `--token`, else `TRUST_CTL_TOKEN`, else the project's
  `runtime.control.auth_token`. Before, the device refused the first request with `missing auth
  token`. `trust-runtime conformance` on a device takes the token from `TRUST_CTL_TOKEN`.
