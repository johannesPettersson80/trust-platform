### Added

- A method may declare CODESYS's `VAR_INST` section: variables kept per instance and method between
  calls, as truST's method `VAR_STAT` is, with the same qualifiers (specification 04). It is
  refused anywhere but in a method, as in CODESYS; it failed with a parse error.
