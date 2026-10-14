//! Construction-only mutable accessors stay on `RuntimeBuilder`; the installed `Runtime`
//! exposes checked operations only (spec 34 §4.3.1 "construction versus admitted facades",
//! H acceptance: compile-fail denial of raw mutable installed access).
//!
//! The installed controller must not expose the type registry, the I/O interface or the
//! access map mutably. Each block below must fail to compile.
//!
//! ```compile_fail,E0599
//! fn bypass(runtime: &mut trust_runtime::Runtime) {
//!     let _ = runtime.registry_mut();
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn bypass(runtime: &mut trust_runtime::Runtime) {
//!     let _ = runtime.io_mut();
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn bypass(runtime: &mut trust_runtime::Runtime) {
//!     let _ = runtime.access_map_mut();
//! }
//! ```
//!
//! Installation consumes the construction object, which offers no execution:
//!
//! ```compile_fail,E0382
//! fn install_then_edit(mut declarations: trust_runtime::RuntimeBuilder) {
//!     let _installed = declarations.build();
//!     let _ = declarations.storage_mut();
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn no_execution(mut declarations: trust_runtime::RuntimeBuilder) {
//!     let _ = declarations.execute_cycle();
//! }
//! ```
//!
//! Retained checkpoint capture needs the exclusive owner borrow (spec 34 §4.3.1):
//!
//! ```compile_fail,E0596
//! fn shared_capture(runtime: &trust_runtime::Runtime) {
//!     let _ = runtime.retain_snapshot();
//! }
//! ```
//!
//! Control: the construction object still exposes the same accessors before installation.
//!
//! ```
//! let mut builder = trust_runtime::RuntimeBuilder::new();
//! let _ = builder.registry_mut();
//! let _ = builder.io_mut();
//! let _ = builder.access_map_mut();
//! ```
