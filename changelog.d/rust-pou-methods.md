### Added

- Rust function blocks have methods: a `pub fn` of the block's `impl` block with `#[trust_method]`
  and its budget, taking `&mut self` and the context, then its inputs, its in-outs as `InOut<T>` and
  its outputs as `&mut T`, and returning its result; Structured Text calls it as a method of the
  block, which runs on the instance as the body does, and a task counts its budget. The generated
  declaration carries each method with its access specifier (`#[trust_method(private)]`,
  `(protected)`, `(internal)`; public otherwise) (specification 48 RU-32).
