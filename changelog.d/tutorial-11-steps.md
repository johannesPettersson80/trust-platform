### Fixed

- Tutorial 11 walks through testing control logic without hardware step by step, with the values of a real run: the
  tank's logic in a function block, the program as wiring and the addresses in the configuration, the tests that
  drive the block with the inputs a plant would give, a failure traced to its branch, and the same block run in the
  runtime with its inputs written by `trust-runtime ctl`. It ran `trust-dev` through `cargo run -p trust-dev` from a
  truST checkout; it now runs the tools of the VS Code extension or a release archive. Its web interface moved from
  port 8080, often taken on a developer's computer, to 18081. The Try-its test the band's edges, which the shipped
  tests miss, and an endless test, which the runner's work limit ends as an error. CI checks the steps and the
  Try-its.
