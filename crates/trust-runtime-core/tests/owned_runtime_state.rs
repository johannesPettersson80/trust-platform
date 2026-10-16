//! Hosted state owns its metadata; the borrowed and owned facades share semantics.
#![cfg(feature = "std")]

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use trust_runtime_core::{
    error::RuntimeError,
    memory::VariableStorage,
    retain::RestartMode,
    value::{DateTimeValue, Duration, RefSegment, Value},
    vm::{ExecutionServices, PreparationLimits, PreparedModule},
};

const ARTIFACT: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

fn plant_field(storage: &VariableStorage, name: &str) -> Value {
    let Some(Value::Instance(plant)) = storage.get_global("Plant") else {
        panic!("admitted program root")
    };
    storage.get_instance_var(*plant, name).unwrap().clone()
}

#[test]
fn owned_and_borrowed_facades_preserve_cycle_and_restart_results() {
    let prepared =
        Arc::new(PreparedModule::from_bytes(ARTIFACT, PreparationLimits::default()).unwrap());
    let mut borrowed = prepared.instantiate(0).unwrap();
    let mut owned = Arc::clone(&prepared).instantiate_owned(0).unwrap();
    for ms in (0..=1000).step_by(10) {
        let now = Duration::from_millis(ms);
        borrowed.execute_cycle(now).unwrap();
        owned.execute_cycle(now).unwrap();
        for name in ["activations", "enabled", "result"] {
            assert_eq!(
                plant_field(owned.storage(), name),
                plant_field(borrowed.storage(), name)
            );
        }
        assert_eq!(owned.outputs(), borrowed.outputs());
        assert_eq!(
            owned.task_states()[0].last_run,
            borrowed.task_states()[0].last_run
        );
        assert_eq!(
            owned.task_states()[0].overrun_count,
            borrowed.task_states()[0].overrun_count
        );
    }
    assert_eq!(plant_field(owned.storage(), "activations"), Value::DInt(40));
    for mode in [RestartMode::Warm, RestartMode::Cold] {
        borrowed.restart(mode).unwrap();
        owned.restart(mode).unwrap();
        for name in ["activations", "enabled", "result"] {
            assert_eq!(
                plant_field(owned.storage(), name),
                plant_field(borrowed.storage(), name)
            );
        }
        assert_eq!(owned.outputs(), borrowed.outputs());
        assert_eq!(owned.fault(), borrowed.fault());
    }
}

#[test]
fn metadata_outlives_authoring_scope_and_owned_controller_moves_between_threads() {
    let state = Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap())
        .instantiate_owned(0)
        .unwrap();
    let mut state = std::thread::spawn(move || {
        let mut state = state;
        state.write_global("probe_depth", Value::Int(4)).unwrap();
        state.inputs_mut()[0] = 1;
        state.execute_cycle(Duration::from_millis(10)).unwrap();
        state
    })
    .join()
    .unwrap();
    assert_eq!(plant_field(state.storage(), "depth_result"), Value::Int(4));
    assert_eq!(state.outputs()[0] & 1, 1);
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert_eq!(state.task_states()[0].last_run, Duration::from_millis(20));
}

