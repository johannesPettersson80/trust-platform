use super::lower::normalize_stack_for_block_exit;
use super::*;
use core::cell::Cell;

mod corrections;
mod owned_entry;
mod work_allowance_parity;
use crate::bytecode::BytecodeVersion;
use crate::memory::{FrameId, MemoryLocation, VariableStorage};
use crate::value::{DateTimeProfile, DateTimeValue, Duration, RefSegment, ValueRef, ValueRefView};
use crate::vm::call::context::{BuiltinParam, CallContext, VmEdgeInput};
use crate::vm::context::ReferenceContext;
use crate::vm::module::{VmPouEntry, VmRef};

fn module(code: Vec<u8>, constant: Value) -> VmModule {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
    ));
    let prepared =
        crate::vm::PreparedModule::from_bytes(bytes, crate::vm::PreparationLimits::default())
            .unwrap();
    let mut module = prepared.vm.clone();
    module.version = BytecodeVersion::SOURCE_FREE;
    module.storage_abi = module.version.storage_abi().unwrap();
    module.code = code;
    module.consts = vec![constant];
    module.refs = vec![VmRef::Global {
        offset: 0,
        path: crate::value::RefPath::new(),
    }];
    module.ref_types = vec![None; module.refs.len()];
    module.pou_by_id.clear();
    module.pou_by_id.insert(
        1,
        VmPouEntry {
            kind: crate::bytecode::PouKind::Program,
            name: "MAIN".into(),
            code_start: 0,
            code_end: module.code.len(),
            local_ref_start: 0,
            local_ref_count: 0,
            primary_instance_owner: None,
        },
    );
    module
}

struct Context {
    storage: VariableStorage,
    profile: RegisterProfileState,
    tier1: RegisterTier1SpecializedExecutorState,
    plans: PreparedRegisterPlans,
    scratch: Option<RegisterExecutionBuffers>,
    budget: super::super::budget::ExecutionBudget,
    date_profile: DateTimeProfile,
    library: crate::stdlib::StandardLibrary,
    initialized: usize,
    retired: usize,
    deadline: bool,
    charge_clone_work: bool,
    clone_checks: Cell<usize>,
    debug_active: bool,
    fail_initialization: bool,
    seen_parameter_presence: Vec<bool>,
    seen_presence_addresses: Vec<usize>,
    compatibility_optimized_calls: usize,
    compatibility_optimized_result: bool,
}
impl Context {
    fn new(module: &VmModule) -> Self {
        let plans = PreparedRegisterPlans::prepare(module, |_, _| Ok(())).unwrap();
        let local_count = module.pou(1).unwrap().local_ref_count as usize;
        let scratch = RegisterExecutionBuffers::reserve_with_frame(
            plans.max_registers(),
            plans.max_native_arguments(),
            local_count,
        )
        .unwrap();
        let mut profile = RegisterProfileState::default();
        profile.prepare(&plans);
        profile.set_enabled(true);
        let mut tier1 = RegisterTier1SpecializedExecutorState::default();
        tier1.prepare(&plans).unwrap();
        let mut storage = VariableStorage::new();
        storage.set_global("g0", Value::DInt(0));
        Self {
            storage,
            profile,
            tier1,
            plans,
            scratch: Some(scratch),
            budget: super::super::budget::ExecutionBudget::new(99),
            date_profile: DateTimeProfile::default(),
            library: crate::stdlib::StandardLibrary::new(),
            initialized: 0,
            retired: 0,
            deadline: false,
            charge_clone_work: false,
            clone_checks: Cell::new(0),
            debug_active: false,
            fail_initialization: false,
            seen_parameter_presence: Vec::new(),
            seen_presence_addresses: Vec::new(),
            compatibility_optimized_calls: 0,
            compatibility_optimized_result: false,
        }
    }
}
impl ReferenceContext for Context {
    fn charge_text_work(&self, units: usize) -> Result<(), RuntimeError> {
        self.budget.charge(units).map(drop)
    }
    fn storage(&self) -> &VariableStorage {
        &self.storage
    }
    fn storage_mut(&mut self) -> &mut VariableStorage {
        &mut self.storage
    }
    fn check_reference_write(&self, _: ValueRefView<'_>, _: &Value) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn before_value_clone(&self, _: &Value) -> Result<(), RuntimeError> {
        self.clone_checks
            .set(self.clone_checks.get().saturating_add(1));
        if self.charge_clone_work {
            self.budget.charge(1).map(|_| ())
        } else {
            Ok(())
        }
    }
    fn check_reference_read(&self, _: ValueRefView<'_>) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn initializer_reference<'a>(
        &self,
        _: u32,
        _: &'a [RefSegment],
    ) -> Result<ValueRefView<'a>, VmTrap> {
        Err(VmTrap::NullReference)
    }
    fn write_typed_storage(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> Result<bool, RuntimeError> {
        Ok(self
            .storage
            .write_by_ref_parts(location, offset, path, value))
    }
    fn write_typed_local(&self, root: &mut Value, path: &[RefSegment], value: Value) -> bool {
        if path.is_empty() {
            *root = value;
            true
        } else {
            false
        }
    }
}
impl CallContext for Context {
    fn try_execute_optimized(
        &mut self,
        _: &VmModule,
        _: u32,
        _: Option<InstanceId>,
        _: Option<&[Value]>,
        _: bool,
        _: u32,
        _: ExecutionEntry,
    ) -> Result<Option<super::super::dispatch::VmPouStackResult>, RuntimeError> {
        self.compatibility_optimized_calls += 1;
        Ok(self.compatibility_optimized_result.then_some(
            super::super::dispatch::VmPouStackResult {
                return_value: None,
                locals: Vec::new(),
            },
        ))
    }

