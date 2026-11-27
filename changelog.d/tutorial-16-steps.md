### Fixed

- Tutorial 16 walks through opening a runtime's web interface to the network step by step, with the values of a real
  run: the request refused without the token, the status with it and its Admin role, plain HTTP getting nothing, and
  a paired device's own Operator token. Its Try-its show revoking the paired device with `pair.revoke`, and the
  refusals of `auth = "token"` without a token and of a control endpoint on a network address. CI checks the
  configuration and both refusals against the README.
