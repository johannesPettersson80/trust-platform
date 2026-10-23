//! Cycle-service engineering writes return their structured rejection causes.
use trust_runtime_core::{
    bytecode::{BytecodeModule, SectionData, SectionId, StorageOwner},
    error::RuntimeError,
    io_address::IoAddress,
    memory::{FrameId, InstanceId, MemoryLocation},
    value::{Duration, Value, ValueRefView},
    vm::{CycleAccess, CycleHooks, CyclePhase, PreparationLimits, PreparedModule},
};

const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

fn constant_module() -> BytecodeModule {
    let mut module = BytecodeModule::decode(GPIO).unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let name = strings
        .entries
        .iter()
        .position(|name| name == "probe_depth")
        .unwrap() as u32;
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        panic!("layout")
    };
    layout
        .entries
        .iter_mut()
        .find(|entry| entry.owner == StorageOwner::Global && entry.name_idx == name)
        .unwrap()
        .flags |= 1;
    module
}

/// Spec 34 §4.3.1: a type-rejected cycle-service write returns TypeMismatch, point-locally.
#[test]
fn cycle_port_type_mismatch_returns_type_mismatch_and_keeps_destination() {
    struct Hooks {
        writes: Vec<Result<(), RuntimeError>>,
    }
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::CycleStart {
                self.writes
                    .push(access.write_named_path("probe_depth", &[], Value::Bool(false)));
                let reference = access.storage().ref_for_global("probe_depth").unwrap();
                self.writes
                    .push(access.write_reference(reference.as_view(), Value::Bool(false)));
            }
            Ok(()) // Rejected requests are reported here, not returned as PLC failures.
        }
    }
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut hooks = Hooks { writes: Vec::new() };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .unwrap();
    assert_eq!(
        hooks.writes,
        [
            Err(RuntimeError::TypeMismatch),
            Err(RuntimeError::TypeMismatch)
        ]
    );
    assert_eq!(
        state.storage().get_global("probe_depth"),
        Some(&Value::Int(1))
    );
    assert_eq!(state.task_states()[0].last_run, Duration::from_millis(10));
    assert!(state.fault().is_none());
}

/// Spec 34 §4.3.1: a constant destination returns ConstantWrite through each cycle port.
#[test]
fn cycle_port_constant_destination_returns_constant_write() {
    struct Hooks {
        writes: Vec<Result<(), RuntimeError>>,
    }
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::CycleStart {
                self.writes
                    .push(access.write_named_path("probe_depth", &[], Value::Int(3)));
                let reference = access.storage().ref_for_global("probe_depth").unwrap();
                self.writes
                    .push(access.write_reference(reference.as_view(), Value::Int(3)));
            }
            Ok(())
        }
    }
    let module = constant_module();
    let prepared = PreparedModule::from_decoded(&module, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut hooks = Hooks { writes: Vec::new() };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .unwrap();
    assert_eq!(
        hooks.writes,
        [
            Err(RuntimeError::ConstantWrite),
            Err(RuntimeError::ConstantWrite)
        ]
    );
    assert_eq!(
        state.storage().get_global("probe_depth"),
        Some(&Value::Int(1))
    );
    assert_eq!(state.task_states()[0].last_run, Duration::from_millis(10));
    assert!(state.fault().is_none());
}

/// Spec 34 §4.3.1: a stale frame or instance reference returns ReferenceLifetime, point-locally.
#[test]
fn cycle_port_stale_reference_returns_reference_lifetime() {
    struct Hooks {
        writes: Vec<Result<(), RuntimeError>>,
    }
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::CycleStart {
                for location in [
                    MemoryLocation::Local(FrameId(u32::MAX)),
                    MemoryLocation::Instance(InstanceId(u32::MAX)),
                ] {
                    self.writes.push(access.write_reference(
                        ValueRefView {
                            location,
                            offset: 0,
                            path: &[],
                        },
                        Value::Int(9),
                    ));
                }
            }
            Ok(())
        }
    }
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut hooks = Hooks { writes: Vec::new() };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .unwrap();
    assert_eq!(
        hooks.writes,
        [
            Err(RuntimeError::ReferenceLifetime),
            Err(RuntimeError::ReferenceLifetime)
        ]
    );
    assert_eq!(
        state.storage().get_global("probe_depth"),
        Some(&Value::Int(1))
    );
    assert_eq!(state.task_states()[0].last_run, Duration::from_millis(10));
    assert!(state.fault().is_none());
}

