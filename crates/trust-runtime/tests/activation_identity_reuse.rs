//! Specification 34 §4.3.1 and HOST-02: every top-level entry restarts its activation identities
//! and, once every instance the entry before it created is retired, issues those instance
//! identities again, so calls and function-local instances no longer exhaust a 32-bit sequence
//! that only a restart reset. A reference into a finished entry still never resolves to a later
//! activation.
use std::cell::RefCell;

use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::{FrameId, InstanceId, MemoryLocation, VariableStorage};
use trust_runtime_core::value::{DateTimeValue, Duration, Value, ValueRef};
use trust_runtime_core::vm::{
    ExecutionServices, PreparationLimits, PreparedModule, RuntimeState, StatementFrame,
};

/// `Exercise` constructs a fresh `Leaf` object, with its two inherited parents, on each call:
/// three function-local instances per call, nine per scan. A fresh object returns
/// (5 + stride) + (5 + 2 * stride) + 7 + 11, so one scan sums 102.
const SOURCE: &str = r#"
CLASS Base
VAR PUBLIC base_value : INT := INT#5; END_VAR
METHOD PUBLIC Bump : INT
VAR_INPUT amount : INT; END_VAR
base_value := base_value + amount;
Bump := base_value;
END_METHOD
END_CLASS
CLASS Middle EXTENDS Base
VAR PUBLIC middle_value : INT := INT#7; END_VAR
END_CLASS
CLASS Leaf EXTENDS Middle
VAR PUBLIC leaf_value : INT := INT#11; END_VAR
END_CLASS
FUNCTION_BLOCK Total
VAR_INPUT stride : INT; END_VAR
VAR_OUTPUT sum : DINT; END_VAR
sum := sum + INT_TO_DINT(stride);
END_FUNCTION_BLOCK
FUNCTION Exercise : INT
VAR_INPUT stride : INT; END_VAR
VAR
    object : Leaf;
    first : INT;
    second : INT;
END_VAR
first := object.Bump(amount := stride);
second := object.Bump(amount := stride);
Exercise := first + second + object.middle_value + object.leaf_value;
END_FUNCTION
VAR_GLOBAL leak : REF_TO INT; END_VAR
PROGRAM Main
VAR
    total : Total;
    index : INT;
    sum : INT;
    scans : DINT;
END_VAR
sum := INT#0;
FOR index := INT#1 TO INT#3 DO
    sum := sum + Exercise(stride := index);
