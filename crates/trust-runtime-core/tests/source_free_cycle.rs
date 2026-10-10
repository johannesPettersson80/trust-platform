//! Cycle ordering from saved bytes, without source authoring or HIR.
use std::cell::RefCell;
use trust_runtime_core::bytecode::{BytecodeModule, SectionData, SectionId};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule};

const ARTIFACT: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

fn background_module() -> BytecodeModule {
    let mut module = BytecodeModule::decode(ARTIFACT).unwrap();
    let Some(SectionData::ResourceMeta(meta)) = module.section_mut(SectionId::ResourceMeta) else {
        panic!("resource metadata")
    };
    meta.resources[0].tasks.clear();
    module
}

#[derive(Default)]
struct Store {
    snapshots: RefCell<Vec<RetainSnapshot>>,
    fail: bool,
}
impl ExecutionServices for Store {
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
    fn has_retain_store(&self) -> bool {
        true
    }
    fn save_retain_snapshot(&self, snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        self.snapshots.borrow_mut().push(snapshot.clone());
        if self.fail {
            Err(RuntimeError::RetainStore("injected storage failure".into()))
        } else {
            Ok(())
        }
    }
}

#[test]
fn background_runs_every_cycle_and_retain_observes_completed_program() {
    let module = background_module();
    let prepared = PreparedModule::from_decoded(&module, PreparationLimits::default()).unwrap();
    let store = Store::default();
    let mut state = prepared.instantiate_with_services(0, &store).unwrap();
    for sample in 1..=3 {
        state
            .execute_cycle(Duration::from_millis(sample * 10))
            .unwrap();
        let Some(Value::Instance(instance)) = state.storage().get_global("Plant") else {
            panic!("program root")
        };
        assert_eq!(
            state.storage().get_instance_var(*instance, "activations"),
            Some(&Value::DInt(sample as i32))
        );
        assert_eq!(store.snapshots.borrow().len(), sample as usize);
        assert_eq!(
            store
                .snapshots
                .borrow()
                .last()
                .unwrap()
                .values()
                .get("@program/Plant/activations"),
            Some(&Value::DInt(sample as i32))
        );
    }
}

#[test]
fn failed_retain_latches_fault_and_stops_subsequent_cycles() {
    let module = background_module();
    let prepared = PreparedModule::from_decoded(&module, PreparationLimits::default()).unwrap();
    let store = Store {
        fail: true,
        ..Default::default()
    };
    let mut state = prepared.instantiate_with_services(0, &store).unwrap();
    let outputs = state.outputs().to_vec();
    assert!(matches!(
        state.execute_cycle(Duration::from_millis(10)),
        Err(RuntimeError::RetainStore(_))
    ));
    assert_eq!(state.outputs(), outputs);
    assert_eq!(
        state.execute_cycle(Duration::from_millis(20)),
        Err(RuntimeError::ResourceFaulted)
    );
    assert_eq!(store.snapshots.borrow().len(), 1);
}

#[test]
fn configured_store_without_save_implementation_fails_closed() {
    struct MissingStore;
    impl ExecutionServices for MissingStore {
        fn deadline_exceeded(&self) -> bool {
            false
        }
        fn has_wall_clock(&self) -> bool {
            false
        }
        fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
            Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
        }
        fn has_retain_store(&self) -> bool {
            true
        }
    }
    let module = background_module();
    let prepared = PreparedModule::from_decoded(&module, PreparationLimits::default()).unwrap();
    let mut state = prepared
        .instantiate_with_services(0, &MissingStore)
        .unwrap();
    assert!(matches!(
        state.execute_cycle(Duration::from_millis(10)),
        Err(RuntimeError::RetainStore(_))
    ));
}
