### Added

- Rust POUs have `EN` and `ENO` as every POU does: a call with `EN := FALSE` skips the Rust body, and
  the body sets `ENO` with `cx.set_eno(false)`, which the caller reads with `ENO =>`; each Rust POU's
  generated declaration declares the input `EN : BOOL := TRUE` first and the output `ENO : BOOL`
  last (specification 48 RU-30).