/// Spec 34 §4.3.1: a destination offset that resolves to no declaration is point-local and returns
/// its structured cause, TypeMismatch (no declaration gives no reference type, policy.rs:341-342).
#[test]
fn cycle_port_unresolvable_reference_offset_returns_type_mismatch() {
    struct Hooks {
        writes: Vec<Result<(), RuntimeError>>,
    }
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::CycleStart {
                self.writes.push(access.write_reference(
                    ValueRefView {
                        location: MemoryLocation::Global,
                        offset: usize::MAX,
                        path: &[],
                    },
                    Value::Int(7),
                ));
            }
            Ok(())
        }
    }
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut hooks = Hooks { writes: Vec::new() };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .unwrap();
    assert_eq!(hooks.writes, [Err(RuntimeError::TypeMismatch)]);
    assert_eq!(
        state.storage().get_global("probe_depth"),
        Some(&Value::Int(1))
    );
    assert_eq!(state.task_states()[0].last_run, Duration::from_millis(10));
    assert!(state.fault().is_none());
}

/// Spec 34 §4.3.1: a process-image address beyond the admitted window returns InvalidIoAddress.
#[test]
fn cycle_port_image_address_past_admitted_window_returns_invalid_io_address() {
    struct Hooks {
        writes: Vec<Result<(), RuntimeError>>,
    }
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::CycleStart {
                self.writes
                    .push(access.write_image(&IoAddress::parse("%QB1").unwrap(), Value::Byte(0)));
            }
            Ok(())
        }
    }
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut hooks = Hooks { writes: Vec::new() };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .unwrap();
    assert_eq!(
        hooks.writes,
        [Err(RuntimeError::InvalidIoAddress(
            "address exceeds admitted process image".into()
        ))]
    );
    assert_eq!(state.outputs(), &[0]);
    assert_eq!(state.task_states()[0].last_run, Duration::from_millis(10));
    assert!(state.fault().is_none());
}

/// Spec 34 §4.3.1: an engineering write to an unknown destination name returns NullReference.
#[test]
fn cycle_port_unknown_destination_name_returns_null_reference() {
    struct Hooks {
        writes: Vec<Result<(), RuntimeError>>,
    }
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::CycleStart {
                self.writes.push(access.write_named_path(
                    "no_such_destination",
                    &[],
                    Value::Int(3),
                ));
            }
            Ok(())
        }
    }
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut hooks = Hooks { writes: Vec::new() };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .unwrap();
    assert_eq!(hooks.writes, [Err(RuntimeError::NullReference)]);
    assert_eq!(
        state.storage().get_global("probe_depth"),
        Some(&Value::Int(1))
    );
    assert_eq!(state.task_states()[0].last_run, Duration::from_millis(10));
    assert!(state.fault().is_none());
}

/// Spec 10 §4.1.2: an unknown VAR_ACCESS name reports runtime_undefined_variable on write.
/// EXPECTED-FAIL: CycleAccess::write_access returns InvalidAlias (vm/engine/access.rs:103).
#[test]
fn cycle_port_unknown_access_name_reports_undefined_variable() {
    struct Hooks {
        writes: Vec<Result<(), RuntimeError>>,
    }
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::CycleStart {
                self.writes
                    .push(access.write_access("no_such_alias", Value::Int(3)));
            }
            Ok(())
        }
    }
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut hooks = Hooks { writes: Vec::new() };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .unwrap();
    assert_eq!(
        hooks.writes,
        [Err(RuntimeError::UndefinedVariable("no_such_alias".into()))]
    );
    assert_eq!(
        state.storage().get_global("probe_depth"),
        Some(&Value::Int(1))
    );
    assert_eq!(state.task_states()[0].last_run, Duration::from_millis(10));
    assert!(state.fault().is_none());
}

/// Spec 34 §4.3.1: replacing a program root is a point-local ProgramRootReplacement rejection.
#[test]
fn cycle_port_program_root_replacement_returns_program_root_replacement() {
    struct Hooks {
        writes: Vec<Result<(), RuntimeError>>,
        root_unchanged: bool,
    }
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::CycleStart {
                let before = access.storage().get_global("Plant").cloned();
                self.writes
                    .push(access.write_named_path("Plant", &[], Value::Null));
                self.root_unchanged = access.storage().get_global("Plant").cloned() == before;
            }
            Ok(())
        }
    }
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut hooks = Hooks {
        writes: Vec::new(),
        root_unchanged: false,
    };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .unwrap();
    assert_eq!(hooks.writes, [Err(RuntimeError::ProgramRootReplacement)]);
    assert!(hooks.root_unchanged);
    assert_eq!(
        state.storage().get_global("probe_depth"),
        Some(&Value::Int(1))
    );
    assert_eq!(state.task_states()[0].last_run, Duration::from_millis(10));
    assert!(state.fault().is_none());
}
