### Added

- Under the CODESYS profiles an instance that a function or a method declares, or the `VAR_TEMP` of a program or a
  function block, gets `FB_init(bInitRetains := FALSE, bInCopyCode := FALSE)` each time a call starts it and
  `FB_exit(bInCopyCode := FALSE)` when the call returns, also through `RETURN`; `{attribute 'no-exit'}` leaves a marked
  one out.
