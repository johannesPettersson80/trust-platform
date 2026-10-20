//! Installation generations (spec 34 §11.3, MEM-07, UPDATE-04): a reference taken before a
//! restart or a replacement names no state afterwards, a refused restart and a resource-only
//! reconfiguration keep every reference, and a fresh reference of the installed generation works.
use trust_runtime_core::{
    error::RuntimeError,
    io_address::IoAddress,
    memory::Generation,
    retain::RestartMode,
    value::{DateTimeValue, Duration, Value, ValueRef},
    vm::{ExecutionServices, PreparationLimits, PreparedModule, RuntimeState},
};

const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

/// A global, a program member and the persistent binding of the `%QX0.0` output.
fn handles(prepared: &PreparedModule, state: &mut RuntimeState<'_>) -> [ValueRef; 3] {
    let global = state.storage().ref_for_global("probe_depth").unwrap();
    let Some(Value::Instance(plant)) = state.storage().get_global("Plant") else {
        panic!("program root")
    };
    let member = state
        .storage()
        .ref_for_instance(*plant, "depth_result")
        .unwrap();
    let output = IoAddress::parse("%QX0.0").unwrap();
    let binding = prepared
        .io_bindings()
        .iter()
        .find(|binding| binding.address == output)
        .expect("the fixture binds %QX0.0");
    let image = state.resolve_persistent_reference(binding.ref_idx).unwrap();
    [global, member, image]
}

/// Every PLC value and image the checked operations could change.
fn observable(state: &RuntimeState<'_>) -> (Vec<Value>, Vec<Value>, Vec<u8>) {
    let globals = state.storage().globals().values().cloned().collect();
    let Some(Value::Instance(plant)) = state.storage().get_global("Plant") else {
        panic!("program root")
    };
    let members = state
        .storage()
        .get_instance(*plant)
        .unwrap()
        .variables
        .values()
        .cloned()
        .collect();
    (globals, members, state.outputs().to_vec())
}

/// A stale reference reads nothing and every checked operation refuses it before any change.
fn assert_stale(state: &mut RuntimeState<'_>, stale: &[ValueRef; 3]) {
    let before = observable(state);
    for reference in stale {
        assert_ne!(reference.generation, state.storage().generation());
        assert_eq!(state.storage().read_by_ref_ref(reference), None);
        assert_eq!(state.storage().materialize_by_ref_ref(reference), None);
        assert_eq!(
            state.read_reference(reference),
            Err(RuntimeError::ReferenceLifetime)
        );
        let value = match state.storage().read_by_ref_ref(&fresh(state, reference)) {
            Some(Value::Bool(_)) => Value::Bool(true),
            _ => Value::Int(3),
        };
        assert_eq!(
            state.write_reference(reference.as_view(), value),
            Err(RuntimeError::ReferenceLifetime)
        );
        assert_eq!(
            state.execute_function_block(reference),
            Err(RuntimeError::ReferenceLifetime)
        );
    }
    assert_eq!(
        observable(state),
        before,
        "a refused handle changes nothing"
    );
    assert!(state.fault().is_none());
}

/// The same address in the installed generation.
fn fresh(state: &RuntimeState<'_>, stale: &ValueRef) -> ValueRef {
    ValueRef {
        generation: state.storage().generation(),
        ..stale.clone()
    }
}

#[test]
fn handles_go_stale_at_every_restart_and_fresh_ones_resolve() {
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    assert_eq!(state.storage().generation(), Generation::FIRST);
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let first = handles(&prepared, &mut state);
    assert_eq!(
        state.read_reference(&first[0]).unwrap(),
        Value::Int(1),
        "a reference of the installed generation reads its destination"
    );

    state.restart(RestartMode::Warm).unwrap();
    assert_eq!(state.storage().generation(), Generation(1));
    assert_stale(&mut state, &first);
    let second = handles(&prepared, &mut state);
    assert_eq!(second[0].generation, Generation(1));
    state
        .write_reference(second[0].as_view(), Value::Int(2))
        .unwrap();
    assert_eq!(state.read_reference(&second[0]).unwrap(), Value::Int(2));

    state.restart(RestartMode::Cold).unwrap();
    assert_eq!(state.storage().generation(), Generation(2));
    assert_stale(&mut state, &first);
    assert_stale(&mut state, &second);
    let third = handles(&prepared, &mut state);
    assert_eq!(state.read_reference(&third[0]).unwrap(), Value::Int(1));
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert!(state.fault().is_none());
}

