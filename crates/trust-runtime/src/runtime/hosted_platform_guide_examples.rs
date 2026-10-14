//! Doc-test harness: the Rust examples in `docs/guides/RUNTIME_EMBEDDING_MIGRATION.md`
//! compile and run as doc tests (spec 34 §4.3 HOST-03). The module declares no items;
//! it exists only so rustdoc collects the guide's `rust` blocks. The examples use
//! `harness::CompileSession` and the full `Runtime`, so it is compiled only under `full`.
#![doc = include_str!("../../../../docs/guides/RUNTIME_EMBEDDING_MIGRATION.md")]
