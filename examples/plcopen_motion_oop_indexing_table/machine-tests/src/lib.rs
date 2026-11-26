//! Machine tests of the example `plcopen_motion_oop_indexing_table` (specification 48 RU-58): its
//! program and the PLCopen Motion OOP library's simulated axis run in a simulator of each test's own,
//! on a clock that only the test moves, and the tests in `tests/` check that the table stops at each
//! station in turn.
//!
//! Run them from this folder with `cargo test`. They need the installed trust-runtime of the same
//! version as `trust-sim`, which `TRUST_RUNTIME` may name.
