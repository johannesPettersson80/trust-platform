### Changed

- An unused global constant now warns (W001) wherever it is declared, also at the top level of a file. Before, only
  one declared in a NAMESPACE, a CONFIGURATION or a RESOURCE warned (specification 28 §1, decided by the owner on
  1 November 2026, as CODESYS's static analysis reports a project's unused global variables).

### Fixed

- A global constant that a program uses through its `VAR_EXTERNAL` declaration is no longer reported as unused (W001).
