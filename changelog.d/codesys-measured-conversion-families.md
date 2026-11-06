### Added

- Under the CODESYS conversion profile, the round-2 conversion families of
  DEV-036: `<ANY_INT>_TO_TIME` (a millisecond count), `REAL_TO_TIME` and
  `LREAL_TO_TIME` (milliseconds truncated toward zero, negative counts
  saturating to `T#0ms`), `DWORD_TO_TOD` (the ms-of-day magnitude modulo
  24 h), and string-to-integer/bit-string conversions (typed `STRING_TO_*`
  and overloaded `TO_*`) with the one CODESYS parse rule: leading spaces,
  an optional sign, a decimal or `2#`/`8#`/`16#` literal, trailing
  characters ignored, no number yielding `0`, out of range keeping the
  low-order bits. The values are measured on a live e!COCKPIT runtime and
  recorded in specification 07 section 2.10.
