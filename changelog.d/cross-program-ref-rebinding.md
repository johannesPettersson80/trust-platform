### Added

- Compiled-program tests pin the `REF=` rebinding across programs under
  the CODESYS profile: a reference binds to another program's variable
  (through members and indexed function-block array elements included),
  the binding is live, and a global array element's reference-typed
  member takes the rebinding; the IEC rules refuse the program-qualified
  source (specification 06).
