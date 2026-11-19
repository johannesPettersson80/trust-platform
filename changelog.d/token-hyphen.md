### Fixed

- Every `--token` takes a token that begins with `-`, as one pairing or service token in 64 does:
  `trust-runtime ctl`, `ui`, `discover`, `identify`, `ads server` and `registry`, and `trust-dev
  test`, read such a token as an option and refused the command line (specification 11 §6.9.6).
