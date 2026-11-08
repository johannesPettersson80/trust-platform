### Added

- A property of a function block or class whose GET only returns one of its variables (`Kp := _rKp;`) can be read by
  its name, as CODESYS monitors it: `eval`, Live Values and the debugger read `TankInst.fbPI.Kp` as `_rKp`, and `set`,
  `var.force` and the debugger write it when the property's SET only assigns that variable (specification 44 §5.8;
  specification 11). A property whose GET runs more code is still an unknown identifier.
