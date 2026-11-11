### Fixed

- Under a CODESYS profile, the native build now compiles a string `VAR_IN_OUT` of a function or a
  method that is given a string of another capacity, as the language rules already allowed
  (docs/IEC_DEVIATIONS.md DEV-035). The vendor idiom of a `STRING(1)` in-out whose real capacity a
  size argument gives, such as WAGO's `StrAppend(sBuffer, SIZEOF(sBuffer), sText)`, stopped the
  build with "a variable of another type is not compiled yet". The in-out is the caller's string
  seen through the capacity it declares: it reads and writes at most that many characters, and
  never more than the caller's string holds (specification 44 §7.3).
