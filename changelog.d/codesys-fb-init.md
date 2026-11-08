### Added

- Under the CODESYS profiles a cold and a warm restart call `FB_init` of every function block
  instance whose type declares it, before the first cycle, as CODESYS does: `bInitRetains` is TRUE
  at a cold restart and FALSE at a warm one, `bInCopyCode` FALSE, and an instance's base part and
  nested instances come before its own `FB_init` (specification 10 §4.8). Its further inputs take
  the arguments of the instance's declaration, `com1 : serialdevice(iCOMnum := 1)`, by name or in
  order; a string or another input that is no scalar is refused at build for now.
