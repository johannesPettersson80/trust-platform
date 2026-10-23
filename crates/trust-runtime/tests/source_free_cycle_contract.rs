//! Source-level regressions for portable cooperative cycle boundaries.
use std::cell::Cell;
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule};

fn prepare(source: &str) -> PreparedModule {
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default()).unwrap()
}

#[test]
fn initially_true_single_does_not_activate_until_a_new_rising_edge() {
    let prepared = prepare(
        r#"
VAR_GLOBAL Trigger : BOOL := TRUE; Runs : DINT; END_VAR
PROGRAM Main
VAR_EXTERNAL Runs : DINT; END_VAR
Runs := Runs + 1;
END_PROGRAM
PROGRAM Toggle
VAR_EXTERNAL Trigger : BOOL; END_VAR
Trigger := NOT Trigger;
END_PROGRAM
CONFIGURATION C
TASK EventTask (SINGLE := Trigger, PRIORITY := 0);
PROGRAM Active WITH EventTask : Main;
PROGRAM Background : Toggle;
END_CONFIGURATION
"#,
    );
    let mut state = prepared.instantiate(0).unwrap();
    for (time, expected) in [(10, 0), (20, 0), (30, 1)] {
        state.execute_cycle(Duration::from_millis(time)).unwrap();
        assert_eq!(
            state.storage().get_global("Runs"),
            Some(&Value::DInt(expected))
        );
    }
}

struct FailingStore {
    calls: Cell<usize>,
}
impl ExecutionServices for FailingStore {
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
        self.calls.set(self.calls.get() + 1);
        Err(RuntimeError::RetainStore("injected save failure".into()))
    }
}

#[test]
fn retain_failure_blocks_a_changed_output_image() {
    let prepared =
        prepare("PROGRAM Main VAR Output AT %QX0.0 : BOOL; END_VAR Output := TRUE; END_PROGRAM");
    let store = FailingStore {
        calls: Cell::new(0),
    };
    let mut state = prepared.instantiate_with_services(0, &store).unwrap();
    assert_eq!(state.outputs(), [0]);
    assert!(matches!(
        state.execute_cycle(Duration::from_millis(10)),
        Err(RuntimeError::RetainStore(_))
    ));
    assert_eq!(
        state.outputs(),
        [0],
        "the program's TRUE value must not be published after save failure"
    );
    assert_eq!(store.calls.get(), 1);
}

#[test]
fn task_execution_precedes_background_execution() {
    let prepared = prepare(
        r#"
VAR_GLOBAL Order : DINT; END_VAR
PROGRAM Scheduled
VAR_EXTERNAL Order : DINT; END_VAR
Order := Order * 10 + 1;
END_PROGRAM
PROGRAM Background
VAR_EXTERNAL Order : DINT; END_VAR
Order := Order * 10 + 2;
END_PROGRAM
CONFIGURATION C
TASK Tick (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM First WITH Tick : Scheduled;
PROGRAM Second : Background;
END_CONFIGURATION
"#,
    );
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert_eq!(state.storage().get_global("Order"), Some(&Value::DInt(12)));
}

#[test]
fn sparse_one_mebibyte_marker_image_cost_depends_on_bindings() {
    let prepared = prepare(
        r#"
PROGRAM Main
VAR
    marker AT %MD1048572 : DINT;
    published AT %QD0 : DINT;
END_VAR
marker := marker + DINT#1;
published := marker;
END_PROGRAM
"#,
    );
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert_eq!(state.outputs(), &1i32.to_le_bytes());
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert_eq!(state.outputs(), &2i32.to_le_bytes());
}

struct BoundaryClock {
    polls: Cell<usize>,
    expire_at: Cell<Option<usize>>,
}
impl ExecutionServices for BoundaryClock {
    fn deadline_exceeded(&self) -> bool {
        let count = self.polls.get() + 1;
        self.polls.set(count);
        self.expire_at.get().is_some_and(|limit| count >= limit)
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::TypeMismatch)
    }
}

