### Added

- A Rust POU's variables may be strings (specification 48 RU-25): `Str<n>` and `WStr<n>` hold what
  Structured Text's `STRING[n]` and `WSTRING[n]` hold, start at a text such as
  `#[init("pump 1")]`, and keep the longest prefix of whole characters their capacity holds, as
  every write of Structured Text does; a Rust function takes and returns them too.