    fn recycle_optimized_result_locals(
        &mut self,
        _pou_id: u32,
        locals: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        let scratch = self
            .scratch
            .as_mut()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        scratch.recycle_result_locals(locals)
    }

    fn profile(&self) -> &DateTimeProfile {
        &self.date_profile
    }
    fn current_time(&self) -> Duration {
        Duration::ZERO
    }
    fn current_dt(&mut self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::ProfileUnsupported("clock".into()))
    }
    fn stdlib(&self) -> &crate::stdlib::StandardLibrary {
        &self.library
    }
    fn builtin_params(&self, _: &str) -> Option<Vec<BuiltinParam>> {
        None
    }
    fn edge_inputs(&self, _: &str) -> Vec<VmEdgeInput> {
        Vec::new()
    }
    fn suspend_frame(&mut self, _: &mut super::super::VmFrame) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn resume_frame(&mut self, _: &mut super::super::VmFrame) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn record_call_op(&mut self, kind: RegisterCallOpKind) {
        self.profile.record_call_op(kind);
    }
    fn record_value_op(&mut self, kind: RegisterValueOpKind) {
        self.profile.record_value_op(kind);
    }
    fn check_native_write(
        &self,
        _: Option<&super::super::VmFrame>,
        _: &ValueRef,
        _: &Value,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn check_builtin_call(&self, _: InstanceId) -> Result<(), RuntimeError> {
        Ok(())
    }
}
impl ExecutionContext for Context {
    fn initialize_frame(
        &mut self,
        _: &VmModule,
        frame: &mut super::super::VmFrame,
        _: u32,
    ) -> Result<(), RuntimeError> {
        self.initialized += 1;
        self.seen_parameter_presence
            .clone_from(&frame.parameter_values_present);
        self.seen_presence_addresses
            .push(frame.parameter_values_present.as_ptr() as usize);
        frame.activation = Some(FrameId(self.initialized as u32));
        if self.fail_initialization {
            return Err(RuntimeError::InvalidExecutionState);
        }
        Ok(())
    }
    fn execution_budget(&self) -> &super::super::budget::ExecutionBudget {
        &self.budget
    }
    fn retire_frame(&mut self, _: &super::super::VmFrame) -> Result<(), RuntimeError> {
        self.retired += 1;
        Ok(())
    }
    fn check_frame_return(&self, _: &super::super::VmFrame, _: &Value) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn execute_initialization_opcode(
        &mut self,
        _: &VmModule,
        _: &mut super::super::VmFrame,
        _: u8,
        _: u32,
        _: &mut OperandStack,
        _: u32,
    ) -> Result<(), RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }
    fn deadline_exceeded(&self) -> bool {
        self.deadline
    }
    fn on_statement(&mut self, _: &VmModule, _: u32, _: usize, _: u32) {}
    fn sizeof_value(&self, _: &Value) -> Result<u64, RuntimeError> {
        Ok(4)
    }
}
impl RegisterContext for Context {
    fn register_profile(&self) -> &RegisterProfileState {
        &self.profile
    }
    fn register_profile_mut(&mut self) -> &mut RegisterProfileState {
        &mut self.profile
    }
    fn tier1_state(&self) -> &RegisterTier1SpecializedExecutorState {
        &self.tier1
    }
    fn tier1_state_mut(&mut self) -> &mut RegisterTier1SpecializedExecutorState {
        &mut self.tier1
    }
    fn prepared_register_plan(&self, id: u32) -> Option<Arc<PreparedRegisterProgram>> {
        self.plans.get(id)
    }
    fn register_buffers_available(&self, _pou_id: u32) -> bool {
        !self.debug_active
            && self
                .scratch
                .as_ref()
                .is_some_and(RegisterExecutionBuffers::can_enter_with_frame_payload)
    }
    fn debug_active(&self) -> bool {
        self.debug_active
    }
    fn take_register_buffers(
        &mut self,
        _pou_id: u32,
        count: usize,
    ) -> Result<RegisterExecutionBuffers, RuntimeError> {
        let mut scratch = self
            .scratch
            .take()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        scratch.select(count)?;
        Ok(scratch)
    }
    fn recycle_register_buffers(
        &mut self,
        _pou_id: u32,
        scratch: RegisterExecutionBuffers,
    ) -> Result<(), RuntimeError> {
        if self.scratch.is_some() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        self.scratch = Some(scratch);
        Ok(())
    }
}

