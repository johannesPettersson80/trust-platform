### Changed

- `trust-runtime check` and `trust-runtime build --json` report a construct that the build does not
  compile yet at its place in the sources: the issue `compile` now has the file, line and column of
  the construct when the build knows them, as the parse and resolve diagnostics have.
