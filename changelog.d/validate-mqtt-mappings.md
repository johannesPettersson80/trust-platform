### Fixed

- `trust-runtime validate` resolves an MQTT driver's tag mappings against the built program, as the start does, so it
  refuses a mapping whose tag names no variable of a program instance with the start's error. It used to answer
  `Project ok` for a project that `trust-runtime run` then refused to start.