#[test]
fn public_dispatch_entry_keeps_its_explicit_stack_root() {
    let module = module(vec![0x06], Value::DInt(1));
    let mut context = Context::new(&module);
    context.compatibility_optimized_result = true;
    let result = crate::vm::dispatch::execute_pou_stack_with_locals(
        &mut context,
        &module,
        1,
        None,
        None,
        false,
        0,
        ExecutionEntry::Nested,
    )
    .unwrap();
    assert_eq!(result.return_value, None);
    assert_eq!(context.compatibility_optimized_calls, 0);
    assert_eq!(context.initialized, 1);
    assert_eq!(context.retired, 1);
}

#[test]
fn completion_deadline_recycles_returned_locals_into_the_activation_lease() {
    let mut module = module(vec![0x06], Value::DInt(1));
    module.pou_by_id.get_mut(&1).unwrap().local_ref_count = 1;
    let mut context = Context::new(&module);
    let mut activation = start_register_activation(
        &mut context,
        &module,
        1,
        None,
        None,
        None,
        None,
        false,
        0,
        ExecutionEntry::Nested,
        true,
    )
    .unwrap()
    .unwrap();

    context.deadline = true;
    assert!(matches!(
        step_register_activation(&mut context, &module, &mut activation),
        Err(error) if error == VmTrap::DeadlineExceeded.into_runtime_error()
    ));
    assert_eq!(context.retired, 1);
    assert!(
        context.scratch.is_none(),
        "the active call still owns its lease"
    );
    abort_register_activation(&mut context, activation).unwrap();
    assert!(context.scratch.is_some(), "the lease returns exactly once");

    context.deadline = false;
    let activation = start_register_activation(
        &mut context,
        &module,
        1,
        None,
        None,
        None,
        None,
        false,
        0,
        ExecutionEntry::Nested,
        true,
    )
    .unwrap()
    .unwrap();
    abort_register_activation(&mut context, activation).unwrap();
    assert!(context.scratch.is_some());
}

