use alloc::{vec, vec::Vec};
use smol_str::SmolStr;

use super::bindings::{
    bind_builtin_function_block_arguments, VmNativeArg, VmNativeArgValue, VmWriteTarget,
};
use super::context::*;
use crate::error::RuntimeError;
use crate::memory::{InstanceId, MemoryLocation, VariableStorage};
use crate::stdlib::StandardLibrary;
use crate::value::{
    write_value_path, DateTimeProfile, DateTimeValue, Duration, RefSegment, Value, ValueRef,
};
use crate::vm::{context::ReferenceContext, VmFrame, VmTrap};

struct TestContext {
    storage: VariableStorage,
    stdlib: StandardLibrary,
    profile: DateTimeProfile,
    reject_writes: bool,
    reject_reads: bool,
    reject_copies: bool,
}

impl TestContext {
    fn new() -> Self {
        Self {
            storage: VariableStorage::new(),
            stdlib: StandardLibrary::new(),
            profile: DateTimeProfile::default(),
            reject_writes: false,
            reject_reads: false,
            reject_copies: false,
        }
    }

    fn frame(&mut self, value: i32) -> VmFrame {
        VmFrame {
            parameter_values_present: Vec::new(),
            activation: Some(self.storage.reserve_execution_frame().unwrap()),
            pou_id: Some(1),
            return_pc: 0,
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 1,
            locals: vec![Value::DInt(value)],
            runtime_instance: None,
            instance_owner: None,
        }
    }
}

impl ReferenceContext for TestContext {
    fn before_value_clone(&self, _: &Value) -> Result<(), RuntimeError> {
        if self.reject_copies {
            Err(VmTrap::BudgetExceeded.into_runtime_error())
        } else {
            Ok(())
        }
    }
    fn check_reference_write(
        &self,
        reference: crate::value::ValueRefView<'_>,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        self.check_native_write(None, &reference.to_owned(), value)
    }
    fn initializer_reference<'a>(
        &self,
        _: u32,
        _: &'a [RefSegment],
    ) -> Result<crate::value::ValueRefView<'a>, VmTrap> {
        Err(VmTrap::Runtime(RuntimeError::NullReference))
    }
    fn check_reference_read(&self, _: crate::value::ValueRefView<'_>) -> Result<(), RuntimeError> {
        if self.reject_reads {
            Err(RuntimeError::Overflow)
        } else {
            Ok(())
        }
    }
    fn storage(&self) -> &VariableStorage {
        &self.storage
    }
    fn storage_mut(&mut self) -> &mut VariableStorage {
        &mut self.storage
    }
    fn write_typed_storage(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> Result<bool, crate::error::RuntimeError> {
        Ok(self
            .storage
            .write_by_ref_parts(location, offset, path, value))
    }
    fn write_typed_local(&self, root: &mut Value, path: &[RefSegment], value: Value) -> bool {
        write_value_path(root, path, value)
    }
}

impl CallContext for TestContext {
    fn profile(&self) -> &DateTimeProfile {
        &self.profile
    }
    fn current_time(&self) -> Duration {
        Duration::ZERO
    }
    fn current_dt(&mut self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::Overflow)
    }
    fn stdlib(&self) -> &StandardLibrary {
        &self.stdlib
    }
    fn builtin_params(&self, _: &str) -> Option<Vec<BuiltinParam>> {
        Some(vec![BuiltinParam {
            name: SmolStr::new("IN"),
            direction: BuiltinParamDirection::In,
        }])
    }
    fn edge_inputs(&self, _: &str) -> Vec<VmEdgeInput> {
        Vec::new()
    }
    fn suspend_frame(&mut self, frame: &mut VmFrame) -> Result<(), RuntimeError> {
        self.storage
            .suspend_execution_frame(frame.activation.unwrap(), &mut frame.locals)
    }
    fn resume_frame(&mut self, frame: &mut VmFrame) -> Result<(), RuntimeError> {
        self.storage
            .resume_execution_frame(frame.activation.unwrap(), &mut frame.locals)
    }
    fn record_call_op(&mut self, _: RegisterCallOpKind) {}
    fn record_value_op(&mut self, _: RegisterValueOpKind) {}
    fn check_native_write(
        &self,
        _: Option<&VmFrame>,
        _: &ValueRef,
        _: &Value,
    ) -> Result<(), RuntimeError> {
        if self.reject_writes {
            Err(RuntimeError::Overflow)
        } else {
            Ok(())
        }
    }
    fn check_builtin_call(&self, _: InstanceId) -> Result<(), RuntimeError> {
        Ok(())
    }
}

fn reference(frame: &VmFrame) -> ValueRef {
    ValueRef {
        location: MemoryLocation::Local(frame.activation.unwrap()),
        offset: 0,
        path: Vec::new(),
    }
}

#[test]
fn active_local_copyback_checks_admission_before_mutation() {
    let mut context = TestContext::new();
    let mut frame = context.frame(3);
    let target = VmWriteTarget::from_reference(&reference(&frame));
    context.reject_writes = true;
    assert!(matches!(
        target.write(&mut context, &mut frame, Value::DInt(9)),
        Err(VmTrap::Runtime(RuntimeError::Overflow))
    ));
    assert_eq!(frame.locals, vec![Value::DInt(3)]);
    context.reject_writes = false;
    target
        .write(&mut context, &mut frame, Value::DInt(9))
        .unwrap();
    assert_eq!(target.read(&mut context, &frame).unwrap(), Value::DInt(9));
}

