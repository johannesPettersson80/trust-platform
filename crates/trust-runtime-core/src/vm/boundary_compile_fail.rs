//! Compile-fail boundaries of the portable owners (spec 34 §4.3.1). Each
//! `compile_fail` block must fail with the code it names. Each control uses the
//! same API in its valid form and must compile, so a failure cannot come from a
//! misspelled path.
//!
//! A staged reconfiguration holds the owner exclusively until it is committed or dropped:
//!
//! ```compile_fail,E0502
//! use std::sync::Arc;
//! use trust_runtime_core::vm::{LogicalExecutionServices, OwnedRuntimeState, PreparedModule};
//! fn overlap(
//!     owner: &mut OwnedRuntimeState<LogicalExecutionServices>,
//!     prepared: Arc<PreparedModule>,
//! ) {
//!     let staged = owner.prepare_resource_reconfiguration(prepared, 0, false).unwrap();
//!     let _ = owner.storage();
//!     let _ = staged.commit();
//! }
//! ```
//!
//! ```
//! use std::sync::Arc;
//! use trust_runtime_core::vm::{LogicalExecutionServices, OwnedRuntimeState, PreparedModule};
//! fn commit_then_inspect(
//!     owner: &mut OwnedRuntimeState<LogicalExecutionServices>,
//!     prepared: Arc<PreparedModule>,
//! ) {
//!     let staged = owner.prepare_resource_reconfiguration(prepared, 0, false).unwrap();
//!     staged.commit().unwrap();
//!     let _ = owner.storage();
//! }
//! ```
//!
//! A state that borrows its admitted module cannot be returned after that module is a local:
//!
//! ```compile_fail,E0515
//! use trust_runtime_core::vm::{PreparationLimits, PreparedModule, RuntimeState};
//! fn escape(bytes: &[u8]) -> RuntimeState<'static> {
//!     let prepared = PreparedModule::from_bytes(bytes, PreparationLimits::default()).unwrap();
//!     prepared.instantiate(0).unwrap()
//! }
//! ```
//!
//! ```
//! use trust_runtime_core::vm::{PreparedModule, RuntimeState};
//! fn borrowed(prepared: &PreparedModule) -> RuntimeState<'_> {
//!     prepared.instantiate(0).unwrap()
//! }
//! ```
//!
//! The validation token has private fields, so a caller cannot forge one:
//!
//! ```compile_fail,E0451
//! use trust_runtime_core::bytecode::{BytecodeModuleView, ValidatedBytecode, ValidationStats};
//! fn forge<'a>(view: BytecodeModuleView<'a>, stats: ValidationStats) -> ValidatedBytecode<'a> {
//!     ValidatedBytecode { view, stats }
//! }
//! ```
//!
//! ```
//! use trust_runtime_core::bytecode::{BytecodeModule, ValidatedBytecode};
//! fn validated(raw: &BytecodeModule) -> Option<ValidatedBytecode<'_>> {
//!     raw.validated().ok()
//! }
//! ```
//!
//! The admitted layout of a prepared module is private to the crate:
//!
//! ```compile_fail,E0616
//! use trust_runtime_core::vm::PreparedModule;
//! fn peek(prepared: &PreparedModule) {
//!     let _ = &prepared.layout;
//! }
//! ```
//!
//! ```
//! use trust_runtime_core::vm::PreparedModule;
//! fn instantiate_ok(prepared: &PreparedModule) {
//!     let _ = prepared.instantiate(0);
//! }
//! ```
//!
//! A staged candidate does not expose the owner it would replace:
//!
//! ```compile_fail,E0616
//! use trust_runtime_core::vm::{LogicalExecutionServices, ResourceReconfiguration};
//! fn peek(staged: &ResourceReconfiguration<'_, LogicalExecutionServices>) {
//!     let _ = &staged.owner;
//! }
//! ```
//!
//! ```
//! use trust_runtime_core::{
//!     error::RuntimeError,
//!     vm::{LogicalExecutionServices, ResourceReconfiguration},
//! };
//! fn publish(
//!     staged: ResourceReconfiguration<'_, LogicalExecutionServices>,
//! ) -> Result<(), RuntimeError> {
//!     staged.commit()
//! }
//! ```
//!
//! Output images are published by the owner's cycle, so no caller can mutate them:
//!
//! ```compile_fail,E0599
//! use trust_runtime_core::vm::{LogicalExecutionServices, OwnedRuntimeState};
//! fn bypass(state: &mut OwnedRuntimeState<LogicalExecutionServices>) {
//!     let _ = state.outputs_mut();
//! }
//! ```
//!
//! ```compile_fail,E0599
//! use trust_runtime_core::vm::RuntimeState;
//! fn bypass(state: &mut RuntimeState<'_>) {
//!     let _ = state.outputs_mut();
//! }
//! ```
//!
//! ```
//! use trust_runtime_core::vm::{LogicalExecutionServices, OwnedRuntimeState};
//! fn sample(state: &mut OwnedRuntimeState<LogicalExecutionServices>) -> &mut [u8] {
//!     state.inputs_mut()
//! }
//! ```
//!
//! ```
//! use trust_runtime_core::vm::RuntimeState;
//! fn sample<'a>(state: &'a mut RuntimeState<'_>) -> &'a mut [u8] {
//!     state.inputs_mut()
//! }
//! ```
#![cfg_attr(
    feature = "hir",
    doc = r#"
Owner storage is reached only through the dispatcher trait, which the owners do not
implement. Importing that trait keeps each bypass denied if an implementation is added later.
The control shows that the import path itself resolves.

```compile_fail,E0599
use trust_runtime_core::vm::{
    hosted::context::ReferenceContext, LogicalExecutionServices, OwnedRuntimeState,
};
fn bypass(state: &mut OwnedRuntimeState<LogicalExecutionServices>) {
    let _ = state.storage_mut();
}
```

```compile_fail,E0599
use trust_runtime_core::vm::{hosted::context::ReferenceContext, RuntimeState};
fn bypass(state: &mut RuntimeState<'_>) {
    let _ = state.storage_mut();
}
```

```compile_fail,E0599
use trust_runtime_core::vm::{hosted::context::ReferenceContext, CycleAccess};
fn bypass(access: &mut CycleAccess<'_, '_>) {
    let _ = access.storage_mut();
}
```

```
use trust_runtime_core::vm::hosted::context::ReferenceContext;
fn names_trait<T: ReferenceContext>(_: &T) {}
```
"#
)]
