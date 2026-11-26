### Added

- The example `rust_first_cell`: a press cell whose whole program is Rust. `trust.toml` is its configuration,
  a sequence clamps, presses, holds, lifts and unclamps with a time limit on each move, and machine tests in
  the crate's own `tests/` run the program and its simulated clamp and press under `cargo test`
  (specification 48 RU-88).
