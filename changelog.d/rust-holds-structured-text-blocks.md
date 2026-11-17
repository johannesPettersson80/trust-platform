### Added

- A Rust function block or program holds a Structured Text function block of the project as a
  field, `hysteresis: st::HYSTERESIS`, and calls it: `self.hysteresis.input.IN = level;
  self.hysteresis.call(cx)?; let q = self.hysteresis.output.Q;` (specification 48 RU-38, RU-39).
  The held instance lies inside the Rust instance in the area, where Live Values, the debugger and
  online change see it. Blocks whose inputs and outputs are elementary types, strings,
  arrays of any dimension, references or the project's structures, enumerations, types with named values and
  interfaces are held, each as the Rust type of RU-25's table or its binding; a held block's in-outs are `Ref<T>`
  fields of its `input`, and its `PUBLIC` methods are methods of its binding, `self.meter.ADD(cx, x)`. A class instance is
  held the same way, its binding with the class's `PUBLIC` methods and no `call`.
- A Rust POU names a Structured Text function block or class instance through a reference,
  `#[input] pub target: Ref<st::COUNTER>`, which a Structured Text caller gives `REF(counter)`:
  `self.target.get(cx)?` gives the block's binding naming that instance, whose `call` and methods
  run the body and the methods of the instance's own class, a derived block's included. `get` of
  `NULL` ends the scan, and such a reference has no `set`, as an instance is no value. Calls and
  held blocks pass such references too (specification 48 RU-39).