#[test]
fn instruction_observer_borrows_the_complete_live_call_stack_and_retires_ancestors() {
    use trust_runtime_core::vm::StatementFrame;

    struct Observer {
        prepared: Arc<PreparedModule>,
        deepest: Arc<AtomicUsize>,
        root_only: Arc<AtomicUsize>,
        stack: bool,
    }
    impl ExecutionServices for Observer {
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
            storage: &VariableStorage,
            _: Duration,
            active: StatementFrame<'_>,
        ) {
            let mut previous = None;
            let mut count = 0;
            let mut last = None;
            for frame in active.call_stack() {
                let id = frame.activation().expect("admitted live invocation");
                assert!(storage.execution_frame_is_live(id));
                assert!(previous.is_none_or(|before| before < id.0));
                previous = Some(id.0);
                let name = self.prepared.pou_name(frame.pou_id().unwrap()).unwrap();
                if count == 0 {
                    assert_eq!(name, "Gpio");
                    let Some(Value::Instance(root)) = storage.get_global("Plant") else {
                        panic!("installed program root");
                    };
                    assert_eq!(frame.runtime_instance(), Some(*root));
                } else {
                    assert!(matches!(name, "ProbeOuter" | "ProbeMiddle" | "ProbeLeaf"));
                    assert_eq!(frame.runtime_instance(), None);
                    assert!(
                        !frame.locals().is_empty(),
                        "function return slot is visible"
                    );
                }
                assert_eq!(
                    frame.call_stack().count(),
                    1,
                    "a projected frame is not recursive"
                );
                count += 1;
                last = Some(id);
            }
            assert_eq!(
                last,
                active.activation(),
                "current frame appears exactly once at the end"
            );
            self.deepest.fetch_max(count, Ordering::SeqCst);
            if count == 1 {
                self.root_only.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    let prepared =
        Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap());
    for stack in [true, false] {
        let deepest = Arc::new(AtomicUsize::new(0));
        let root_only = Arc::new(AtomicUsize::new(0));
        let mut state = Arc::clone(&prepared)
            .instantiate_owned_with_services(
                0,
                Observer {
                    prepared: Arc::clone(&prepared),
                    deepest: Arc::clone(&deepest),
                    root_only: Arc::clone(&root_only),
                    stack,
                },
            )
            .unwrap();
        state.write_global("probe_depth", Value::Int(4)).unwrap();
        state.execute_cycle(Duration::from_millis(10)).unwrap();
        assert_eq!(deepest.load(Ordering::SeqCst), 4);
        assert_eq!(state.storage().execution_frame_count(), 0);
        deepest.store(0, Ordering::SeqCst);
        root_only.store(0, Ordering::SeqCst);
        state.write_global("probe_depth", Value::Int(1)).unwrap();
        state.execute_cycle(Duration::from_millis(20)).unwrap();
        assert_eq!(
            deepest.load(Ordering::SeqCst),
            1,
            "retired ancestors do not reappear"
        );
        assert!(root_only.load(Ordering::SeqCst) > 0);
        assert_eq!(state.storage().execution_frame_count(), 0);
    }
}

#[test]
fn fallible_instruction_observer_unwinds_nested_frames_before_latching_a_cycle_fault() {
    use trust_runtime_core::vm::StatementFrame;
    struct Observer {
        stack: bool,
    }
    impl ExecutionServices for Observer {
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
        fn on_statement_with_frame_checked(
            &self,
            _: u32,
            _: usize,
            _: u32,
            _: &VariableStorage,
            _: Duration,
            frame: StatementFrame<'_>,
        ) -> Result<(), RuntimeError> {
            if frame.call_stack().count() == 3 {
                Err(RuntimeError::InvalidExecutionState)
            } else {
                Ok(())
            }
        }
    }
    let prepared =
        Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap());
    for stack in [true, false] {
        let mut state = Arc::clone(&prepared)
            .instantiate_owned_with_services(0, Observer { stack })
            .unwrap();
        state.write_global("probe_depth", Value::Int(4)).unwrap();
        state.inputs_mut()[0] = 1;
        assert_eq!(
            state.execute_cycle(Duration::from_millis(10)),
            Err(RuntimeError::InvalidExecutionState)
        );
        assert_eq!(state.fault(), Some(&RuntimeError::InvalidExecutionState));
        assert_eq!(
            state.storage().execution_frame_count(),
            0,
            "failed observer retires every invocation"
        );
        assert_eq!(
            state.outputs()[0],
            0,
            "failed cycle cannot publish provisional PLC outputs"
        );
        state.restart(RestartMode::Cold).unwrap();
        assert!(state.fault().is_none());
        state.write_global("probe_depth", Value::Int(1)).unwrap();
        state.inputs_mut()[0] = 1;
        state.execute_cycle(Duration::from_millis(20)).unwrap();
        assert_eq!(state.storage().execution_frame_count(), 0);
        assert_eq!(state.outputs()[0] & 1, 1);
    }
}