END_FOR;
total(stride := sum);
scans := scans + DINT#1;
END_PROGRAM
CONFIGURATION Conf
RESOURCE Controller ON PLC
TASK Tick (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Tick : Main;
END_RESOURCE
END_CONFIGURATION
"#;

fn prepared() -> PreparedModule {
    let bytes = CompileSession::from_source(SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted ST source")
        .encode()
        .expect("encode STBC");
    PreparedModule::from_bytes(&bytes, PreparationLimits::default()).expect("admitted STBC")
}

/// The call-stack depth, activation and runtime instance of one executed instruction.
type Instruction = (usize, Option<FrameId>, Option<InstanceId>);

/// Selects the executor and records every executed instruction on the stack path.
struct Services {
    stack: bool,
    instructions: RefCell<Vec<Instruction>>,
}
impl Services {
    fn new(stack: bool) -> Self {
        Self {
            stack,
            instructions: RefCell::new(Vec::new()),
        }
    }
}
impl ExecutionServices for Services {
    fn debug_active(&self) -> bool {
        self.stack
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
    fn on_statement_with_frame(
        &self,
        _: u32,
        _: usize,
        _: u32,
        _: &VariableStorage,
        _: Duration,
        frame: StatementFrame<'_>,
    ) {
        self.instructions.borrow_mut().push((
            frame.call_stack().count(),
            frame.activation(),
            frame.runtime_instance(),
        ));
    }
}

fn instance(value: Option<&Value>) -> InstanceId {
    match value {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected an instance, got {other:?}"),
    }
}

/// `(sum, scans, total.sum)` of the program instance.
fn results(storage: &VariableStorage) -> (Value, Value, Value) {
    let plant = instance(storage.get_global("Plant"));
    let total = instance(storage.get_instance_var(plant, "total"));
    let field = |owner, name| storage.get_instance_var(owner, name).cloned().unwrap();
    (
        field(plant, "sum"),
        field(plant, "scans"),
        field(total, "sum"),
    )
}

/// Every live instance identity with its type, in identity order.
fn live_instances(storage: &VariableStorage) -> Vec<(InstanceId, String)> {
    let mut live: Vec<_> = storage
        .instances()
        .iter()
        .map(|(id, data)| (*id, data.type_name.to_string()))
        .collect();
    live.sort();
    live
}

/// Persistent storage holds no reference into an activation and names only live instances.
fn assert_no_activation_state_outlives_its_entry(storage: &VariableStorage) {
    fn visit(storage: &VariableStorage, value: &Value) {
        match value {
            Value::Reference(Some(reference)) => {
                assert!(
                    !matches!(reference.location, MemoryLocation::Local(_)),
                    "persistent storage holds a reference into an activation: {reference:?}"
                );
                if let MemoryLocation::Instance(id) = reference.location {
                    assert!(storage.get_instance(id).is_some());
                }
            }
            Value::Instance(id) => assert!(storage.get_instance(*id).is_some()),
            Value::Array(array) => array.elements().iter().for_each(|v| visit(storage, v)),
            Value::Struct(structure) => structure.fields().values().for_each(|v| visit(storage, v)),
            _ => {}
        }
    }
    storage.globals().values().for_each(|v| visit(storage, v));
    for data in storage.instances().values() {
        data.variables.values().for_each(|v| visit(storage, v));
    }
}

#[test]
fn scans_with_function_local_instances_run_past_both_identity_limits_on_both_executors() {
    let prepared = prepared();
    for stack in [true, false] {
        let services = Services::new(stack);
        let mut state = prepared.instantiate_with_services(0, &services).unwrap();
        let installed = live_instances(state.storage());
        // Two activation identities and 64 instance identities are left. Every scan takes
        // more activations than that, and 50 scans take 450 function-local instances.
        state.set_next_identities_for_test(u32::MAX - 2, u32::MAX - 64);
        for scan in 1..=50 {
            state
                .execute_cycle(Duration::from_millis(10 * scan))
                .unwrap_or_else(|error| panic!("scan {scan} (stack {stack}): {error:?}"));
            assert_eq!(
                results(state.storage()),
                (
                    Value::Int(102),
                    Value::DInt(scan as i32),
                    Value::DInt(102 * scan as i32)
                ),
                "scan {scan} (stack {stack})"
            );
            assert_eq!(live_instances(state.storage()), installed);
            assert_eq!(state.storage().execution_frame_count(), 0);
            assert_no_activation_state_outlives_its_entry(state.storage());
            services.instructions.borrow_mut().clear();
        }
        assert!(state.fault().is_none());
    }
}

/// Every checked engineering operation refuses references into finished activations and
/// function-local instances, and none of them can be stored into PLC state.
fn assert_refused(state: &mut RuntimeState<'_>, local: &ValueRef, owned: &ValueRef) {
    let leak = state.storage().get_global("leak").cloned();
    for stale in [local, owned] {
        assert_eq!(
            state.read_reference(stale),
            Err(RuntimeError::ReferenceLifetime)
        );
        assert_eq!(
            state.write_reference(stale.as_view(), Value::Int(99)),
            Err(RuntimeError::ReferenceLifetime)
        );
    }
    assert_eq!(
        state.write_global("leak", Value::Reference(Some(local.clone()))),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(state.storage().get_global("leak").cloned(), leak);
}

/// Runs one scan and returns what a debugger that inspected it holds: the first activation
/// below the program root (the first `Exercise` call) and the first function-local instance
/// that ran.
fn observe(
    state: &mut RuntimeState<'_>,
    services: &Services,
    persistent: &[InstanceId],
    now: i64,
) -> (FrameId, InstanceId) {
    state.execute_cycle(Duration::from_millis(now)).unwrap();
    let instructions = services.instructions.take();
    let call = instructions
        .iter()
        .find_map(|(depth, activation, _)| activation.filter(|_| *depth == 2))
        .expect("a call below the program root");
    let object = instructions
        .iter()
        .filter_map(|(_, _, instance)| *instance)
        .find(|id| !persistent.contains(id))
        .expect("a function-local instance");
    (call, object)
}

#[test]
fn references_held_from_a_finished_entry_stay_refused_after_their_identities_are_reissued() {
    let prepared = prepared();
    let services = Services::new(true);
    let mut state = prepared.instantiate_with_services(0, &services).unwrap();
    let persistent: Vec<InstanceId> = live_instances(state.storage())
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let (call, object) = observe(&mut state, &services, &persistent, 10);
    let local = ValueRef {
        location: MemoryLocation::Local(call),
        offset: 0,
        generation: state.storage().generation(),
        path: Vec::new(),
    };
    let owned = ValueRef {
        location: MemoryLocation::Instance(object),
        offset: 0,
        generation: state.storage().generation(),
        path: Vec::new(),
    };
    assert_refused(&mut state, &local, &owned);
    for scan in 2..=4 {
        // The next entries give both identities to their own, different call and object,
        // which start from their declared values.
        assert_eq!(
            observe(&mut state, &services, &persistent, 10 * scan),
            (call, object)
        );
        assert_eq!(
            results(state.storage()),
            (
                Value::Int(102),
                Value::DInt(scan as i32),
                Value::DInt(102 * scan as i32)
            )
        );
        assert_no_activation_state_outlives_its_entry(state.storage());
        assert_refused(&mut state, &local, &owned);
    }
    assert!(state.fault().is_none());
}
