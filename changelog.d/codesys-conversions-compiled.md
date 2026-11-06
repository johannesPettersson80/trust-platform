### Fixed

- Under a CODESYS `vendor_profile`, `TO_STRING` of an enumerated value gives the value's name in a running
  program, as specification 07 says; it gave the number (`'1'` for `Color#Green`).
- Under a CODESYS `vendor_profile`, `TIME_TO_<integer>`, `LTIME_TO_<integer>`, `TOD_TO_DWORD`, `LTOD_TO_DWORD` and
  `TO_STRING` of a `BOOL` build and run; the build refused them as not compiled yet. A duration's count that does
  not fit its integer keeps the low-order bits (`TIME_TO_SINT(T#300ms)` is `44`), as specification 07 says, where
  the runtime's evaluator stopped with an overflow.
