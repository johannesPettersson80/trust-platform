//! Real saved-artifact cycles with checked, borrowed host adapter boundaries.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use trust_runtime_core::{
    bytecode::{BytecodeModule, SectionData, SectionId},
    error::RuntimeError,
    io_address::IoAddress,
    memory::{MemoryLocation, VariableStorage},
    retain::RetainSnapshot,
    value::{DateTimeValue, Duration, RefSegment, Value, ValueRefView},
    vm::{
        CycleAccess, CycleHooks, CyclePhase, ExecutionServices, PreparationLimits, PreparedModule,
    },
};

const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

fn field(storage: &VariableStorage, name: &str) -> Value {
    let Some(Value::Instance(instance)) = storage.get_global("Plant") else {
        panic!("admitted root")
    };
    storage.get_instance_var(*instance, name).unwrap().clone()
}

struct RetainObserver(Rc<RefCell<Vec<&'static str>>>);
impl ExecutionServices for RetainObserver {
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
    fn save_retain_snapshot(&self, _: &RetainSnapshot) -> Result<(), RuntimeError> {
        self.0.borrow_mut().push("retain");
        Ok(())
    }
}

#[test]
fn program_observation_finishes_even_when_execution_deadline_faults() {
    struct Services(Rc<Cell<bool>>);
    impl ExecutionServices for Services {
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
    struct Hooks {
        expired: Rc<Cell<bool>>,
        observations: Vec<(&'static str, smol_str::SmolStr)>,
    }
    impl CycleHooks for Hooks {
        fn has_program_hooks(&self) -> bool {
            true
        }
        fn program_started(&mut self, name: &smol_str::SmolStr) {
            self.observations.push(("start", name.clone()));
            self.expired.set(true);
        }
        fn program_finished(&mut self, name: &smol_str::SmolStr) {
            self.observations.push(("finish", name.clone()));
        }
    }
    let expired = Rc::new(Cell::new(false));
    let services = Services(Rc::clone(&expired));
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate_with_services(0, &services).unwrap();
    let mut hooks = Hooks {
        expired,
        observations: Vec::new(),
    };
    assert_eq!(
        state.execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks),
        Err(RuntimeError::ExecutionTimeout)
    );
    assert_eq!(
        hooks.observations,
        [("start", "Plant".into()), ("finish", "Plant".into())]
    );
}

#[test]
fn borrowed_adapters_preserve_input_protocol_retain_output_force_phase_order() {
    struct Hooks(Rc<RefCell<Vec<&'static str>>>);
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            let name = match phase {
                CyclePhase::CycleStart => {
                    access.write_named_path("probe_depth", &[], Value::Int(2))?;
                    "start"
                }
                CyclePhase::BeforeInputSample => {
                    access.inputs_mut()?[0] = 1;
                    "physical-inputs"
                }
                CyclePhase::AfterInputSample => {
                    assert_eq!(field(access.storage(), "button"), Value::Bool(true));
                    access.write_named_path(
                        "Plant",
                        &[RefSegment::Field("button".into())],
                        Value::Bool(false),
                    )?;
                    "protocol-inputs"
                }
                CyclePhase::TaskStart(0) => "task-start",
                CyclePhase::TaskEnd(0) => {
                    assert_eq!(field(access.storage(), "led"), Value::Bool(false));
                    assert_eq!(field(access.storage(), "depth_result"), Value::Int(2));
                    "task-end"
                }
                CyclePhase::BeforeOutputSample => "before-output",
                CyclePhase::AfterOutputSample => {
                    assert_eq!(access.outputs()[0] & 1, 0);
                    // A storage force is deliberately later than mapped sampling.
                    access.write_named_path(
                        "Plant",
                        &[RefSegment::Field("led".into())],
                        Value::Bool(true),
                    )?;
                    assert_eq!(
                        access.outputs()[0] & 1,
                        0,
                        "a force must not resample normal output bindings"
                    );
                    access.write_image(&IoAddress::parse("%QX0.0")?, Value::Bool(true))?;
                    assert_eq!(access.outputs()[0] & 1, 1);
                    "output-forces-capture-drivers"
                }
                CyclePhase::CycleEnd => "end",
                other => panic!("unexpected phase {other:?}"),
            };
            self.0.borrow_mut().push(name);
            Ok(())
        }
    }
    let trace = Rc::new(RefCell::new(Vec::new()));
    let services = RetainObserver(Rc::clone(&trace));
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate_with_services(0, &services).unwrap();
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut Hooks(Rc::clone(&trace)))
        .unwrap();
    assert_eq!(
        trace.borrow().as_slice(),
        [
            "start",
            "physical-inputs",
            "protocol-inputs",
            "task-start",
            "task-end",
            "before-output",
            "retain",
            "output-forces-capture-drivers",
            "end"
        ]
    );
    assert_eq!(state.outputs()[0] & 1, 1);
    assert_eq!(field(state.storage(), "led"), Value::Bool(true));
    assert!(state.fault().is_none());
}

