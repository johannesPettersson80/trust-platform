### Changed

- Interface and abstract-method conformance diagnostics move from `E202` to
  the new dedicated code `E208` ("must implement method/property",
  signature and visibility mismatches, missing abstract implementations).
  Type-used-as-value, access-specifier and initializer-constraint
  diagnostics keep `E202`, so per-code severity overrides no longer mask
  interface completeness regressions.

### Fixed

- Declaration sizes over flattened library containers: under the CODESYS
  profile, a dotted size reference (`STRING(LogParameter.MAX_LOG_MSG_LEN)`,
  `STRING(WagoTypesLog.LogParameter.MAX_LOG_MSG_LEN)`) resolves when the
  named GVL level was flattened into namespace-level constants — the
  reference's last segment must match exactly one constant or
  initializer-backed declared value table-wide, in the string-capacity and
  array-bound contexts only. Ambiguous leaves and references rooted at a
  scope-local value stay rejected; bare names never take the tolerance, and
  ordinary initializers keep the strict rule (IEC_DEVIATIONS 2026-10-25).
  The analyzer and the runtime harness fold the same value, so
  documentation-faithful sizes (128/20/40-class stub constants) compile at
  their declared capacities instead of failing the compile. PLCopen
  import/export passes symbolic string lengths through verbatim (mirroring
  array dimensions) instead of failing POUs closed at import.
