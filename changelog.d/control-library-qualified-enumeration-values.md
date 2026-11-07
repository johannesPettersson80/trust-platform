### Fixed

- A project that declares its own enumeration with a value named like one of the Control library's, such as a
  CODESYS or TwinCAT `E_Mode` with `Manual` and `Auto`, compiles with the library. The library compared its
  modes, anti-windup and filter order by unqualified values (`Applied.Mode = Manual`), which IEC 61131-3 (6.4.4.2.1)
  makes ambiguous once another type names the same value; it now names each value with its type
  (`PIDMode#Manual`) in every project's copy (specification 45, 1.5).
