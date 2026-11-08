### Added

- `__QUERYINTERFACE(source, target)`, CODESYS's query whether the instance an interface names
  implements another interface, under every profile: `TRUE` with the target naming the instance,
  or `FALSE` with the target `NULL` (specification 04). Every interface may be queried. It failed
  with "expected expression".
