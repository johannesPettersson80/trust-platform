### Fixed

- A program that names an enumeration value in CODESYS's dotted form, `E_Mode.MANUAL`, builds and
  runs: the build refused it with "the name E_Mode is not compiled yet" though the editor accepted
  it, with or without `{attribute 'qualified_only'}`. It works in initializers, comparisons, CASE
  labels, call arguments and assignments, under every profile.