#[test]
fn shared_prepared_module_has_independent_mutable_controllers_and_rejected_writes() {
    let prepared =
        Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap());
    let mut first = Arc::clone(&prepared).instantiate_owned(0).unwrap();
    let mut second = Arc::clone(&prepared).instantiate_owned(0).unwrap();
    first.write_global("probe_depth", Value::Int(4)).unwrap();
    for state in [&mut first, &mut second] {
        state.execute_cycle(Duration::from_millis(10)).unwrap();
    }
    assert_eq!(plant_field(first.storage(), "depth_result"), Value::Int(4));
    assert_eq!(plant_field(second.storage(), "depth_result"), Value::Int(1));
    let old_value = first.storage().get_global("probe_depth").unwrap().clone();
    let old_root = first.storage().get_global("Plant").unwrap().clone();
    let last_run = first.task_states()[0].last_run;
    let outputs = first.outputs().to_vec();
    assert!(first
        .write_global("probe_depth", Value::String("wrong".into()))
        .is_err());
    assert_eq!(
        first.write_global("Plant", old_root.clone()),
        Err(RuntimeError::ProgramRootReplacement)
    );
    assert_eq!(first.storage().get_global("probe_depth"), Some(&old_value));
    assert_eq!(first.storage().get_global("Plant"), Some(&old_root));
    assert_eq!(first.task_states()[0].last_run, last_run);
    assert_eq!(first.outputs(), outputs);
    assert!(
        first.fault().is_none(),
        "management rejection is not a PLC fault"
    );
    assert!(std::ptr::eq(first.prepared(), second.prepared()));
}