#[test]
fn ancestor_copyback_uses_parked_activation_and_rejects_released_frame() {
    let mut context = TestContext::new();
    let mut caller = context.frame(7);
    let target = VmWriteTarget::from_reference(&reference(&caller));
    context.suspend_frame(&mut caller).unwrap();
    assert!(caller.locals.is_empty());
    let mut callee = context.frame(29);
    assert_eq!(target.read(&mut context, &callee).unwrap(), Value::DInt(7));
    target
        .write(&mut context, &mut callee, Value::DInt(11))
        .unwrap();
    assert_eq!(callee.locals, vec![Value::DInt(29)]);
    context.resume_frame(&mut caller).unwrap();
    assert_eq!(caller.locals, vec![Value::DInt(11)]);
    assert!(context
        .storage
        .release_execution_frame(caller.activation.unwrap()));
    assert!(matches!(
        target.read(&mut context, &callee),
        Err(VmTrap::Runtime(RuntimeError::NullReference))
    ));
}

#[test]
fn builtin_input_binding_checks_admission_before_instance_write() {
    let mut context = TestContext::new();
    let caller = context.frame(0);
    let instance = context.storage.create_instance("TON");
    context
        .storage
        .set_instance_var(instance, "IN", Value::Bool(false));
    context.reject_writes = true;
    let args = [VmNativeArg {
        name: None,
        value: VmNativeArgValue::Expr(Value::Bool(true)),
    }];
    assert!(matches!(
        bind_builtin_function_block_arguments(
            &mut context,
            &caller,
            &"TON".into(),
            "TON",
            instance,
            &args
        ),
        Err(VmTrap::Runtime(RuntimeError::Overflow))
    ));
    assert_eq!(
        context.storage.get_instance_var(instance, "IN"),
        Some(&Value::Bool(false))
    );
}

#[test]
fn native_local_and_omitted_builtin_reads_obey_visibility_gate() {
    let mut context = TestContext::new();
    let caller = context.frame(7);
    let target = VmWriteTarget::from_reference(&reference(&caller));
    let instance = context.storage.create_instance("TON");
    context
        .storage
        .set_instance_var(instance, "IN", Value::Bool(false));
    context.reject_reads = true;
    assert!(matches!(
        target.read(&mut context, &caller),
        Err(VmTrap::Runtime(RuntimeError::Overflow))
    ));
    assert!(matches!(
        bind_builtin_function_block_arguments(
            &mut context,
            &caller,
            &"TON".into(),
            "TON",
            instance,
            &[]
        ),
        Err(VmTrap::Runtime(RuntimeError::Overflow))
    ));
    assert_eq!(
        context.storage.get_instance_var(instance, "IN"),
        Some(&Value::Bool(false))
    );
}

#[test]
fn exhausted_copy_budget_prevents_native_input_mutation() {
    let mut context = TestContext::new();
    let caller = context.frame(7);
    let instance = context.storage.create_instance("TON");
    context
        .storage
        .set_instance_var(instance, "IN", Value::Bool(false));
    context.reject_copies = true;
    let args = [VmNativeArg {
        name: None,
        value: VmNativeArgValue::Expr(Value::Bool(true)),
    }];
    let error = bind_builtin_function_block_arguments(
        &mut context,
        &caller,
        &"TON".into(),
        "TON",
        instance,
        &args,
    )
    .unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        VmTrap::BudgetExceeded.into_runtime_error()
    );
    assert_eq!(
        context.storage.get_instance_var(instance, "IN"),
        Some(&Value::Bool(false))
    );
    assert!(VmWriteTarget::from_reference(&reference(&caller))
        .read(&mut context, &caller)
        .is_err());
    assert_eq!(caller.locals, vec![Value::DInt(7)]);
}

#[test]
fn string_element_reference_materialization_preserves_char_and_copy_budget() {
    let mut context = TestContext::new();
    let mut caller = context.frame(0);
    caller.locals[0] = Value::WString("åβ".into());
    let mut selected = reference(&caller);
    selected.path.push(crate::value::single_ref_index(2));
    let target = VmWriteTarget::from_reference(&selected);
    assert_eq!(
        target.read(&mut context, &caller).unwrap(),
        Value::WChar('β' as u16)
    );
    context.reject_copies = true;
    let error = target.read(&mut context, &caller).unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        VmTrap::BudgetExceeded.into_runtime_error()
    );
    assert_eq!(caller.locals[0], Value::WString("åβ".into()));
}

#[test]
fn dynamic_reference_read_charges_before_materializing_owned_value() {
    let mut context = TestContext::new();
    let mut caller = context.frame(0);
    caller.locals[0] = Value::Array(alloc::boxed::Box::new(
        crate::value::ArrayValue::from_canonical_parts(vec![Value::DInt(9); 8], vec![(1, 8)]),
    ));
    let selected = reference(&caller);
    let mut frames = crate::vm::frames::FrameStack::default();
    frames.push(caller).unwrap();
    context.reject_copies = true;
    let error =
        crate::vm::dispatch_refs::dynamic_load_ref(&context, &frames, &selected).unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        VmTrap::BudgetExceeded.into_runtime_error()
    );
    assert!(
        matches!(&frames.current().unwrap().locals[0],Value::Array(array) if array.elements().len()==8)
    );
}
