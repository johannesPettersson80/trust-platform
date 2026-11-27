### Fixed

- Tutorial 13 walks through a project from an empty folder step by step, with the values of a real run: the four
  files, the build, the check, the start command written and read with `trust-runtime ctl`, and the changed rule.
  Its "common mistakes" said that a missing `io.toml` gives confusing I/O and a missing configuration cannot
  schedule the program; the Try-its now show what happens: `validate` refuses a project without `io.toml`, and
  without the configuration the program runs with no inputs or outputs bound. CI checks both.
