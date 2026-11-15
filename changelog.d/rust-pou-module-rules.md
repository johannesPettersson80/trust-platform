### Added

- The build of a project with Rust POUs refuses a `static` that changes, a recursion through Rust
  code and a call of the target's inexact math functions (`sin`, `exp` and the rest), naming the
  `static` or the functions (specification 48 RU-13, RU-17, RU-18, RU-45).
