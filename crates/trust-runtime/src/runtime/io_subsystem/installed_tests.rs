//! Installed drivers and presentation share the admitted engine images.
use super::*;
use crate::io::{IoAddress, IoSnapshotValue};
use crate::value::{Duration, Value};
use trust_runtime_core::vm::{
    CycleHooks, CyclePhase, LogicalExecutionServices, PreparationLimits, PreparedModule,
};

const GPIO: &[u8] = include_bytes!("../../../tests/fixtures/portability/f401/gpio.stbc");
fn state() -> OwnedRuntimeState<LogicalExecutionServices> {
    Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap())
        .instantiate_owned(0)
        .unwrap()
}

#[test]
fn installed_view_reads_the_core_images_and_keeps_only_authoring_binding_schema() {
    let mut state = state();
    let mut io = IoSubsystem::new();
    io.try_resize(128, 128, 128).unwrap();
    io.interface_mut().bind_typed(
        "Button",
        IoAddress::parse("%IX0.0").unwrap(),
        trust_hir::TypeId::BOOL,
    );
    io.interface_mut().bind_typed(
        "Led",
        IoAddress::parse("%QX0.0").unwrap(),
        trust_hir::TypeId::BOOL,
    );
    io.interface_mut()
        .set_binding_sources(|_| Some("reviewed test driver".into()));
    io.interface_mut().inputs_mut()[0] = 0;
    io.freeze_bindings();
    assert!(io.interface().inputs().is_empty());
    assert!(io.interface().outputs().is_empty());
    assert!(io.interface().memory().is_empty());
    state
        .write_image(&IoAddress::parse("%IX0.0").unwrap(), Value::Bool(true))
        .unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let view = io.installed_view(&state);
    assert_eq!(view.inputs().as_ptr(), state.inputs().as_ptr());
    assert_eq!(view.outputs().as_ptr(), state.outputs().as_ptr());
    assert_eq!(view.inputs()[0] & 1, 1);
    assert_eq!(
        view.read(&IoAddress::parse("%QX0.0").unwrap()).unwrap(),
        Value::Bool(true)
    );
    let snapshot = view.snapshot();
    assert_eq!(snapshot.inputs[0].name.as_deref(), Some("Button"));
    assert_eq!(
        snapshot.inputs[0].source.as_deref(),
        Some("reviewed test driver")
    );
    assert!(matches!(
        snapshot.inputs[0].value,
        IoSnapshotValue::Value(Value::Bool(true))
    ));
    assert!(matches!(
        snapshot.outputs[0].value,
        IoSnapshotValue::Value(Value::Bool(true))
    ));
}

struct Driver {
    name: &'static str,
    input: u8,
    fail_read: bool,
    fail_write: bool,
    events: Arc<Mutex<Vec<String>>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}
impl IoDriver for Driver {
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        self.events
            .lock()
            .unwrap()
            .push(format!("{} input", self.name));
        if self.fail_read {
            return Err(RuntimeError::IoDriver("read failed".into()));
        }
        image[0] = self.input;
        Ok(())
    }
    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        self.events
            .lock()
            .unwrap()
            .push(format!("{} output", self.name));
        self.writes.lock().unwrap().push(image.to_vec());
        if self.fail_write {
            Err(RuntimeError::IoDriver("write failed".into()))
        } else {
            Ok(())
        }
    }
}
struct Hooks<'a>(&'a mut IoSubsystem);
impl CycleHooks for Hooks<'_> {
    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        match phase {
            CyclePhase::BeforeInputSample => self.0.sample_inputs_with_cycle(access),
            CyclePhase::AfterInputSample => {
                let Some(Value::Instance(root)) = access.storage().get_global("Plant") else {
                    panic!("root")
                };
                assert_eq!(
                    access.storage().get_instance_var(*root, "button"),
                    Some(&Value::Bool(true))
                );
                Ok(())
            }
            CyclePhase::AfterOutputSample => self.0.publish_outputs_with_cycle(access),
            _ => Ok(()),
        }
    }
}

