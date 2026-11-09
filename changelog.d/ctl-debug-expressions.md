### Added

- `trust-runtime ctl breakpoints-set` takes `--condition`, `--hit-condition` and `--log-message`,
  `ctl evaluate <expression>` evaluates a debug expression in the held program or in the last scan,
  and `ctl compile-expression` prints an expression's compiled form for scripts. The expressions
  are compiled with the project's types, as the VS Code debugger compiles them.
