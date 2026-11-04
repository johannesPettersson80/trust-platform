### Fixed

- PLCopen import and export say that trust-runtime was not found only when Node cannot start it at all (`ENOENT`).
  A launch failure whose text only said "not found", such as a trust-runtime the user may not run, was reported as a
  missing trust-runtime instead of with its own text (specification 25).