#[test]
fn nested_scratch_fallback_has_no_execution_side_effects_or_budget_reset() {
    let module = module(vec![0x06], Value::DInt(1));
    let mut context = Context::new(&module);
    let scratch = context.scratch.take().unwrap();
    context.budget.charge(17).unwrap();
    let remaining = context.budget.remaining();
    assert!(try_execute_pou_with_register_ir_with_locals(
        &mut context,
        &module,
        1,
        None,
        None,
        false,
        1,
        ExecutionEntry::Nested
    )
    .unwrap()
    .is_none());
    assert_eq!(context.budget.remaining(), remaining);
    assert_eq!(context.initialized, 0);
    assert_eq!(context.retired, 0);
    assert_eq!(context.storage.get_global("g0"), Some(&Value::DInt(0)));
    assert_eq!(
        context.profile.snapshot().fallback_reasons[0].reason,
        "nested_scratch"
    );
    context.scratch = Some(scratch);
}

#[test]
fn register_fault_retires_frame_and_returns_clean_owner_scratch() {
    let module = module(
        vec![0x10, 0, 0, 0, 0, 0x10, 0, 0, 0, 0, 0x43, 0x06],
        Value::DInt(0),
    );
    let mut context = Context::new(&module);
    assert_eq!(
        try_execute_pou_with_register_ir(&mut context, &module, 1, None),
        Err(RuntimeError::DivisionByZero)
    );
    assert_eq!(context.initialized, 1);
    assert_eq!(context.retired, 1);
    let (frames, registers, counts, stack) = context.scratch.as_mut().unwrap().buffers_mut();
    assert!(frames.is_empty());
    assert!(registers.iter().all(|value| matches!(value, Value::Null)));
    assert!(counts.iter().all(|count| *count == 0));
    assert!(stack.peek().is_err());
}

#[test]
fn prepared_lowering_budget_rejects_before_any_source_plan_reservation() {
    let module = module(vec![0x06], Value::DInt(1));
    let mut calls = 0;
    let result = PreparedRegisterPlans::prepare(&module, |_, _| {
        calls += 1;
        Err(RuntimeError::PreparationLimit)
    });
    assert!(matches!(result, Err(RuntimeError::PreparationLimit)));
    assert_eq!(calls, 1);
}

#[test]
fn prepared_scratch_rejects_work_and_storage_before_allocating() {
    let mut module = module(vec![0x06], Value::DInt(1));
    module.pou_by_id.get_mut(&1).unwrap().local_ref_count = 3;
    let plans = PreparedRegisterPlans::prepare(&module, |_, _| Ok(())).unwrap();
    let base = RegisterExecutionBuffers::reservation_demand(plans.max_registers(), 0).unwrap();
    let with_frame =
        RegisterExecutionBuffers::reservation_demand_with_frame(plans.max_registers(), 0, 3)
            .unwrap();
    assert_eq!(
        with_frame.0 - base.0,
        3 * (core::mem::size_of::<Value>() + core::mem::size_of::<bool>())
    );
    assert_eq!(with_frame.1 - base.1, 3);
    let mut charged = Vec::new();
    let buffers = plans
        .reserve_execution_buffers(&module, |bytes, work| {
            charged.push((bytes, work));
            Ok(())
        })
        .unwrap();
    assert_eq!(buffers.len(), 1);
    assert_eq!(buffers[0].0, 1);
    assert!(buffers[0].1.is_some());
    assert!(charged.iter().any(|(bytes, _)| *bytes > 0));
    assert!(charged.iter().map(|(_, work)| *work).sum::<usize>() > 1);
    assert!(matches!(
        plans.reserve_execution_buffers(&module, |_, _| Err(RuntimeError::PreparationLimit)),
        Err(RuntimeError::PreparationLimit)
    ));
    assert!(matches!(
        plans.reserve_execution_buffers(&module, |bytes, _| {
            if bytes > 0 {
                Err(RuntimeError::PreparationLimit)
            } else {
                Ok(())
            }
        }),
        Err(RuntimeError::PreparationLimit)
    ));
}

