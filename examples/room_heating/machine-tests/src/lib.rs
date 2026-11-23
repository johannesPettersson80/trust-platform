//! Machine tests of the example `room_heating` (specification 48 RU-58): its program and its
//! simulated plant run in a simulator of each test's own, on a clock that only the test moves, and
//! the tests in `tests/` press the heating's Start and Stop as the operator does and check what the
//! plant then does.
//!
//! Run them from this folder with `cargo test`. They need the installed trust-runtime of the same
//! version as `trust-sim`, which `TRUST_RUNTIME` may name.