#[test]
fn rejected_management_requests_leave_the_owner_executable_and_destination_unchanged() {
    struct Hooks {
        rejections: usize,
    }
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::CycleStart {
                assert!(access
                    .write_named_path("probe_depth", &[], Value::Bool(false))
                    .is_err());
                self.rejections += 1;
                assert_eq!(
                    access.storage().get_global("probe_depth"),
                    Some(&Value::Int(1))
                );
                assert!(access
                    .write_reference(
                        ValueRefView {
                            location: MemoryLocation::Global,
                            offset: usize::MAX,
                            path: &[]
                        },
                        Value::Int(7)
                    )
                    .is_err());
                self.rejections += 1;
            }
            Ok(()) // Report request rejection without returning a PLC failure.
        }
    }
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut hooks = Hooks { rejections: 0 };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .unwrap();
    assert_eq!(hooks.rejections, 2);
    assert!(state.fault().is_none());
    assert_eq!(
        state.storage().get_global("probe_depth"),
        Some(&Value::Int(1))
    );
    assert_eq!(state.task_states()[0].last_run, Duration::from_millis(10));
}

fn extra_output_bindings() -> BytecodeModule {
    let mut module = BytecodeModule::decode(GPIO).unwrap();
    let Some(SectionData::IoMap(io)) = module.section(SectionId::IoMap) else {
        panic!("I/O metadata")
    };
    let output = io
        .bindings
        .iter()
        .find(|binding| {
            let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable)
            else {
                panic!("strings")
            };
            strings.entries[binding.address_str_idx as usize].starts_with("%Q")
        })
        .unwrap()
        .clone();
    let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable) else {
        panic!("strings")
    };
    let marker = strings.entries.len() as u32;
    strings.entries.push("%MX0.0".into());
    let hierarchical = strings.entries.len() as u32;
    strings.entries.push("%QX3.4.0".into());
    let Some(SectionData::IoMap(io)) = module.section_mut(SectionId::IoMap) else {
        panic!("I/O metadata")
    };
    io.bindings.push(trust_runtime_core::bytecode::IoBinding {
        address_str_idx: marker,
        ..output.clone()
    });
    io.bindings.push(trust_runtime_core::bytecode::IoBinding {
        address_str_idx: hierarchical,
        ..output
    });
    let Some(SectionData::ResourceMeta(meta)) = module.section_mut(SectionId::ResourceMeta) else {
        panic!("resource metadata")
    };
    meta.resources[0].memory_size = meta.resources[0].memory_size.max(1);
    module
}

