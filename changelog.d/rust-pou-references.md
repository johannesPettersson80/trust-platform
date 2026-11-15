### Added

- Rust POUs take in-outs and references: a function's `VAR_IN_OUT` is an `InOut<T>` parameter, the
  caller's variable for the call, a function block's is a field `#[in_out]` of type `Ref<T>`, the
  reference of its last call, and a `REF_TO` variable or input is a `Ref<T>`, read and written with
  Structured Text's checks (specification 48 RU-29). An in-out of a structure and a reference to one
  read and write every field where the structure lies, at once, and a function's or a method's
  string in-out is the caller's string, of any capacity under the CODESYS profile, read and written
  as an assignment cuts it.
  An in-out of a reference and a reference to a reference are refused for now, with that reason.
