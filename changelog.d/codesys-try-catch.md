### Added

- CODESYS's exception handling, `__TRY`, `__CATCH(variable)`, `__FINALLY` and `__ENDTRY`, with the system type
  `__SYSTEM.ExceptionCode`, under every profile. A division or `MOD` by zero, an overflow, an index out of bounds, a
  `NULL` dereference, the call depth and a few others raised inside `__TRY` are handled with CODESYS's codes; the
  statement budget, the task stack's margin and a test's failed assertion still end the scan (specification 06 §15).
