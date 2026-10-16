use alloc::{vec, vec::Vec};
use core::cell::Cell;
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
    edge_owner: Option<SmolStr>,
    edge_inputs: Vec<VmEdgeInput>,
    reject_writes: bool,
    reject_reads: bool,
    reject_copies: bool,
    owned_entry_error: bool,
    initialized_frames: usize,
    initialized_presence: Vec<bool>,
    reject_initialization: bool,
    reject_local_transfer: bool,
    retired_frames: usize,
    owned_entry_calls: usize,
    clone_admissions: Cell<usize>,
    recycled_locals: Vec<Value>,
    execution_budget: crate::vm::budget::ExecutionBudget,
}

impl TestContext {
    fn new() -> Self {
        Self {
            storage: VariableStorage::new(),
            stdlib: StandardLibrary::new(),
            profile: DateTimeProfile::default(),
            edge_owner: None,
            edge_inputs: Vec::new(),
            reject_writes: false,
            reject_reads: false,
            reject_copies: false,
            owned_entry_error: false,
            initialized_frames: 0,
            initialized_presence: Vec::new(),
            reject_initialization: false,
            reject_local_transfer: false,
            retired_frames: 0,
            owned_entry_calls: 0,
            clone_admissions: Cell::new(0),
            recycled_locals: Vec::new(),
            execution_budget: crate::vm::budget::ExecutionBudget::new(4096),
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
    fn charge_text_work(&self, units: usize) -> Result<(), RuntimeError> {
        self.execution_budget.charge(units).map(drop)
    }
    fn before_value_clone(&self, _value: &Value) -> Result<(), RuntimeError> {
        self.clone_admissions
            .set(self.clone_admissions.get().saturating_add(1));
        let result = if self.reject_copies {
            Err(VmTrap::BudgetExceeded.into_runtime_error())
        } else {
            Ok(())
        };
        #[cfg(all(feature = "std", feature = "test-support"))]
        crate::vm::diagnostic_timing::record_copy_admission(
            core::panic::Location::caller(),
            _value,
            0,
            0,
            result
                .as_ref()
                .map_or_else(crate::vm::diagnostic_timing::copy_outcome, |_| {
                    crate::vm::diagnostic_timing::CopyAdmissionOutcome::Admitted
                }),
        );
        result
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

impl crate::vm::context::ExecutionContext for TestContext {
    fn initialize_frame(
        &mut self,
        _: &crate::vm::module::VmModule,
        frame: &mut VmFrame,
        _: u32,
    ) -> Result<(), RuntimeError> {
        self.initialized_frames = self.initialized_frames.saturating_add(1);
        self.initialized_presence = frame.parameter_values_present.clone();
        if self.reject_initialization {
            Err(RuntimeError::Overflow)
        } else {
            Ok(())
        }
    }

    fn charge_call_storage(&self, bytes: usize, work: usize) -> Result<(), RuntimeError> {
        if self.reject_local_transfer && bytes == 0 && work == 1 {
            Err(RuntimeError::Overflow)
        } else {
            Ok(())
        }
    }

    fn execution_budget(&self) -> &crate::vm::budget::ExecutionBudget {
        &self.execution_budget
    }

    fn retire_frame(&mut self, _: &VmFrame) -> Result<(), RuntimeError> {
        self.retired_frames += 1;
        Ok(())
    }

    fn check_frame_return(&self, _: &VmFrame, _: &Value) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn execute_initialization_opcode(
        &mut self,
        _: &crate::vm::module::VmModule,
        _: &mut VmFrame,
        opcode: u8,
        _: u32,
        _: &mut crate::vm::OperandStack,
        _: u32,
    ) -> Result<(), RuntimeError> {
        Err(crate::vm::VmTrap::InvalidOpcode(opcode).into_runtime_error())
    }

    fn deadline_exceeded(&self) -> bool {
        false
    }

    fn on_statement(&mut self, _: &crate::vm::module::VmModule, _: u32, _: usize, _: u32) {}

    fn sizeof_value(&self, _: &Value) -> Result<u64, RuntimeError> {
        Ok(0)
    }
}

#[test]
fn native_reference_read_charges_wide_string_unicode_scan() {
    let mut context = TestContext::new();
    context
        .storage
        .set_global("text", Value::WString("\u{03b1}".repeat(64)));
    let frame = context.frame(0);
    let reference = ValueRef {
        location: MemoryLocation::Global,
        offset: 0,
        path: vec![RefSegment::Index(vec![1])],
    };
    // TIME-08 TEXT: 64 two-byte characters = 128 bytes, charged at 2 per byte = 256.
    context.execution_budget.reset(255);
    let error = super::bindings::read_vm_reference(&mut context, &frame, &reference)
        .unwrap_err()
        .into_runtime_error();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::Work,
            requested: 256,
            limit: 255,
            ..
        }
    ));
    context.execution_budget.reset(256);
    assert_eq!(
        super::bindings::read_vm_reference(&mut context, &frame, &reference).unwrap(),
        Value::WChar(0x03b1)
    );
}

impl CallContext for TestContext {
    fn recycle_optimized_result_locals(
        &mut self,
        _: u32,
        locals: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        self.recycled_locals = locals;
        Ok(())
    }
    fn try_execute_optimized_with_parameter_presence_owned(
        &mut self,
        module: &crate::vm::module::VmModule,
        pou_id: u32,
        instance: Option<InstanceId>,
        initial_locals: &mut Option<Vec<Value>>,
        present: Option<&[bool]>,
        capture_return: bool,
        depth: u32,
        entry: crate::vm::budget::ExecutionEntry,
    ) -> Result<Option<crate::vm::dispatch::VmPouStackResult>, RuntimeError> {
        self.owned_entry_calls += 1;
        if self.owned_entry_error {
            return Err(RuntimeError::InvalidExecutionState);
        }
        self.try_execute_optimized_with_parameter_presence(
            module,
            pou_id,
            instance,
            initial_locals.as_deref(),
            present,
            capture_return,
            depth,
            entry,
        )
    }

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
    fn edge_inputs(&self, owner: &str) -> Vec<VmEdgeInput> {
        if self.edge_owner.as_deref() == Some(owner) {
            self.edge_inputs.clone()
        } else {
            Vec::new()
        }
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

#[cfg(test)]
mod call_transfer_binding_tests;
#[cfg(test)]
mod owned_binding_tests;
#[cfg(test)]
mod stdlib_text_work_tests;

fn reference(frame: &VmFrame) -> ValueRef {
    ValueRef {
        location: MemoryLocation::Local(frame.activation.unwrap()),
        offset: 0,
        path: Vec::new(),
    }
}

#[test]
fn edge_transaction_preserves_rising_falling_and_phase_restore() {
    let prepared = crate::vm::PreparedModule::from_bytes(
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let pou_id = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
    let owner = SmolStr::new(prepared.vm.pou_name(pou_id).unwrap());

    let mut context = TestContext::new();
    context.edge_owner = Some(owner);
    context.edge_inputs = vec![
        VmEdgeInput {
            name: "rise".into(),
            phase_name: "rise_phase".into(),
            rising: true,
        },
        VmEdgeInput {
            name: "fall".into(),
            phase_name: "fall_phase".into(),
            rising: false,
        },
    ];
    let instance = context.storage.create_instance("Counter");
    for (name, value) in [
        ("rise", Value::Bool(true)),
        ("rise_phase", Value::Bool(false)),
        ("fall", Value::Bool(false)),
        ("fall_phase", Value::Bool(true)),
    ] {
        assert!(context.storage.set_instance_var(instance, name, value));
    }

    let transaction = crate::vm::edge::EdgeInputTransaction::begin(
        &mut context,
        &prepared.vm,
        pou_id,
        Some(instance),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        context.storage.get_instance_var(instance, "rise"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        context.storage.get_instance_var(instance, "fall"),
        Some(&Value::Bool(true))
    );
    transaction.restore(&mut context);
    assert_eq!(
        context.storage.get_instance_var(instance, "rise"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        context.storage.get_instance_var(instance, "fall"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        context.storage.get_instance_var(instance, "rise_phase"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        context.storage.get_instance_var(instance, "fall_phase"),
        Some(&Value::Bool(false))
    );

    let transaction = crate::vm::edge::EdgeInputTransaction::begin(
        &mut context,
        &prepared.vm,
        pou_id,
        Some(instance),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        context.storage.get_instance_var(instance, "rise"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        context.storage.get_instance_var(instance, "fall"),
        Some(&Value::Bool(false))
    );
    transaction.restore(&mut context);
    assert_eq!(
        context.storage.get_instance_var(instance, "rise_phase"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        context.storage.get_instance_var(instance, "fall_phase"),
        Some(&Value::Bool(false))
    );
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