struct Deadline(Arc<AtomicBool>);
impl ExecutionServices for Deadline {
    fn deadline_exceeded(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

#[test]
fn unsuccessful_owned_restart_keeps_the_installed_state_and_readiness() {
    let expired = Arc::new(AtomicBool::new(false));
    let mut state =
        Arc::new(PreparedModule::from_bytes(ARTIFACT, PreparationLimits::default()).unwrap())
            .instantiate_owned_with_services(0, Deadline(Arc::clone(&expired)))
            .unwrap();
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    let activations = plant_field(state.storage(), "activations");
    let last_run = state.task_states()[0].last_run;
    let outputs = state.outputs().to_vec();
    expired.store(true, Ordering::SeqCst);
    assert!(state.restart(RestartMode::Warm).is_err());
    assert_eq!(plant_field(state.storage(), "activations"), activations);
    assert_eq!(state.task_states()[0].last_run, last_run);
    assert_eq!(state.outputs(), outputs);
    assert!(state.fault().is_none());
    expired.store(false, Ordering::SeqCst);
    state.execute_cycle(Duration::from_millis(50)).unwrap();
    assert_eq!(plant_field(state.storage(), "activations"), Value::DInt(2));
}

#[test]
fn checked_program_member_write_preserves_state_and_rejects_invalid_array_paths() {
    let mut state =
        Arc::new(PreparedModule::from_bytes(ARTIFACT, PreparationLimits::default()).unwrap())
            .instantiate_owned(0)
            .unwrap();
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    let last_run = state.task_states()[0].last_run;
    let enabled = [RefSegment::Field("enabled".into())];
    state
        .write_named_path("Plant", &enabled, Value::Bool(false))
        .unwrap();
    assert_eq!(plant_field(state.storage(), "enabled"), Value::Bool(false));
    assert_eq!(plant_field(state.storage(), "activations"), Value::DInt(1));
    assert_eq!(state.task_states()[0].last_run, last_run);
    assert!(state
        .write_named_path("Plant", &enabled, Value::String("invalid".into()))
        .is_err());
    let invalid = [
        RefSegment::Field("counter".into()),
        RefSegment::Field("history".into()),
        RefSegment::Index(vec![4]),
    ];
    assert!(state
        .write_named_path("Plant", &invalid, Value::Int(99))
        .is_err());
    assert_eq!(plant_field(state.storage(), "enabled"), Value::Bool(false));
    assert_eq!(state.task_states()[0].last_run, last_run);
    assert!(state.fault().is_none());
    state.execute_cycle(Duration::from_millis(50)).unwrap();
    assert_eq!(plant_field(state.storage(), "activations"), Value::DInt(2));
}

#[test]
fn instruction_observer_receives_read_only_current_state_during_the_common_dispatch() {
    use trust_runtime_core::bytecode::{
        BytecodeModule, InitializationPhase, SectionData, SectionId, StorageOwner,
    };
    use trust_runtime_core::vm::StatementFrame;

    struct Observer {
        observations: Arc<AtomicUsize>,
        locals_seen: Arc<AtomicUsize>,
        stack: bool,
        method: u32,
        delta_slot: usize,
    }
    impl ExecutionServices for Observer {
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
            pou: u32,
            _: usize,
            _: u32,
            storage: &VariableStorage,
            now: Duration,
            frame: StatementFrame<'_>,
        ) {
            // Count every invocation, including logical-time zero. Filtering by
            // scan time would make the initial constructor-isolation assertion
            // vacuous because candidate construction also executes bytecode.
            self.observations.fetch_add(1, Ordering::SeqCst);
            assert!(matches!(now.as_millis(), 30 | 60));
            assert!(storage.get_global("Plant").is_some());
            assert_eq!(frame.pou_id(), Some(pou));
            if pou == self.method
                && frame.activation().is_some()
                && frame.locals().get(self.delta_slot) == Some(&Value::Int(1))
                && frame
                    .locals()
                    .iter()
                    .any(|value| matches!(value, Value::Reference(Some(_))))
            {
                assert!(frame.runtime_instance().is_some());
                self.locals_seen.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    let raw = BytecodeModule::decode(ARTIFACT).unwrap();
    let Some(SectionData::StringTable(strings)) = raw.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let Some(SectionData::StorageLayout(layout)) = raw.section(SectionId::StorageLayout) else {
        panic!("layout")
    };
    let delta = layout
        .entries
        .iter()
        .find(|entry| {
            entry.owner == StorageOwner::Frame
                && strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("delta")
        })
        .expect("method parameter");
    let method = delta.owner_pou_id.unwrap();
    let delta_slot = delta.slot as usize;
    let Some(SectionData::Initializers(initializers)) = raw.section(SectionId::Initializers) else {
        panic!("initializers")
    };
    assert!(
        initializers.entries.iter().any(|entry| {
            entry.phase == InitializationPhase::Instance
                && entry.owner_pou_id.is_some()
                && entry.code_length > 0
        }),
        "fixture must execute named constructor bytecode"
    );
    let prepared =
        Arc::new(PreparedModule::from_bytes(ARTIFACT, PreparationLimits::default()).unwrap());
    for stack in [false, true] {
        let observations = Arc::new(AtomicUsize::new(0));
        let locals_seen = Arc::new(AtomicUsize::new(0));
        let mut state = Arc::clone(&prepared)
            .instantiate_owned_with_services(
                0,
                Observer {
                    observations: Arc::clone(&observations),
                    locals_seen: Arc::clone(&locals_seen),
                    stack,
                    method,
                    delta_slot,
                },
            )
            .unwrap();
        assert_eq!(
            observations.load(Ordering::SeqCst),
            0,
            "initial constructor cannot publish snapshots"
        );
        state.execute_cycle(Duration::from_millis(30)).unwrap();
        assert!(observations.load(Ordering::SeqCst) > 0);
        assert!(
            locals_seen.load(Ordering::SeqCst) > 0,
            "callback sees active bound parameter and initialized local reference"
        );
        assert_eq!(plant_field(state.storage(), "activations"), Value::DInt(1));
        let before_restart = observations.load(Ordering::SeqCst);
        state.restart(RestartMode::Cold).unwrap();
        assert_eq!(
            observations.load(Ordering::SeqCst),
            before_restart,
            "candidate at the same scan time cannot publish constructor snapshots"
        );
        assert_eq!(plant_field(state.storage(), "activations"), Value::DInt(0));
        // Restart registers the task at the installed logical time (30 ms).
        // Its next nominal deadline is 55 ms, so the 60 ms sample is due.
        state.execute_cycle(Duration::from_millis(60)).unwrap();
        assert!(
            observations.load(Ordering::SeqCst) > before_restart,
            "installed replacement resumes observing"
        );
    }
}

#[test]
fn legacy_instruction_observer_remains_supported_by_frame_hook_default_delegation() {
    struct Observer(Arc<AtomicUsize>);
    impl ExecutionServices for Observer {
        fn deadline_exceeded(&self) -> bool {
            false
        }
        fn has_wall_clock(&self) -> bool {
            false
        }
        fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
            Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
        }
        fn on_statement(&self, _: u32, _: usize, _: u32, storage: &VariableStorage, now: Duration) {
            assert_eq!(now, Duration::from_millis(30));
            assert!(storage.get_global("Plant").is_some());
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let observations = Arc::new(AtomicUsize::new(0));
    let mut state =
        Arc::new(PreparedModule::from_bytes(ARTIFACT, PreparationLimits::default()).unwrap())
            .instantiate_owned_with_services(0, Observer(Arc::clone(&observations)))
            .unwrap();
    assert_eq!(observations.load(Ordering::SeqCst), 0);
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    assert!(
        observations.load(Ordering::SeqCst) > 0,
        "old hook receives dispatcher events through default delegation"
    );
}

#[test]
#[cfg(feature = "register-ir")]
fn saved_numeric_program_preserves_prepared_fallback_without_run_compilation() {
    let artifact =
        include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc");
    let mut state =
        Arc::new(PreparedModule::from_bytes(artifact, PreparationLimits::default()).unwrap())
            .instantiate_owned(0)
            .unwrap();
    state.set_register_profile_enabled(true);
    state.set_tier1_enabled(true);
    let preparation = state.tier1_snapshot();
    for ms in (0..=1000).step_by(10) {
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
    }
    // EXPT (0x4C) deliberately retains the existing whole-POU stack fallback.
    // Its numeric contract still executes, without compiling a plan in RUN.
    let profile = state.register_profile_snapshot();
    assert_eq!(profile.register_programs_executed, 0);
    assert_eq!(profile.register_program_fallbacks, 40);
    assert!(profile
        .fallback_reasons
        .iter()
        .any(|reason| { reason.reason == "unsupported_opcode_0x4C" && reason.count == 40 }));
    assert_eq!(
        state.tier1_snapshot().compile_attempts,
        preparation.compile_attempts
    );
    assert_eq!(
        state.tier1_snapshot().compile_successes,
        preparation.compile_successes
    );
    assert_eq!(plant_field(state.storage(), "activations"), Value::DInt(40));
    assert_eq!(
        plant_field(state.storage(), "exact_power"),
        Value::LReal(8.0)
    );
    assert_eq!(plant_field(state.storage(), "exact_root"), Value::Real(3.0));
    assert_eq!(state.task_states()[0].overrun_count, 0);
    assert!(state.fault().is_none());
}

#[cfg(feature = "register-ir")]
struct ClockServices {
    stack: bool,
}

#[cfg(feature = "register-ir")]
impl ExecutionServices for ClockServices {
    fn debug_active(&self) -> bool {
        self.stack
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        true
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Ok(DateTimeValue::new(1_700_000_000_123))
    }
}

#[cfg(feature = "register-ir")]
#[test]
fn saved_clock_program_uses_prepared_register_plans_without_run_compilation() {
    let artifact =
        include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/clock-v2.stbc");
    let prepared =
        Arc::new(PreparedModule::from_bytes(artifact, PreparationLimits::default()).unwrap());
    let stack_services = ClockServices { stack: true };
    let mut stack = prepared
        .instantiate_with_services(0, &stack_services)
        .unwrap();
    let mut state = Arc::clone(&prepared)
        .instantiate_owned_with_services(0, ClockServices { stack: false })
        .unwrap();
    state.set_register_profile_enabled(true);
    state.set_tier1_enabled(true);
    let preparation = state.tier1_snapshot();
    for ms in (0..=1000).step_by(10) {
        let logical_time = Duration::from_millis(ms);
        state.execute_cycle(logical_time).unwrap();
        stack.execute_cycle(logical_time).unwrap();
        for storage in [state.storage(), stack.storage()] {
            let Some(Value::Instance(clock)) = storage.get_global("Clock") else {
                panic!("clock program root")
            };
            assert_eq!(
                storage.get_instance_var(*clock, "now"),
                Some(&Value::Dt(DateTimeValue::new(1_700_000_000_123)))
            );
            assert_eq!(
                storage.get_instance_var(*clock, "elapsed"),
                Some(&Value::Time(logical_time))
            );
        }
        assert_eq!(state.outputs(), stack.outputs());
    }
    let profile = state.register_profile_snapshot();
    assert_eq!(profile.register_programs_executed, 101);
    assert_eq!(profile.register_program_fallbacks, 0);
    assert!(profile.fallback_reasons.is_empty());
    let running = state.tier1_snapshot();
    assert!(running.cached_blocks > 0);
    assert!(running.block_executions > 0);
    assert_eq!(running.compile_attempts, preparation.compile_attempts);
    assert_eq!(running.compile_successes, preparation.compile_successes);
    assert!(state.fault().is_none());
    assert!(stack.fault().is_none());
}

#[cfg(feature = "register-ir")]
#[test]
fn owned_register_resets_distinguish_counters_selection_and_prepared_code() {
    let artifact =
        include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/clock-v2.stbc");
    let mut state =
        Arc::new(PreparedModule::from_bytes(artifact, PreparationLimits::default()).unwrap())
            .instantiate_owned_with_services(0, ClockServices { stack: false })
            .unwrap();
    state.set_register_profile_enabled(true);
    state.set_tier1_enabled(true);
    for ms in (0..=2000).step_by(10) {
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
    }
    let selected = state.tier1_snapshot().cached_blocks;
    assert!(
        selected > 0,
        "saved application must cross the selection threshold"
    );
    let prepared_entries = state.register_lowering_snapshot().cached_entries;
    assert!(prepared_entries > 0);
    state.reset_register_profile();
    assert_eq!(
        state.register_profile_snapshot().register_programs_executed,
        0
    );
    assert_eq!(state.tier1_snapshot().cached_blocks, selected);
    state.reset_register_lowering_cache();
    assert_eq!(
        state.register_lowering_snapshot().cached_entries,
        prepared_entries
    );
    assert_eq!(state.register_lowering_snapshot().hits, 0);
    assert_eq!(state.tier1_snapshot().cached_blocks, selected);
    state.reset_tier1();
    assert_eq!(state.tier1_snapshot().cached_blocks, 0);
    assert_eq!(state.tier1_snapshot().compile_attempts, 0);
    state.execute_cycle(Duration::from_millis(2030)).unwrap();
    assert_eq!(
        state.tier1_snapshot().cached_blocks,
        0,
        "explicit tier reset restarts hot selection"
    );
    assert_eq!(
        state.tier1_snapshot().compile_attempts,
        0,
        "warm-up cannot compile inside RUN"
    );
    assert!(state.register_lowering_snapshot().hits > 0);
    let Some(Value::Instance(clock)) = state.storage().get_global("Clock") else {
        panic!("clock program root")
    };
    assert_eq!(
        state.storage().get_instance_var(*clock, "now"),
        Some(&Value::Dt(DateTimeValue::new(1_700_000_000_123)))
    );
    assert_eq!(
        state.storage().get_instance_var(*clock, "elapsed"),
        Some(&Value::Time(Duration::from_millis(2030)))
    );
    assert!(state.fault().is_none());
}
