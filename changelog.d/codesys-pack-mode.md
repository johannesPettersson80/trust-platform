### Added
- `{attribute 'pack_mode' := 'n'}` before a structure's `TYPE` packs its members as CODESYS does: with `0` or `1`
  without gaps, with `2` or `4` at most that far apart, the size rounded up to the largest alignment. `SIZEOF` gives
  that size, a `STRUCT OVERLAP` sees the structure's bytes there, and a located structure lies on the wire there; a
  value other than 0, 1, 2, 4 or 8 is an error (specification 02 §3.5.2).
