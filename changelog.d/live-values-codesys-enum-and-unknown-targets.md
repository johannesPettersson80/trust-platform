### Fixed

- `set` and `var.force` take an enumeration's value in CODESYS's form, `E_TankMode.AUTO`, as well as
  `E_TankMode#AUTO`; the type may carry its namespaces (`Lib.E_TankMode.AUTO`).

### Changed

- `set`, `var.force` and `retain:` writes to a variable the running program does not have are
  refused at once, naming it: `PRG_Tank.fbPI.Kp names no member`. Before, an integer was answered
  `queued` and then dropped at the scan boundary, and any other value was refused as an unsupported
  value, which blamed the value instead of the target.
