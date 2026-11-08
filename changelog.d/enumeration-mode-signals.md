### Added

- Control Studio tunes a loop whose Auto/Manual signal is an enumeration, as CODESYS and TwinCAT code
  selects a controller's mode (TwinCAT's `E_CTRL_MODE`, a project's own `E_Mode`). The loop names the
  member that means Manual and the one that means Auto (`manual_value`, `auto_value`); its other
  members, such as `Off`, mean neither mode, and an operation that finds the loop in neither does not
  start and names the member it found (specification 35 A6.1).
  - Transfers, Apply, Identify's experiments, Validate's and Tune's isolated runs, the monitor and
    recordings write and read the mode by its members.
  - A setup draft names an enumeration mode by `manualValue` and `autoValue`, for the loop's own mode
    and for a cascade's outer controller. Setup and discovery check both members against the
    compiled enumeration.