#[test]
fn cycle_completion_deadline_is_checked_before_any_output_is_published() {
    let prepared = prepare(
        "PROGRAM Main VAR output_bit AT %QX0.0 : BOOL; END_VAR output_bit := TRUE; END_PROGRAM",
    );
    let clock = BoundaryClock {
        polls: Cell::new(0),
        expire_at: Cell::new(None),
    };
    let mut successful = prepared.instantiate_with_services(0, &clock).unwrap();
    clock.polls.set(0);
    successful.execute_cycle(Duration::from_millis(10)).unwrap();
    let final_poll = clock.polls.get();
    assert!(final_poll >= 2);
    assert_eq!(successful.outputs(), [1]);
    let mut interrupted = prepared.instantiate_with_services(0, &clock).unwrap();
    clock.polls.set(0);
    clock.expire_at.set(Some(final_poll));
    let fault = interrupted
        .execute_cycle(Duration::from_millis(10))
        .unwrap_err();
    assert_eq!(
        fault,
        trust_runtime_core::vm::VmTrap::DeadlineExceeded.into_runtime_error()
    );
    assert_eq!(
        interrupted.outputs(),
        [0],
        "completion expiry cannot expose the staged TRUE output"
    );
    assert_eq!(interrupted.fault(), Some(&fault));
}

#[test]
fn engineering_completion_deadline_keeps_the_destination_unchanged() {
    let prepared = prepare("VAR_GLOBAL probe : BOOL; END_VAR PROGRAM Main END_PROGRAM");
    let clock = BoundaryClock {
        polls: Cell::new(0),
        expire_at: Cell::new(None),
    };
    let mut successful = prepared.instantiate_with_services(0, &clock).unwrap();
    clock.polls.set(0);
    successful.write_global("probe", Value::Bool(true)).unwrap();
    let final_poll = clock.polls.get();
    assert!(final_poll >= 2);
    let mut interrupted = prepared.instantiate_with_services(0, &clock).unwrap();
    clock.polls.set(0);
    clock.expire_at.set(Some(final_poll));
    assert_eq!(
        interrupted.write_global("probe", Value::Bool(true)),
        Err(trust_runtime_core::vm::VmTrap::DeadlineExceeded.into_runtime_error())
    );
    assert_eq!(
        interrupted.storage().get_global("probe"),
        Some(&Value::Bool(false))
    );
}

#[test]
fn restart_completion_expiry_preserves_the_previous_runtime() {
    let prepared = prepare("VAR_GLOBAL probe : DINT := DINT#7; END_VAR PROGRAM Main END_PROGRAM");
    let clock = BoundaryClock {
        polls: Cell::new(0),
        expire_at: Cell::new(None),
    };
    // This fixture has no tasks, statics, images or retained data: a fresh build
    // and a cold restart candidate perform the same construction work.
    clock.polls.set(0);
    let mut successful = prepared.instantiate_with_services(0, &clock).unwrap();
    let build_polls = clock.polls.get();
    successful.write_global("probe", Value::DInt(99)).unwrap();
    clock.polls.set(0);
    successful
        .restart(trust_runtime_core::retain::RestartMode::Cold)
        .unwrap();
    let final_poll = build_polls + 1;
    assert_eq!(
        clock.polls.get(),
        final_poll,
        "restart checks again after candidate construction before publication"
    );
    assert_eq!(
        successful.storage().get_global("probe"),
        Some(&Value::DInt(7))
    );

    let mut interrupted = prepared.instantiate_with_services(0, &clock).unwrap();
    interrupted.write_global("probe", Value::DInt(99)).unwrap();
    clock.polls.set(0);
    clock.expire_at.set(Some(final_poll));
    assert_eq!(
        interrupted.restart(trust_runtime_core::retain::RestartMode::Cold),
        Err(trust_runtime_core::vm::VmTrap::DeadlineExceeded.into_runtime_error())
    );
    assert_eq!(clock.polls.get(), final_poll);
    assert_eq!(
        interrupted.storage().get_global("probe"),
        Some(&Value::DInt(99))
    );
    assert!(
        interrupted.fault().is_none(),
        "failed candidate must not replace/fault the original state"
    );
}
