### Added

- Under the CODESYS profiles a cold and a warm restart call `FB_init` of every function block
  instance whose type declares it, before the first cycle, as CODESYS does: `bInitRetains` is TRUE
  at a cold restart and FALSE at a warm one, `bInCopyCode` FALSE, and an instance's base part and
  nested instances come before its own `FB_init` (specification 10 §4.8). An `FB_init` with inputs
  of its own, which CODESYS gives at the instance's declaration, is refused at build for now.
