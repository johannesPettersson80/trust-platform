### Fixed

- `trust-runtime completions` no longer panics when its reader stops early, as in
  `trust-runtime completions bash | head`: a closed pipe ends the output without an error.