#[test]
fn post_output_adapter_failure_rolls_back_flat_q_without_changing_host_marker_hierarchy_domain() {
    struct Hooks;
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::BeforeInputSample {
                access.inputs_mut()?[0] = 1;
            }
            if phase == CyclePhase::AfterOutputSample {
                assert_eq!(access.outputs()[0] & 1, 1);
                access.write_image(&IoAddress::parse("%QX0.0")?, Value::Bool(false))?;
                access.write_image(&IoAddress::parse("%MX0.0")?, Value::Bool(true))?;
                access.write_image(&IoAddress::parse("%QX3.4.0")?, Value::Bool(true))?;
                return Err(RuntimeError::IoDriver(
                    "injected output driver failure".into(),
                ));
            }
            Ok(())
        }
    }
    let module = extra_output_bindings();
    let prepared = PreparedModule::from_decoded(&module, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let previous = state.outputs()[0];
    assert!(matches!(
        state.execute_cycle_with_hooks(Duration::from_millis(10), &mut Hooks),
        Err(RuntimeError::IoDriver(_))
    ));
    assert_eq!(
        state.outputs()[0],
        previous,
        "flat Q restores the pre-cycle published bytes"
    );
    assert_eq!(
        state.memory_image()[0] & 1,
        1,
        "marker effects retain old hosted behavior"
    );
    assert_eq!(
        state.read_hierarchical_output(&IoAddress::parse("%QX3.4.0").unwrap()),
        Some(&Value::Bool(true)),
        "hierarchical effects are not silently added to flat Q rollback"
    );
    assert!(state.fault().is_some());
    assert_eq!(
        state.execute_cycle(Duration::from_millis(20)),
        Err(RuntimeError::ResourceFaulted)
    );
}

#[test]
fn pre_output_adapter_failure_cannot_publish_normal_outputs() {
    struct Hooks;
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::BeforeInputSample {
                access.inputs_mut()?[0] = 1;
            }
            if phase == CyclePhase::BeforeOutputSample {
                return Err(RuntimeError::RetainStore(
                    "injected host retain failure".into(),
                ));
            }
            Ok(())
        }
    }
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let previous = state.outputs().to_vec();
    assert!(matches!(
        state.execute_cycle_with_hooks(Duration::from_millis(10), &mut Hooks),
        Err(RuntimeError::RetainStore(_))
    ));
    assert_eq!(state.outputs(), previous);
}

#[test]
fn missed_activation_event_precedes_priority_execution_without_replaying_tasks() {
    #[derive(Default)]
    struct Hooks(Vec<CyclePhase>);
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            _: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            self.0.push(phase);
            Ok(())
        }
    }
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut hooks = Hooks::default();
    state
        .execute_cycle_with_hooks(Duration::from_millis(35), &mut hooks)
        .unwrap();
    let overrun = hooks
        .0
        .iter()
        .position(|phase| {
            *phase
                == CyclePhase::TaskOverrun {
                    index: 0,
                    missed: 2,
                }
        })
        .unwrap();
    let start = hooks
        .0
        .iter()
        .position(|phase| *phase == CyclePhase::TaskStart(0))
        .unwrap();
    assert!(overrun < start);
    assert_eq!(
        hooks
            .0
            .iter()
            .filter(|phase| **phase == CyclePhase::TaskStart(0))
            .count(),
        1
    );
    assert_eq!(state.task_states()[0].last_run, Duration::from_millis(30));
    assert_eq!(state.task_states()[0].overrun_count, 2);
}

#[test]
fn deadline_after_output_callback_restores_published_q_and_latches_execution_fault() {
    struct Services(Rc<Cell<bool>>);
    impl ExecutionServices for Services {
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
    struct Hooks(Rc<Cell<bool>>);
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::BeforeInputSample {
                access.inputs_mut()?[0] = 1;
            }
            if phase == CyclePhase::AfterOutputSample {
                assert_eq!(access.outputs()[0] & 1, 1);
                self.0.set(true);
            }
            Ok(())
        }
    }
    let deadline = Rc::new(Cell::new(false));
    let services = Services(Rc::clone(&deadline));
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate_with_services(0, &services).unwrap();
    let previous = state.outputs().to_vec();
    assert_eq!(
        state.execute_cycle_with_hooks(Duration::from_millis(10), &mut Hooks(deadline)),
        Err(RuntimeError::ExecutionTimeout)
    );
    assert_eq!(state.outputs(), previous);
    assert_eq!(state.fault(), Some(&RuntimeError::ExecutionTimeout));
}

