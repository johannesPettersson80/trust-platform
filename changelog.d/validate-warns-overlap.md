### Added

- `trust-runtime validate` warns of composed I/O drivers that would fill the same bytes of the input image, as the
  runtime warns at its start: one `warning:` line for each overlap before `Project ok`, and the same data under
  `warnings` in its `--ci` report. It reads each driver's bytes from its parameters, so it opens no connection and
  starts no driver, and the project still validates.
