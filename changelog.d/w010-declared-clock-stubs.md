### Added

- W010 recognizes a body-less library stub that declares it reads the host clock: the
  `{attribute 'reads_host_clock'}` pragma (specification 07, "Declared Clock Sources") makes the
  stub's call sites warn, and the message names the declared source instead of `CURRENT_DT`. The
  stub's own file stays silent, and the pragma-attribute parser now accepts CODESYS-style flags
  that carry no value (issue #264).
