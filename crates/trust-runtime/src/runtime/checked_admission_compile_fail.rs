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
//! Installed borrows end before the next exclusive operation. A shared borrow from
//! `storage()` or `io()` cannot survive a write or a cycle:
//!
//! ```compile_fail,E0502
//! use trust_runtime::{value::Value, Runtime};
//! fn escape(runtime: &mut Runtime) {
//!     let storage = runtime.storage();
//!     let _ = runtime.write_global("probe", Value::Int(1));
//!     let _ = storage.get_global("probe");
//! }
//! ```
//!
//! ```compile_fail,E0502
//! use trust_runtime::Runtime;
//! fn escape(runtime: &mut Runtime) {
//!     let io = runtime.io();
//!     let _ = runtime.execute_cycle();
//!     let _ = io.inputs();
//! }
//! ```
//!
//! ```
//! use trust_runtime::{value::Value, Runtime};
//! fn sequenced(runtime: &mut Runtime) {
//!     let _ = runtime.write_global("probe", Value::Int(1));
//!     let _ = runtime.storage().get_global("probe");
//! }
//! ```
//!
//! ```
//! use trust_runtime::Runtime;
//! fn sequenced(runtime: &mut Runtime) {
//!     let _ = runtime.io().inputs().len();
//!     let _ = runtime.execute_cycle();
//! }
//! ```
//!
//! The storage dispatcher trait is internal. The installed controller does not implement
//! it, and naming it from outside the crate fails. Importing the trait keeps the
//! `storage_mut` bypass denied if an implementation is added later.
//!
//! ```compile_fail,E0277
//! use trust_runtime::{runtime_core::vm::hosted::context::ReferenceContext, Runtime};
//! fn bypass_admission(runtime: &mut Runtime) {
//!     let _ = ReferenceContext::storage_mut(runtime);
//! }
//! ```
//!
//! ```compile_fail,E0599
//! use trust_runtime::{runtime_core::vm::hosted::context::ReferenceContext, Runtime};
//! fn bypass(runtime: &mut Runtime) {
//!     let _ = runtime.storage_mut();
//! }
//! ```
//!
//! ```
//! use trust_runtime::runtime_core::vm::hosted::context::ReferenceContext;
//! fn names_trait<T: ReferenceContext>(_: &T) {}
//! ```
//!
//! The construction module of the portable core is private to that core:
//!
//! ```compile_fail,E0603
//! use trust_runtime::runtime_core::vm::construction;
//! ```
//!
//! ```
//! use trust_runtime::runtime_core::vm::PreparedModule;
//! fn names(_: &PreparedModule) {}
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
