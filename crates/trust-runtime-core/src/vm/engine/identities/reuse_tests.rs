//! Specification 34 §4.3.1 and HOST-02: each top-level entry restarts its activation identities
//! and, once every instance the entry before it created is retired, issues those instance
//! identities again, so a runtime keeps running long after one increasing 32-bit sequence would
//! have been exhausted, while a reference into a finished entry still never resolves to a later
//! activation.
use super::super::*;
use crate::memory::MemoryLocation;
use core::cell::RefCell;

const PROGRAM: &[u8] = include_bytes!(
    "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
);
const TRACE: &str = include_str!(
    "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/expected-a4-trace.csv"
);

fn prepared() -> PreparedModule {
    PreparedModule::from_bytes(PROGRAM, crate::vm::PreparationLimits::default()).unwrap()
}

fn instance(value: Option<&Value>) -> InstanceId {
    match value {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected an instance, got {other:?}"),
    }
}

/// The persistent program root and the function-block instances it owns.
fn roots(storage: &VariableStorage) -> (InstanceId, InstanceId, InstanceId) {
    let plant = instance(storage.get_global("Plant"));
    let counter = instance(storage.get_instance_var(plant, "counter"));
    let delay = instance(storage.get_instance_var(plant, "delay"));
    (plant, counter, delay)
}

