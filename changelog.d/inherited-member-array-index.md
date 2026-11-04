### Fixed

- An inherited function-block member used as an array subscript no longer reports E101
  "undefined constant". The index-bounds check folds only compile-time constants; for a
  name it cannot fold it asked the lexical scope whether the index is a computed value,
  and a member declared in a base function block (`fbChild EXTENDS fbBase`, indexed by a
  `VAR_INPUT` of the base) resolved in neither, so a statement like
  `gvlLog[uiEntityIdx] := 1` on the derived block reported an undefined constant while
  the identical statement with a local member stayed silent. The guard now also asks the
  enclosing POU's EXTENDS chain — under both dialects, since function-block inheritance
  is IEC 61131-3 Ed.3 §6.6.3.4 — and a genuinely unknown name keeps E101
  (specification 09 §3.8).