#[test]
fn installed_drivers_fill_and_publish_the_same_images_in_registration_order() {
    let mut state = state();
    let mut io = IoSubsystem::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let writes = Arc::new(Mutex::new(Vec::new()));
    for (name, input) in [("First", 0), ("Second", 1)] {
        io.add_driver(
            name,
            Box::new(Driver {
                name,
                input,
                fail_read: false,
                fail_write: false,
                events: events.clone(),
                writes: writes.clone(),
            }),
        );
    }
    io.freeze_bindings();
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut Hooks(&mut io))
        .unwrap();
    assert_eq!(state.inputs()[0], 1);
    assert_eq!(state.outputs()[0] & 1, 1);
    assert_eq!(
        *events.lock().unwrap(),
        [
            "First input",
            "Second input",
            "First output",
            "Second output"
        ]
    );
    assert_eq!(*writes.lock().unwrap(), [vec![1], vec![1]]);
    assert!(io.interface().inputs().is_empty());
    assert!(io.interface().outputs().is_empty());
}

#[test]
fn normal_output_failure_restores_q_then_safe_state_attempts_all_drivers_from_core_image() {
    let mut state = state();
    let mut io = IoSubsystem::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let writes = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::new(Mutex::new(Vec::new()));
    for (name, fail_write) in [("First", true), ("Second", false)] {
        io.add_driver(
            name,
            Box::new(Driver {
                name,
                input: 1,
                fail_read: false,
                fail_write,
                events: events.clone(),
                writes: writes.clone(),
            }),
        );
    }
    io.set_health_sink(Some(sink.clone()));
    io.set_safe_state(IoSafeState {
        outputs: vec![(IoAddress::parse("%QX0.2").unwrap(), Value::Bool(true))],
    });
    io.freeze_bindings();
    assert!(matches!(
        state.execute_cycle_with_hooks(Duration::from_millis(10), &mut Hooks(&mut io)),
        Err(RuntimeError::IoDriver(_))
    ));
    assert_eq!(
        state.outputs(),
        &[0],
        "flat Q rolls back after the normal driver failure"
    );
    assert_eq!(
        *events.lock().unwrap(),
        ["First input", "Second input", "First output"]
    );
    let failure = io.apply_safe_state_installed(&mut state).unwrap_err();
    assert!(failure.to_string().contains("driver 'First'"));
    assert_eq!(
        *events.lock().unwrap(),
        [
            "First input",
            "Second input",
            "First output",
            "First output",
            "Second output"
        ]
    );
    assert_eq!(*writes.lock().unwrap(), [vec![1], vec![4], vec![4]]);
    assert_eq!(state.outputs(), &[4]);
    assert!(
        state.fault().is_some(),
        "safe-state output writes do not clear the execution fault"
    );
    assert_eq!(sink.lock().unwrap().len(), 2);
}

#[test]
fn physical_input_failure_stops_at_first_driver_without_normal_output_publication() {
    let mut state = state();
    let mut io = IoSubsystem::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let writes = Arc::new(Mutex::new(Vec::new()));
    for (name, fail_read) in [("First", true), ("Second", false)] {
        io.add_driver(
            name,
            Box::new(Driver {
                name,
                input: 1,
                fail_read,
                fail_write: false,
                events: events.clone(),
                writes: writes.clone(),
            }),
        );
    }
    assert!(matches!(
        state.execute_cycle_with_hooks(Duration::from_millis(10), &mut Hooks(&mut io)),
        Err(RuntimeError::IoDriver(_))
    ));
    assert_eq!(*events.lock().unwrap(), ["First input"]);
    assert!(writes.lock().unwrap().is_empty());
    assert_eq!(state.outputs(), &[0]);
}

#[test]
fn explicit_owner_sampling_changes_only_inputs_until_a_shared_cycle_latches_them() {
    let mut state = state();
    let mut io = IoSubsystem::new();
    io.add_driver(
        "Input",
        Box::new(Driver {
            name: "Input",
            input: 1,
            fail_read: false,
            fail_write: false,
            events: Arc::new(Mutex::new(Vec::new())),
            writes: Arc::new(Mutex::new(Vec::new())),
        }),
    );
    io.freeze_bindings();
    let last_run = state.task_states()[0].last_run;
    io.sample_inputs(&mut state).unwrap();
    assert_eq!(state.inputs()[0], 1);
    assert_eq!(state.outputs(), &[0]);
    assert_eq!(state.task_states()[0].last_run, last_run);
    let Some(Value::Instance(root)) = state.storage().get_global("Plant") else {
        panic!("root")
    };
    assert_eq!(
        state.storage().get_instance_var(*root, "button"),
        Some(&Value::Bool(false))
    );
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert_eq!(state.outputs(), &[1]);
}

