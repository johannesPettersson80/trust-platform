### Added

- A Rust POU's variables may be dates and times of day (specification 48 RU-25): `Date`, `Tod` and
  `Dt` and their long forms `LDate`, `LTod` and `Ldt` hold what Structured Text's `DATE`, `TOD` and
  `DT` hold, start at an IEC literal such as `#[init("DT#2024-02-29-12:30:15")]`, and are built,
  split and moved by a duration with the code `CONCAT_DATE`, `SPLIT_DT` and `ADD_DT_TIME` run.
