//! Machine tests of the example `fault_trace` (specification 48 RU-58 to RU-60): its program and
//! its simulation run in a simulator of the test's own, on a clock that only the test moves; the
//! simulated stroke sensor breaks at 5 s, the program faults, and the test keeps the flight
//! recorder's trace of the fault and replays it.
//!
//! Run them from this folder with `cargo test`. They need the installed trust-runtime of the same
//! version as `trust-sim`, which `TRUST_RUNTIME` may name.