#[test]
fn sparse_prepared_reads_equal_canonical_register_use_counts() {
    let module = module(vec![0x10, 0, 0, 0, 0, 0x11, 0x40, 0x06], Value::DInt(2));
    let program = lower_pou_to_register_ir(&module, 1).unwrap();
    let dense = register_read_counts_by_block(&program);
    let sparse = sparse_register_read_counts_by_block(&program);
    for (dense, sparse) in dense.iter().zip(sparse) {
        let mut expanded = vec![0; program.max_registers as usize];
        for (index, count) in sparse {
            expanded[index as usize] = count;
        }
        assert_eq!(dense, &expanded);
    }
}

#[test]
fn profile_reset_preserves_tier1_selection_prepared_plans_and_key_reservations() {
    let module = module(vec![0x06], Value::DInt(1));
    let mut context = Context::new(&module);
    let index = context.plans.get(1).unwrap().tier1_selection_indices[0].unwrap();
    context.tier1.hot_block_threshold = 2;
    assert!(!context.tier1.select_prepared_block(index));
    assert!(context.tier1.select_prepared_block(index));
    context.profile.reset();
    assert_eq!(context.tier1.snapshot().cached_blocks, 1);
    assert!(
        context.tier1.select_prepared_block(index),
        "profile-only reset leaves cached selection immediately eligible"
    );
    assert_eq!(context.tier1.snapshot().cache_evictions, 0);
    assert!(context.plans.get(1).is_some());
    try_execute_pou_with_register_ir(&mut context, &module, 1, None).unwrap();
    assert_eq!(context.profile.snapshot().register_programs_executed, 1);
}

#[test]
fn immutable_preparation_reasons_survive_clearing_execution_observations() {
    let module = module(vec![0x60, 0, 0, 0, 0, 0x06], Value::DInt(1));
    let plans = PreparedRegisterPlans::prepare(&module, |_, _| Ok(())).unwrap();
    let preparation = plans.preparation_snapshot();
    assert_eq!(preparation.pou_count, 1);
    assert_eq!(preparation.lowered_pous, 1);
    assert_eq!(preparation.lowering_failures, 0);
    assert_eq!(preparation.specialized_blocks, 0);
    assert_eq!(preparation.specialization_failures, 1);
    assert_eq!(
        preparation.specialization_failure_reasons,
        vec![VmTier1SpecializedExecutorCompileFailureReason {
            reason: "unsupported_instr:size_of_type".into(),
            count: 1,
        },]
    );
    let mut observations = RegisterTier1SpecializedExecutorState::default();
    observations.prepare(&plans).unwrap();
    observations.reset();
    assert_eq!(observations.snapshot().compile_failures, 0);
    assert_eq!(plans.preparation_snapshot(), preparation);
}

#[test]
fn stack_and_register_sizeof_use_the_installed_temporal_abi() {
    use crate::bytecode::{TypeData, TypeEntry, TypeKind, TypeTable};
    use crate::value::StorageAbi;
    for (version, abi, width) in [
        (
            BytecodeVersion::SOURCE_FREE_V2,
            StorageAbi::LegacyTemporal32,
            4,
        ),
        (
            BytecodeVersion::SOURCE_FREE,
            StorageAbi::UniformTemporal64,
            8,
        ),
    ] {
        for (type_id, count) in [(0u32, 1), (1, 6)] {
            let mut code = vec![0x60];
            code.extend_from_slice(&type_id.to_le_bytes());
            code.extend_from_slice(&[0x21, 0, 0, 0, 0, 0x06]);
            let mut module = module(code, Value::Time(Duration::from_nanos(-1)));
            module.version = version;
            module.storage_abi = abi;
            module.types = TypeTable {
                offsets: vec![],
                entries: vec![
                    TypeEntry {
                        kind: TypeKind::Primitive,
                        name_idx: None,
                        data: TypeData::Primitive {
                            prim_id: 16,
                            max_length: 0,
                        },
                    },
                    TypeEntry {
                        kind: TypeKind::Array,
                        name_idx: None,
                        data: TypeData::Array {
                            elem_type_id: 0,
                            dims: vec![(-2, 0), (4, 5)],
                        },
                    },
                ],
            };
            let mut register = Context::new(&module);
            assert_eq!(
                try_execute_pou_with_register_ir(&mut register, &module, 1, None).unwrap(),
                RegisterExecutionOutcome::Executed
            );
            assert_eq!(
                register.storage.get_global("g0"),
                Some(&Value::DInt(width * count))
            );
            let mut stack = Context::new(&module);
            crate::vm::dispatch::execute_pou_stack_with_locals(
                &mut stack,
                &module,
                1,
                None,
                None,
                false,
                0,
                ExecutionEntry::Root,
            )
            .unwrap();
            assert_eq!(
                stack.storage.get_global("g0"),
                register.storage.get_global("g0")
            );
        }
    }
}