#[test]
fn installed_snapshot_uses_prepared_alias_enum_and_real_codecs_without_hir_index_casts() {
    use trust_runtime_core::{
        bytecode::{
            BytecodeModule, EnumVariant, SectionData, SectionId, TypeData, TypeEntry, TypeKind,
            TypeTable,
        },
        io_image::IoValueCodec,
    };
    let mut module = BytecodeModule::decode(GPIO).unwrap();
    let Some(SectionData::ResourceMeta(meta)) = module.section_mut(SectionId::ResourceMeta) else {
        panic!("resources")
    };
    meta.resources[0].memory_size = 8;
    let prepared =
        Arc::new(PreparedModule::from_decoded(&module, PreparationLimits::default()).unwrap());
    let mut state = prepared.instantiate_owned(0).unwrap();
    let reference = state.storage().ref_for_global("probe_depth").unwrap();
    let mut io = IoSubsystem::new();
    let real = IoAddress::parse("%MD0").unwrap();
    let enumeration = IoAddress::parse("%MW4").unwrap();
    // Exercise the actual source-free alias/enum resolution, independent of
    // optional authoring HIR identities used only for hosted presentation.
    let types = TypeTable {
        offsets: Vec::new(),
        entries: vec![
            TypeEntry {
                kind: TypeKind::Primitive,
                name_idx: None,
                data: TypeData::Primitive {
                    prim_id: 14,
                    max_length: 0,
                },
            },
            TypeEntry {
                kind: TypeKind::Alias,
                name_idx: None,
                data: TypeData::Alias { target_type_id: 0 },
            },
            TypeEntry {
                kind: TypeKind::Primitive,
                name_idx: None,
                data: TypeData::Primitive {
                    prim_id: 7,
                    max_length: 0,
                },
            },
            TypeEntry {
                kind: TypeKind::Enum,
                name_idx: Some(0),
                data: TypeData::Enum {
                    base_type_id: 2,
                    variants: vec![EnumVariant {
                        name_idx: 1,
                        value: 2,
                    }],
                },
            },
        ],
    };
    let strings = vec!["OperatingMode".into(), "Auto".into()];
    io.interface_mut().bind_prepared_ref(
        reference.clone(),
        real.clone(),
        IoValueCodec::from_type(Some(1), &types, &strings).unwrap(),
        None,
        Some("SpeedAlias".into()),
    );
    io.interface_mut().bind_prepared_ref(
        reference,
        enumeration.clone(),
        IoValueCodec::from_type(Some(3), &types, &strings).unwrap(),
        None,
        Some("Mode".into()),
    );
    io.freeze_bindings();
    state
        .write_image(&real, Value::DWord(1.5f32.to_bits()))
        .unwrap();
    state.write_image(&enumeration, Value::Word(2)).unwrap();
    let snapshot = io.installed_view(&state).snapshot();
    assert_eq!(snapshot.memory[0].name.as_deref(), Some("SpeedAlias"));
    assert!(
        snapshot.memory[0].value_type.is_none(),
        "artifact codec does not fabricate a HIR identity"
    );
    assert!(
        matches!(snapshot.memory[0].value, IoSnapshotValue::Value(Value::Real(value)) if value == 1.5)
    );
    assert_eq!(
        snapshot.memory[1].value_type_name.as_deref(),
        Some("OperatingMode")
    );
    let IoSnapshotValue::Value(Value::Enum(value)) = &snapshot.memory[1].value else {
        panic!("enum snapshot")
    };
    assert_eq!(value.type_name(), "OperatingMode");
    assert_eq!(value.variant_name(), "Auto");
}

#[test]
fn installed_empty_safe_state_only_refreshes_health_and_never_writes_outputs() {
    let mut state = state();
    let mut io = IoSubsystem::new();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::new(Mutex::new(Vec::new()));
    io.add_driver(
        "Driver",
        Box::new(Driver {
            name: "Driver",
            input: 1,
            fail_read: false,
            fail_write: false,
            events: Arc::new(Mutex::new(Vec::new())),
            writes: writes.clone(),
        }),
    );
    io.set_health_sink(Some(sink.clone()));
    io.freeze_bindings();
    io.apply_safe_state_installed(&mut state).unwrap();
    assert!(writes.lock().unwrap().is_empty());
    assert_eq!(state.outputs(), &[0]);
    assert_eq!(sink.lock().unwrap().len(), 1);
}