#[test]
#[cfg(feature = "std")]
fn owned_and_borrowed_cycle_ports_execute_the_same_installed_artifact() {
    struct Inputs;
    impl CycleHooks for Inputs {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::BeforeInputSample {
                access.inputs_mut()?[0] = 1;
            }
            Ok(())
        }
    }
    let prepared = std::sync::Arc::new(
        PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap(),
    );
    let mut borrowed = prepared.instantiate(0).unwrap();
    let mut owned = std::sync::Arc::clone(&prepared)
        .instantiate_owned(0)
        .unwrap();
    let mut plain = prepared.instantiate(0).unwrap();
    plain.inputs_mut()[0] = 1;
    plain.execute_cycle(Duration::from_millis(10)).unwrap();
    borrowed
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut Inputs)
        .unwrap();
    owned
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut Inputs)
        .unwrap();
    assert_eq!(borrowed.outputs(), plain.outputs());
    assert_eq!(owned.outputs(), plain.outputs());
    assert_eq!(
        field(borrowed.storage(), "led"),
        field(owned.storage(), "led")
    );
    assert_eq!(
        borrowed.task_states()[0].last_run,
        owned.task_states()[0].last_run
    );
}

#[test]
fn successful_physical_submission_commits_q_before_a_late_deadline_fault() {
    struct Services(Rc<Cell<bool>>);
    impl ExecutionServices for Services {
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
    struct Hooks {
        deadline: Rc<Cell<bool>>,
        submitted: Rc<RefCell<Vec<u8>>>,
        fail: bool,
    }
    impl CycleHooks for Hooks {
        fn on_phase(
            &mut self,
            phase: CyclePhase,
            access: &mut CycleAccess<'_, '_>,
        ) -> Result<(), RuntimeError> {
            if phase == CyclePhase::BeforeInputSample {
                access.inputs_mut()?[0] = 1;
            }
            Ok(())
        }
        fn publish_outputs(&mut self, access: &CycleAccess<'_, '_>) -> Result<(), RuntimeError> {
            self.submitted
                .borrow_mut()
                .extend_from_slice(access.outputs());
            self.deadline.set(true);
            if self.fail {
                Err(RuntimeError::IoDriver("injected submission failure".into()))
            } else {
                Ok(())
            }
        }
    }
    // Both failures occur after a driver sees Q=1. Only a successful return
    // commits the logical image; the driver-error case keeps the old rollback.
    for fail in [false, true] {
        let deadline = Rc::new(Cell::new(false));
        let submitted = Rc::new(RefCell::new(Vec::new()));
        let services = Services(Rc::clone(&deadline));
        let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
        let mut state = prepared.instantiate_with_services(0, &services).unwrap();
        let previous = state.outputs().to_vec();
        let error = state
            .execute_cycle_with_hooks(
                Duration::from_millis(10),
                &mut Hooks {
                    deadline,
                    submitted: Rc::clone(&submitted),
                    fail,
                },
            )
            .unwrap_err();
        assert_eq!(submitted.borrow()[0] & 1, 1);
        if fail {
            assert_eq!(
                error,
                RuntimeError::IoDriver("injected submission failure".into())
            );
            assert_eq!(state.outputs(), previous);
        } else {
            assert_eq!(error, RuntimeError::ExecutionTimeout);
            assert_eq!(state.outputs(), submitted.borrow().as_slice());
        }
        assert_eq!(state.fault(), Some(&error));
    }
}
