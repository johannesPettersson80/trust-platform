### Fixed

- `AND`, `OR`, `XOR` and `MOD` can be called by their names, `A := AND(B, C, D)` and
  `MOD(A, B)`, as IEC 61131-3 Table 31 and Table 29 write them; such a call was refused as
  "expected expression". An untyped literal among the inputs takes the type of the typed ones
  (specification 07 §5).
