### Changed

- W012's message names its remedy: give the global one writing task, or declare it shared by design
  with `{attribute 'cross_task'}`.

### Fixed

- `{attribute 'cross_task'}` before a `NAMESPACE` now covers the global variable lists in it. The
  PLCopen import of a CODESYS global variable list writes the list's attributes there, and W012
  ignored the attribute in that place.
