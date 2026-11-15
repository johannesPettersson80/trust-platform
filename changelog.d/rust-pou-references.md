### Added

- Rust POUs take in-outs and references: a function's `VAR_IN_OUT` is an `InOut<T>` parameter, the
  caller's variable for the call, a function block's is a field `#[in_out]` of type `Ref<T>`, the
  reference of its last call, and a `REF_TO` variable or input is a `Ref<T>`, read and written with
  Structured Text's checks (specification 48 RU-29). An in-out of a structure and a reference to one
  read and write every field where the structure lies, at once. An in-out of a reference, a reference
  to a reference and a function's or a method's string in-out are refused for now, with that reason.
