### Added

- `trust-sim` reads and writes times and dates: a machine test writes a `TIME`, `DATE`,
  `TIME_OF_DAY` or `DATE_AND_TIME` variable, or its long form, from trust_plc's `Time`, `Date`, `Tod`,
  `Dt` and the others, which trust-sim re-exports, and reads them back the same way (specification 48
  RU-58).