#[test]
fn duplicated_self_survives_lowering_and_register_self_field_store() {
    // DUP/SWAP change stack identities without producing register instructions.
    // The store therefore reads the LoadSelf register inside the fusion window.
    let mut module = module(
        vec![0x23, 0x11, 0x30, 0, 0, 0, 0, 0x13, 0x33, 0x06],
        Value::DInt(0),
    );
    module.strings = vec!["self_value".into()];
    module.pou_by_id.get_mut(&1).unwrap().primary_instance_owner = Some(1);
    let program = lower_pou_to_register_ir(&module, 1).unwrap();
    assert!(program
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| matches!(instruction, RegisterInstr::LoadSelf { .. })));
    assert!(!program
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| matches!(instruction, RegisterInstr::StoreSelfFieldDynamic { .. })));

    let mut register = Context::new(&module);
    let register_instance = register.storage.create_instance("SelfHolder");
    register
        .storage
        .set_instance_var(register_instance, "self_value", Value::Null);
    assert_eq!(
        try_execute_pou_with_register_ir(&mut register, &module, 1, Some(register_instance))
            .unwrap(),
        RegisterExecutionOutcome::Executed
    );
    assert_eq!(
        register
            .storage
            .get_instance_var(register_instance, "self_value"),
        Some(&Value::Instance(register_instance))
    );

    let mut stack = Context::new(&module);
    let stack_instance = stack.storage.create_instance("SelfHolder");
    stack
        .storage
        .set_instance_var(stack_instance, "self_value", Value::Null);
    crate::vm::dispatch::execute_pou_stack_with_locals(
        &mut stack,
        &module,
        1,
        Some(stack_instance),
        None,
        false,
        0,
        ExecutionEntry::Root,
    )
    .unwrap();
    assert_eq!(
        stack.storage.get_instance_var(stack_instance, "self_value"),
        Some(&Value::Instance(stack_instance))
    );
}

#[test]
fn precompiled_tier1_selection_honors_threshold_and_capacity_without_new_compiles() {
    let module = module(vec![0x00, 0x02, 0, 0, 0, 0, 0x00, 0x06], Value::DInt(1));
    let plans = PreparedRegisterPlans::prepare(&module, |_, _| Ok(())).unwrap();
    let plan = plans.get(1).unwrap();
    let first = plan.tier1_selection_indices[0].unwrap();
    let second = plan.tier1_selection_indices[1].unwrap();
    let mut state = RegisterTier1SpecializedExecutorState::default();
    state.hot_block_threshold = 2;
    state.cache_capacity = 1;
    state.prepare(&plans).unwrap();
    let preparation = state.snapshot();
    assert!(!state.select_prepared_block(first));
    assert!(state.select_prepared_block(first));
    assert!(!state.select_prepared_block(second));
    assert!(state.select_prepared_block(second));
    assert!(state.select_prepared_block(first));
    let running = state.snapshot();
    assert_eq!(running.cached_blocks, 1);
    assert_eq!(running.cache_evictions, 2);
    assert_eq!(running.compile_attempts, preparation.compile_attempts);
    assert_eq!(running.compile_successes, preparation.compile_successes);
    state.reset();
    assert_eq!(state.snapshot().cached_blocks, 0);
    assert!(
        !state.select_prepared_block(first),
        "explicit tier1 reset clears selection and rewarms prepared code"
    );
    assert!(state.select_prepared_block(first));
    assert_eq!(state.snapshot().cache_evictions, 0);
    assert!(
        !state.select_prepared_block(second),
        "all selection hit counters restart at zero"
    );
    assert!(state.select_prepared_block(second));
    assert_eq!(state.snapshot().cache_evictions, 1);
    // Spec 34 HOST-04: eviction keeps the hotness count, so `first` is selected at its next entry.
    assert!(
        state.select_prepared_block(first),
        "an evicted block is selected again at once without re-warming"
    );
    assert_eq!(state.snapshot().cache_evictions, 2);
    assert_eq!(state.snapshot().compile_attempts, 0);
}