#[test]
fn installed_safe_state_reports_unconfirmed_health_and_still_attempts_later_drivers() {
    let mut state = state();
    let mut io = IoSubsystem::new();
    let first_writes = Arc::new(Mutex::new(Vec::new()));
    let second_writes = Arc::new(Mutex::new(Vec::new()));
    struct HealthDriver {
        health: IoDriverHealth,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }
    impl IoDriver for HealthDriver {
        fn read_inputs(&mut self, _: &mut [u8]) -> Result<(), RuntimeError> {
            Ok(())
        }
        fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
            self.writes.lock().unwrap().push(outputs.to_vec());
            Ok(())
        }
        fn health(&self) -> IoDriverHealth {
            self.health.clone()
        }
    }
    io.add_driver(
        "Degraded",
        Box::new(HealthDriver {
            health: IoDriverHealth::Degraded {
                error: "not confirmed".into(),
            },
            writes: first_writes.clone(),
        }),
    );
    io.add_driver(
        "Later",
        Box::new(HealthDriver {
            health: IoDriverHealth::Ok,
            writes: second_writes.clone(),
        }),
    );
    io.set_safe_state(IoSafeState {
        outputs: vec![(IoAddress::parse("%QX0.2").unwrap(), Value::Bool(true))],
    });
    io.freeze_bindings();
    let error = io.apply_safe_state_installed(&mut state).unwrap_err();
    assert!(error
        .to_string()
        .contains("unconfirmed for driver 'Degraded'"));
    assert_eq!(*first_writes.lock().unwrap(), [vec![4]]);
    assert_eq!(*second_writes.lock().unwrap(), [vec![4]]);
}

#[test]
fn installed_hierarchical_view_reads_admitted_core_input_without_authoring_image_storage() {
    use trust_runtime_core::bytecode::{BytecodeModule, IoBinding, SectionData, SectionId};
    let mut module = BytecodeModule::decode(GPIO).unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let Some(SectionData::IoMap(map)) = module.section(SectionId::IoMap) else {
        panic!("bindings")
    };
    let input = map
        .bindings
        .iter()
        .find(|binding| strings.entries[binding.address_str_idx as usize].starts_with("%I"))
        .unwrap()
        .clone();
    let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable) else {
        panic!("strings")
    };
    let address_str_idx = strings.entries.len() as u32;
    strings.entries.push("%IX3.4.0".into());
    let Some(SectionData::IoMap(map)) = module.section_mut(SectionId::IoMap) else {
        panic!("bindings")
    };
    map.bindings.push(IoBinding {
        address_str_idx,
        ..input
    });
    let prepared =
        Arc::new(PreparedModule::from_decoded(&module, PreparationLimits::default()).unwrap());
    let mut state = prepared.clone().instantiate_owned(0).unwrap();
    let address = IoAddress::parse("%IX3.4.0").unwrap();
    let binding = prepared
        .io_bindings()
        .iter()
        .find(|binding| binding.address == address)
        .unwrap();
    let reference = state.resolve_persistent_reference(binding.ref_idx).unwrap();
    let mut io = IoSubsystem::new();
    io.interface_mut().bind_prepared_ref(
        reference,
        address.clone(),
        binding.codec.clone(),
        None,
        Some("HierarchicalButton".into()),
    );
    io.freeze_bindings();
    state.write_image(&address, Value::Bool(true)).unwrap();
    let view = io.installed_view(&state);
    assert_eq!(view.read(&address).unwrap(), Value::Bool(true));
    assert!(
        io.interface().read(&address).is_err(),
        "authoring object owns no second hierarchical state"
    );
    let snapshot = view.snapshot();
    assert_eq!(
        snapshot.inputs[0].name.as_deref(),
        Some("HierarchicalButton")
    );
    assert!(matches!(
        snapshot.inputs[0].value,
        IoSnapshotValue::Value(Value::Bool(true))
    ));
}
