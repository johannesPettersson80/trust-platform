### Added

- `__ISVALIDREF(ref)`, CODESYS's test whether a reference is bound, under every profile: `TRUE`
  when the `REF_TO` or `REFERENCE TO` value is bound, `FALSE` when it is `NULL` (specification 02
  §4.1). It failed with "expected expression".
