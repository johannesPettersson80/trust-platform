### Fixed

- Under the CODESYS profiles, a type conversion of constant arguments, such as `ANY_TO_BYTE(255)`,
  is accepted in the initial values of a program's, a function block's or a function's `VAR` and of
  a global variable list, scalar or inside a structure's initializer, as it is in a structure's
  member defaults since 0.35.0. The check refused it with E202 "variable initializer must be a
  literal or constant expression". A conversion of a mutable variable is still refused.
