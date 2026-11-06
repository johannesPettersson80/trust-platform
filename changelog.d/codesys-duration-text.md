### Fixed

- Under the CODESYS conversion rules, `TO_STRING` and `TIME_TO_STRING` of a `TIME` or an `LTIME` give
  the text the CODESYS runtime gives, measured on a WAGO PFC200: each unit that is not zero, as in
  `T#1s500ms`, `T#2d` and `LTIME#1h2m3s4ms5us6ns`, and `T#0ms` for no time. A compiled program
  refused to build them, and the evaluator wrote `T#1500ms` (specification 07 §2.7).