#[test]
fn indexed_parallel_moves_preserve_every_small_permutation_and_duplicate_source() {
    for width in 1..=7usize {
        // Enumerate base-width source assignments, including duplicate sources,
        // rather than only bijective cycles. Keep the finite test corpus small.
        let cases = width.pow(3);
        for mut encoded in 0..cases {
            let mut sources = (0..width)
                .map(|slot| RegisterId(slot as u32))
                .collect::<Vec<_>>();
            for source in sources.iter_mut().take(3) {
                *source = RegisterId((encoded % width) as u32);
                encoded /= width;
            }
            let expected = sources
                .iter()
                .map(|source| source.index())
                .collect::<Vec<_>>();
            let mut next = width as u32;
            let mut moves = Vec::new();
            normalize_stack_for_block_exit(&mut next, &mut moves, &sources, None).unwrap();
            let mut actual = (0..next).collect::<Vec<_>>();
            for instruction in moves {
                let RegisterInstr::Move { src, dest } = instruction else {
                    panic!("move only")
                };
                actual[dest.index() as usize] = actual[src.index() as usize];
            }
            assert_eq!(&actual[..width], expected.as_slice());
        }
    }
}

#[test]
fn branch_dense_preparation_has_logarithmic_work_growth_and_linear_plan_reads() {
    fn measured(blocks: usize) -> (usize, usize) {
        let mut code = Vec::new();
        for _ in 0..blocks {
            code.push(0x00);
            code.extend_from_slice(&[0x02, 0, 0, 0, 0]);
        }
        code.push(0x06);
        let module = module(code, Value::DInt(1));
        let mut work = 0usize;
        let mut bytes = 0usize;
        let plans = PreparedRegisterPlans::prepare(&module, |charged_bytes, charged_work| {
            bytes = bytes.checked_add(charged_bytes).unwrap();
            work = work.checked_add(charged_work).unwrap();
            Ok(())
        })
        .unwrap();
        let plan = plans.get(1).unwrap();
        let RegisterLoweringCacheEntry::Ready(lowered) = &plan.lowered else {
            panic!("prepared")
        };
        assert_eq!(lowered.program.blocks.len(), blocks + 1);
        assert!(lowered
            .register_read_counts_by_block
            .iter()
            .all(Vec::is_empty));
        (work, bytes)
    }
    let small = measured(256);
    let large = measured(1024);
    assert!(
        large.0 < small.0 * 6,
        "work must follow input log input, not blocks squared"
    );
    assert!(
        large.1 < small.1 * 5,
        "scratch demand must stay linear for zero-entry-depth blocks"
    );
}