/// Physical deadline services whose expiry the test switches on.
struct Deadline(std::cell::Cell<bool>);

impl ExecutionServices for Deadline {
    fn deadline_exceeded(&self) -> bool {
        self.0.get()
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

#[test]
fn a_refused_restart_keeps_the_generation_and_every_handle() {
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let deadline = Deadline(std::cell::Cell::new(false));
    let mut state = prepared.instantiate_with_services(0, &deadline).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let held = handles(&prepared, &mut state);

    deadline.0.set(true);
    assert_eq!(
        state.restart(RestartMode::Warm),
        Err(RuntimeError::ExecutionTimeout)
    );
    deadline.0.set(false);
    assert_eq!(state.storage().generation(), Generation::FIRST);
    assert_eq!(state.read_reference(&held[0]).unwrap(), Value::Int(1));
    state
        .write_reference(held[1].as_view(), Value::Int(5))
        .unwrap();
    assert_eq!(state.read_reference(&held[1]).unwrap(), Value::Int(5));
    assert!(state.storage().read_by_ref_ref(&held[2]).is_some());
}

#[cfg(feature = "std")]
mod hosted {
    use super::*;
    use std::sync::Arc;
    use trust_runtime_core::vm::{LogicalExecutionServices, OwnedRuntimeState};

    fn owned_handles(state: &mut OwnedRuntimeState<LogicalExecutionServices>) -> [ValueRef; 3] {
        let global = state.storage().ref_for_global("probe_depth").unwrap();
        let Some(Value::Instance(plant)) = state.storage().get_global("Plant") else {
            panic!("program root")
        };
        let member = state
            .storage()
            .ref_for_instance(*plant, "depth_result")
            .unwrap();
        let output = IoAddress::parse("%QX0.0").unwrap();
        let index = state
            .prepared()
            .io_bindings()
            .iter()
            .find(|binding| binding.address == output)
            .map(|binding| binding.ref_idx)
            .expect("the fixture binds %QX0.0");
        let image = state.resolve_persistent_reference(index).unwrap();
        [global, member, image]
    }

    #[test]
    fn a_replacement_continues_the_lineage_and_refuses_earlier_handles() {
        let prepared =
            Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap());
        let mut state = Arc::clone(&prepared).instantiate_owned(0).unwrap();
        state.restart(RestartMode::Warm).unwrap();
        let held = owned_handles(&mut state);
        assert_eq!(held[0].generation, Generation(1));

        let replacement =
            Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap());
        let (mut candidate, _) = state
            .prepare_replacement(replacement, 0, LogicalExecutionServices)
            .unwrap();
        assert_eq!(candidate.storage().generation(), Generation(2));
        // Preparing a candidate changes nothing installed: the held handles still work.
        assert_eq!(state.read_reference(&held[0]).unwrap(), Value::Int(1));

        for reference in &held {
            assert_eq!(candidate.storage().read_by_ref_ref(reference), None);
            assert_eq!(
                candidate.read_reference(reference),
                Err(RuntimeError::ReferenceLifetime)
            );
            assert_eq!(
                candidate.write_reference(reference.as_view(), Value::Int(7)),
                Err(RuntimeError::ReferenceLifetime)
            );
        }
        assert_eq!(
            candidate.storage().get_global("probe_depth"),
            Some(&Value::Int(1))
        );
        let current = owned_handles(&mut candidate);
        candidate
            .write_reference(current[0].as_view(), Value::Int(3))
            .unwrap();
        assert_eq!(
            candidate.read_reference(&current[0]).unwrap(),
            Value::Int(3)
        );
    }

    #[test]
    fn a_resource_only_reconfiguration_keeps_the_generation_and_every_handle() {
        let prepared =
            Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap());
        let mut state = Arc::clone(&prepared).instantiate_owned(0).unwrap();
        state.execute_cycle(Duration::from_millis(10)).unwrap();
        let held = owned_handles(&mut state);
        state
            .prepare_resource_reconfiguration(Arc::clone(&prepared), 0, false)
            .unwrap()
            .commit()
            .unwrap();
        assert_eq!(state.storage().generation(), Generation::FIRST);
        assert_eq!(state.read_reference(&held[0]).unwrap(), Value::Int(1));
        state
            .write_reference(held[1].as_view(), Value::Int(4))
            .unwrap();
        assert_eq!(state.read_reference(&held[1]).unwrap(), Value::Int(4));
        assert!(state.storage().read_by_ref_ref(&held[2]).is_some());
    }
}
