### Added

- The Rust SDK's API documentation shows each attribute (`#[trust_fb]`, `#[trust_program]`,
  `#[trust_sequence]`, `#[trust_function]`, `#[trust_method]`, `#[trust_enum]`, `#[trust_struct]`,
  `#[budget]`, `#[bench_case]`) in a whole program with a unit test that calls it. `trust-plc` and
  `trust-runtime-client` gain an example on their front pages. CI runs the examples of every
  published SDK crate as tests, so they keep matching the API (specification 48 RU-89).