#[test]
fn optimized_reference_operands_materialize_string_elements_without_losing_faults() {
    for (text, expected) in [
        (Value::String("AB".into()), Value::Char(b'B')),
        (
            Value::WString("\u{03b1}\u{03b2}".into()),
            Value::WChar(0x03b2),
        ),
    ] {
        let mut module = module(
            vec![0x20, 0, 0, 0, 0, 0x21, 1, 0, 0, 0, 0x06],
            Value::DInt(1),
        );
        module.refs = vec![
            VmRef::Global {
                offset: 0,
                path: vec![RefSegment::Index(vec![2])],
            },
            VmRef::Global {
                offset: 1,
                path: Vec::new(),
            },
        ];
        module.ref_types = vec![None; module.refs.len()];
        for specialized in [false, true] {
            let mut context = Context::new(&module);
            context.storage.set_global("g0", text.clone());
            context.storage.set_global("out", Value::Null);
            context.tier1.set_enabled(specialized);
            context.tier1.hot_block_threshold = 1;
            assert_eq!(
                try_execute_pou_with_register_ir(&mut context, &module, 1, None).unwrap(),
                RegisterExecutionOutcome::Executed
            );
            assert_eq!(context.storage.get_global("out"), Some(&expected));
        }
        module.refs[0] = VmRef::Global {
            offset: 0,
            path: vec![RefSegment::Index(vec![3])],
        };
        let mut context = Context::new(&module);
        context.storage.set_global("g0", text.clone());
        context.storage.set_global("out", Value::Null);
        let error = try_execute_pou_with_register_ir(&mut context, &module, 1, None).unwrap_err();
        let common = load_ref(&context, &module, &FrameStack::default(), 0)
            .unwrap_err()
            .into_runtime_error();
        assert_eq!(error, common, "preserve the shared reference fault exactly");
        assert!(matches!(
            register_ref_operand(&context, &module, &FrameStack::default(), 999),
            Err(VmTrap::InvalidRefIndex(999))
        ));
        assert_eq!(context.storage.get_global("out"), Some(&Value::Null));
        assert!(context.scratch.is_some());
    }
}

#[test]
fn failed_scratch_selection_preserves_values_and_cleanup_is_repeatable() {
    let mut scratch = RegisterExecutionBuffers::reserve(2, 1).unwrap();
    {
        let (_, values, reads, operands) = scratch.buffers_mut();
        values[0] = Value::String("owned scratch".into());
        reads[0] = 5;
        operands.push(Value::DInt(3)).unwrap();
    }
    assert_eq!(scratch.select(3), Err(RuntimeError::PreparationLimit));
    {
        let (_, values, reads, operands) = scratch.buffers_mut();
        assert_eq!(values[0], Value::String("owned scratch".into()));
        assert_eq!(reads[0], 5);
        assert_eq!(operands.peek().unwrap(), &Value::DInt(3));
    }
    scratch.clear();
    scratch.clear();
    scratch.select(2).unwrap();
    let (frames, values, reads, operands) = scratch.buffers_mut();
    assert!(frames.is_empty());
    assert!(values.iter().all(|value| matches!(value, Value::Null)));
    assert!(reads.iter().all(|count| *count == 0));
    assert!(operands.peek().is_err());
}

#[test]
fn for_zero_step_halt_lowers_to_the_register_trap_without_a_pou_fallback() {
    // LOAD_CONST TRUE; JMP_FALSE past the trap; HALT; RETURN. The encoder lays out a zero-step
    // guard this way: the HALT is the fall-through of its JMP_FALSE and begins its own block.
    let mut code = vec![0x10, 0, 0, 0, 0, 0x04];
    code.extend_from_slice(&1_i32.to_le_bytes());
    code.extend_from_slice(&[0x01, 0x06]);
    let module = module(code, Value::Bool(true));
    let program = lower_pou_to_register_ir(&module, 1).unwrap();
    assert_eq!(program.blocks.len(), 3);
    assert_eq!(
        program.blocks[1].instructions,
        vec![RegisterInstr::ForStepZero]
    );
    assert_eq!(program.blocks[1].instruction_costs, vec![1]);
    let plans = PreparedRegisterPlans::prepare(&module, |_, _| Ok(())).unwrap();
    assert_eq!(plans.get(1).unwrap().fallback_reason(), None);
    // Tier-1 leaves only the trap block to the interpreter.
    assert_eq!(
        plans.preparation_snapshot().specialization_failure_reasons,
        vec![VmTier1SpecializedExecutorCompileFailureReason {
            reason: "unsupported_instr:for_step_zero".into(),
            count: 1,
        }]
    );
}
