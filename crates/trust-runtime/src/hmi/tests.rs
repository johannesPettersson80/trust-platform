//! The tests that run a program build it: they need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

include!("tests/scaffold.rs");
include!("tests/schema.rs");
include!("tests/live_state.rs");
