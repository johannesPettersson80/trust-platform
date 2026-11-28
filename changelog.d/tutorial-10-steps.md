### Fixed

- Tutorial 10 walks through Structured Text unit tests step by step, with the values of a real run: the tests run,
  listed and filtered, a bug that a test finds, and the three kinds of result. It ran `trust-dev` through
  `cargo run -p trust-dev` from a truST checkout, which needs Rust and the repository's build; it now runs the
  `trust-dev` of the VS Code extension or a release archive. Its Try-its show a filter that matches nothing, a
  runtime error that ends a test as an error, and a bug that the shipped tests miss until a test gives both of the
  latch's commands at once. CI checks the steps and the Try-its.
