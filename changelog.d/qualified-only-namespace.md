### Fixed

- Under the CODESYS profiles, `{attribute 'qualified_only'}` before a `NAMESPACE` now applies to the
  global variable lists in it. The PLCopen import of a CODESYS global variable list writes the
  attribute there, and it was ignored. A bare name that reaches one of those globals through
  `USING` from outside the namespace is now refused (E105) and names the qualified form, as
  CODESYS refuses it.
