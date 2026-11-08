### Added

- `THIS^` and `SUPER^`, CODESYS's spelling of `THIS` and `SUPER` as pointers, are accepted under
  every profile and work as `THIS` and `SUPER`: `THIS^.Speed`, `SUPER^.Initialize()` and, in a
  body, `SUPER^()` (specification 04). They failed with E201 "dereference requires pointer type".
