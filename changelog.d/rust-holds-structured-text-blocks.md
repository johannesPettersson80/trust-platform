### Added

- A Rust function block or program holds a Structured Text function block of the project as a
  field, `hysteresis: st::HYSTERESIS`, and calls it: `self.hysteresis.input.IN = level;
  self.hysteresis.call(cx)?; let q = self.hysteresis.output.Q;` (specification 48 RU-38, RU-39).
  The held instance lies inside the Rust instance in the area, where Live Values, the debugger and
  online change see it. Blocks whose inputs and outputs are elementary types, strings,
  arrays of any dimension, references or the project's structures, enumerations and types with named values are
  held, each as the Rust type of RU-25's table or its binding; a held block's in-outs are `Ref<T>`
  fields of its `input`, and its `PUBLIC` methods are methods of its binding, `self.meter.ADD(cx, x)`.
