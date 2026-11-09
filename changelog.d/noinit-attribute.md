### Added

- `{attribute 'noinit'}` (also `no_init` and `no-init`) on the line above a variable's declaration
  keeps the variable's value at a cold and a warm restart, as CODESYS keeps it at a reset. The
  variable takes its starting value when the program is installed or after a power-on, as every
  variable does. It applies to globals, program variables, and a function block's or class's
  variables, inputs and outputs.

### Changed

- The program image format is version 4: a symbol carries the `noinit` mark. A runtime or firmware
  of an earlier version refuses an image built by this version, and this version refuses an older
  image, so a project is built again with this version before it is installed.