/// Counter value, result, activations, TON output and elapsed time, as the A4 trace lists them.
fn sample(storage: &VariableStorage) -> (Value, Value, Value, Value, Value) {
    let (plant, counter, delay) = roots(storage);
    let field = |owner, name| storage.get_instance_var(owner, name).cloned().unwrap();
    (
        field(counter, "value"),
        field(plant, "result"),
        field(plant, "activations"),
        field(delay, "Q"),
        field(delay, "ET"),
    )
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

/// The call-stack depth, POU and activation of one executed instruction.
type Instruction = (usize, u32, Option<FrameId>);

/// Records every executed instruction.
#[derive(Default)]
struct Observer {
    instructions: RefCell<Vec<Instruction>>,
}
impl Observer {
    /// The single activation that ran below the program root, other than `except`'s body,
    /// since the previous call.
    fn nested_activation(&self, except: u32) -> FrameId {
        let instructions = self.instructions.take();
        let mut activations = instructions
            .iter()
            .filter(|(depth, pou, _)| *depth == 2 && *pou != except)
            .map(|(_, _, activation)| activation.expect("admitted invocation"));
        let first = activations.next().expect("a nested invocation ran");
        assert!(activations.all(|activation| activation == first));
        first
    }
}
impl ExecutionServices for Observer {
    fn debug_active(&self) -> bool {
        true
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<crate::value::DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
    fn on_statement_with_frame(
        &self,
        pou: u32,
        _: usize,
        _: u32,
        _: &VariableStorage,
        _: Duration,
        frame: crate::vm::StatementFrame<'_>,
    ) {
        self.instructions
            .borrow_mut()
            .push((frame.call_stack().count(), pou, frame.activation()));
    }
}

/// Every checked engineering operation refuses a reference into a finished activation.
fn assert_refused(state: &mut EngineState<'_>, stale: &ValueRef) {
    assert_eq!(
        state.engineering_operation(|engine| engine.read_checked_reference(stale)),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(
        state.engineering_operation(|engine| {
            engine.write_checked_reference(stale.as_view(), Value::Int(99))
        }),
        Err(RuntimeError::ReferenceLifetime)
    );
}

#[test]
fn scans_run_past_the_activation_identity_limit_and_follow_the_a4_trace() {
    let prepared = prepared();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let persistent = data.storage.instances().len();
    let installed_roots = roots(&data.storage);
    // One activation identity is left. Each task activation takes four: Main, the Counter
    // call, the Advance method and the result of Advance's call-time initializer.
    data.storage.set_next_frame_id_for_test(u32::MAX - 1);
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let mut samples = 0;
    for row in TRACE.lines().filter(|line| !line.starts_with('#')).skip(1) {
        let n: Vec<i64> = row.split(',').map(|cell| cell.parse().unwrap()).collect();
        state.execute_cycle(Duration::from_millis(n[0])).unwrap();
        assert_eq!(
            sample(&state.data.storage),
            (
                Value::Int(n[1] as i16),
                Value::Int(n[2] as i16),
                Value::DInt(n[3] as i32),
                Value::Bool(n[4] != 0),
                Value::Time(Duration::from_millis(n[5]))
            ),
            "sample {}",
            n[0]
        );
        assert_eq!(state.data.storage.execution_frame_count(), 0);
        assert_eq!(state.data.storage.instances().len(), persistent);
        assert_eq!(roots(&state.data.storage), installed_roots);
        samples += 1;
    }
    // The trace ends after 40 task activations, 160 activations against the one left.
    assert_eq!(sample(&state.data.storage).2, Value::DInt(40));
    assert!(samples >= 40);
    assert!(state.data.fault.is_none());
}

#[test]
fn engineering_entries_reissue_the_identities_the_entry_before_them_released() {
    let prepared = prepared();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let persistent = data.storage.instances().len();
    let installed_roots = roots(&data.storage);
    // No activation identity and one instance identity are left. Each entry reserves an
    // engineering scope, constructs a temporary Counter, initializes and runs it.
    data.storage.set_next_frame_id_for_test(u32::MAX);
    data.storage.set_next_instance_id_for_test(u32::MAX - 1);
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    for _ in 0..8 {
        state
            .engineering_operation(|engine| engine.execute_temporary_function_block("Counter"))
            .unwrap();
        assert_eq!(state.data.storage.instances().len(), persistent);
        assert_eq!(state.data.storage.execution_frame_count(), 0);
        assert_eq!(roots(&state.data.storage), installed_roots);
    }
    // The same building blocks show the reissued values: each entry's first activation is 0,
    // and its invocation-owned instance takes the identity the previous entry released.
    let pou = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
    for _ in 0..4 {
        let (scope, owned) = state
            .engineering_operation(|engine| {
                let scope = engine.reserve_activation_identity()?;
                engine.data.storage.activate_execution_frame(scope)?;
                let owned = engine.reserve_instance(pou, Some(scope))?;
                engine.remove_owned_instances(scope)?;
                assert!(engine.data.storage.release_execution_frame(scope));
                Ok((scope, owned))
            })
            .unwrap();
        assert_eq!((scope, owned), (FrameId(0), InstanceId(u32::MAX - 1)));
        assert!(state.data.storage.get_instance(owned).is_none());
    }
    assert_eq!(state.data.storage.instances().len(), persistent);
}

#[test]
fn a_local_reference_from_a_finished_entry_never_resolves_after_its_identity_is_reissued() {
    let prepared = prepared();
    let counter = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
    let observer = Observer::default();
    let mut data = EngineState::construct_data(&prepared, 0, &observer).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &observer);
    // The first task activation (A4 trace, 30 ms) runs the Advance method below Main. A
    // debugger that inspected it holds a reference to the method's return slot.
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    let advance = observer.nested_activation(counter);
    let stale = ValueRef {
        location: MemoryLocation::Local(advance),
        offset: 0,
        generation: state.data.storage.generation(),
        path: Vec::new(),
    };
    assert_no_activation_state_outlives_its_entry(&state.data.storage);
    for (now, result) in [(50, 2), (80, 3), (100, 1)] {
        assert_refused(&mut state, &stale);
        state.execute_cycle(Duration::from_millis(now)).unwrap();
        // The next entry gives the same identity to its own, different Advance activation.
        assert_eq!(observer.nested_activation(counter), advance);
        assert_eq!(sample(&state.data.storage).1, Value::Int(result));
        assert_no_activation_state_outlives_its_entry(&state.data.storage);
    }
    assert_refused(&mut state, &stale);
    assert!(state.data.fault.is_none());
}
