### Fixed

- `ADD` with the duration first, such as `ADD(T#1H, startTime)` for a `TIME_OF_DAY`, is now reported
  as an invalid argument where it is written. IEC 61131-3 Table 35 lists `ADD` with the time of day or
  date and time first, as the `+` operator already required. Before, the editor accepted it and the
  build then failed with a message that called the addition a comparison.
