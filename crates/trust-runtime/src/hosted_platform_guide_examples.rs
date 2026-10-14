//! Doc-test harness: the Rust examples in `docs/guides/RUNTIME_EMBEDDING_MIGRATION.md`
//! compile and run as doc tests (spec 34 §4.3 HOST-03). The module declares no items;
//! it exists only so rustdoc collects the guide's `rust` blocks.
//!
//! Lead wiring (not made here): in `crates/trust-runtime/src/lib.rs`, add
//! `#[cfg(feature = "full")] mod hosted_platform_guide_examples;` next to the other
//! `full`-only modules. The examples use `harness::CompileSession` and the full
//! `Runtime`, so they are compiled only under `full`.
#![doc = include_str!("../../../docs/guides/RUNTIME_EMBEDDING_MIGRATION.md")]
