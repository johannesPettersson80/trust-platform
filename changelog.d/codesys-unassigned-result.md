### Changed

- Under the CODESYS profiles a method or function that never assigns its result returns the
  default value of its result type, as CODESYS builds the methods it scaffolds, where it failed with
  E206 "missing return value" (specification 04). The IEC profile keeps E206.
