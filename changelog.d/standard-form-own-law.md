### Added

- Control Studio tunes a program's own PI or PID that holds its settings in the standard form, as
  CODESYS's `PID` (`KP`, `TN`, `TV`) and blocks like a project's `FB_PI` (`Kp`, `Ti`) do: the gain
  `Kp`, the reset time `Ti` and the derivative time `Td` in seconds (specification 35 C2, numerics
  annex N6.8). Tune designs the law from the model as for the parallel form and writes
  `Ti = Kp/Ki` and `Td = Kd/Kp`; such a block no longer needs the finite search's trial grid.
  - Setup offers the form beside the law (`form`, `resetTime`, `derivativeTime` in a setup draft);
    a law in standard form is PI or PID, and its reset time's range lies above zero.
  - A pinned or entered reset or derivative time is a time in seconds; the transfers, Apply and
    Validate read the times back as gains.
