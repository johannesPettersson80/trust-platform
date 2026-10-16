//! Failure injection: a physical deadline at every poll index of a cycle and of a warm restart.
//!
//! Spec 34 TIME-03 and TIME-07 and spec 11 (output commit rollback-safe, per-cycle deadline
//! lifecycle): a refused cycle never exposes a partially updated image, the execution fault is
//! latched once, and a refused restart leaves the previous runtime intact.
//!
//! The poll count is measured from a successful run of the same fixture instead of being
//! hard-coded. The counting clock is the seam used by `source_free_cycle_contract.rs`.
use std::cell::Cell;

use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::{
    CycleAccess, CycleHooks, ExecutionServices, PreparationLimits, PreparedModule, RuntimeState,
};

/// Counts every deadline sample. The n-th sample and every later one report expiry.
struct PollClock {
    polls: Cell<usize>,
    expire_at: Cell<Option<usize>>,
}

impl PollClock {
    fn new() -> Self {
        Self {
            polls: Cell::new(0),
            expire_at: Cell::new(None),
        }
    }
}

impl ExecutionServices for PollClock {
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

/// Records each physical submission of the output image. The default `has_output_hooks`
/// is true, so the hooked publication path, with its late polls, is exercised.
#[derive(Default)]
struct SubmitHook {
    submitted: usize,
}

impl CycleHooks for SubmitHook {
    fn publish_outputs(&mut self, _access: &CycleAccess<'_, '_>) -> Result<(), RuntimeError> {
        self.submitted += 1;
        Ok(())
    }
}

fn prepare(source: &str) -> PreparedModule {
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default()).unwrap()
}

fn deadline_error() -> RuntimeError {
    trust_runtime_core::vm::VmTrap::DeadlineExceeded.into_runtime_error()
}

const CYCLE_SOURCE: &str = r#"
PROGRAM Main
VAR
    cycles : DINT := DINT#0;
END_VAR
VAR
    first AT %QD0 : DINT;
    second AT %QD4 : DINT;
END_VAR
cycles := cycles + DINT#1;
first := cycles;
second := cycles * DINT#2;
END_PROGRAM
"#;

/// Output image after the n-th successful cycle of `CYCLE_SOURCE` (`%QD4` is bytes 4..8,
/// per spec 10 `%QD10` = bytes 10-13: the number is the byte offset).
fn cycle_image(cycle: i32) -> Vec<u8> {
    let mut image = cycle.to_le_bytes().to_vec();
    image.extend_from_slice(&(cycle * 2).to_le_bytes());
    image
}

/// A fresh instance that has completed cycle 1 at 10 ms with no expiry scheduled.
fn started<'a>(prepared: &'a PreparedModule, clock: &'a PollClock) -> RuntimeState<'a> {
    let mut state = prepared.instantiate_with_services(0, clock).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    state
}

fn hooked_started<'a>(
    prepared: &'a PreparedModule,
    clock: &'a PollClock,
) -> (RuntimeState<'a>, SubmitHook) {
    let mut state = prepared.instantiate_with_services(0, clock).unwrap();
    let mut hook = SubmitHook::default();
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hook)
        .unwrap();
    (state, hook)
}

/// Polls made by cycle 2 of the plain (no-hook) path, measured without expiry.
fn plain_second_cycle_polls() -> usize {
    let prepared = prepare(CYCLE_SOURCE);
    let clock = PollClock::new();
    let mut state = started(&prepared, &clock);
    clock.polls.set(0);
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert_eq!(state.outputs().to_vec(), cycle_image(2));
    clock.polls.get()
}

/// Polls made by cycle 2 of the hooked path, measured without expiry.
fn hooked_second_cycle_polls() -> usize {
    let prepared = prepare(CYCLE_SOURCE);
    let clock = PollClock::new();
    let (mut state, mut hook) = hooked_started(&prepared, &clock);
    clock.polls.set(0);
    hook.submitted = 0;
    state
        .execute_cycle_with_hooks(Duration::from_millis(20), &mut hook)
        .unwrap();
    assert_eq!(hook.submitted, 1);
    assert_eq!(state.outputs().to_vec(), cycle_image(2));
    clock.polls.get()
}

#[test]
fn cycle_deadline_at_every_poll_index_keeps_the_old_image_and_latches_once() {
    let prepared = prepare(CYCLE_SOURCE);
    let polls = plain_second_cycle_polls();
    assert!(
        polls >= 2,
        "a cycle must poll at entry and before publication, observed {polls}"
    );
    for index in 1..=polls {
        let clock = PollClock::new();
        let mut state = started(&prepared, &clock);
        assert_eq!(
            state.outputs().to_vec(),
            cycle_image(1),
            "poll {index}: image before the refused cycle"
        );
        clock.polls.set(0);
        clock.expire_at.set(Some(index));
        assert_eq!(
            state.execute_cycle(Duration::from_millis(20)),
            Err(deadline_error()),
            "poll {index}: the expired deadline must refuse the cycle with ExecutionTimeout"
        );
        assert_eq!(
            state.outputs().to_vec(),
            cycle_image(1),
            "poll {index}: a refused cycle without submission must publish no image"
        );
        assert_eq!(
            state.fault(),
            Some(&deadline_error()),
            "poll {index}: the refusal must latch the execution fault"
        );
        let polls_after_refusal = clock.polls.get();
        clock.expire_at.set(None);
        assert_eq!(
            state.execute_cycle(Duration::from_millis(30)),
            Err(RuntimeError::ResourceFaulted),
            "poll {index}: a latched fault must block later cycles"
        );
        assert_eq!(
            clock.polls.get(),
            polls_after_refusal,
            "poll {index}: a latched fault must not sample the clock again"
        );
        assert_eq!(
            state.fault(),
            Some(&deadline_error()),
            "poll {index}: the first fault stays latched"
        );
    }
}

#[test]
fn hooked_cycle_deadline_exposes_the_new_image_only_after_submission() {
    let prepared = prepare(CYCLE_SOURCE);
    let polls = hooked_second_cycle_polls();
    assert!(
        polls >= 2,
        "a hooked cycle must poll at entry and around publication, observed {polls}"
    );
    for index in 1..=polls {
        let clock = PollClock::new();
        let (mut state, mut hook) = hooked_started(&prepared, &clock);
        hook.submitted = 0;
        clock.polls.set(0);
        clock.expire_at.set(Some(index));
        assert_eq!(
            state.execute_cycle_with_hooks(Duration::from_millis(20), &mut hook),
            Err(deadline_error()),
            "poll {index}: the expired deadline must refuse the cycle"
        );
        let image = state.outputs().to_vec();
        match hook.submitted {
            0 => assert_eq!(
                image,
                cycle_image(1),
                "poll {index}: a refusal before submission must keep the old image"
            ),
            1 => assert_eq!(
                image,
                cycle_image(2),
                "poll {index}: a late refusal after submission keeps the committed new image"
            ),
            other => panic!("poll {index}: {other} submissions in one cycle"),
        }
        assert_eq!(
            state.fault(),
            Some(&deadline_error()),
            "poll {index}: the refusal must latch the execution fault"
        );
    }
}

const RESTART_SOURCE: &str = r#"
VAR_GLOBAL probe : DINT := DINT#7; END_VAR
PROGRAM Main
VAR RETAIN
    kept : DINT := DINT#0;
END_VAR
VAR
    shown AT %QD0 : DINT;
END_VAR
kept := kept + DINT#1;
shown := kept;
END_PROGRAM
"#;

/// The previous runtime after cycle 1 (`kept` = 1, `shown` = 1) with a global set to 99.
fn restart_fixture<'a>(prepared: &'a PreparedModule, clock: &'a PollClock) -> RuntimeState<'a> {
    let mut state = started(prepared, clock);
    state.write_global("probe", Value::DInt(99)).unwrap();
    state
}

/// Polls made by a successful warm restart of the fixture, measured without expiry.
fn warm_restart_polls() -> usize {
    let prepared = prepare(RESTART_SOURCE);
    let clock = PollClock::new();
    let mut state = restart_fixture(&prepared, &clock);
    clock.polls.set(0);
    state.restart(RestartMode::Warm).unwrap();
    clock.polls.get()
}

#[test]
fn warm_restart_deadline_at_every_poll_index_keeps_the_previous_runtime() {
    let prepared = prepare(RESTART_SOURCE);
    let polls = warm_restart_polls();
    assert!(
        polls >= 2,
        "a warm restart must poll during construction and before commit, observed {polls}"
    );
    for index in 1..=polls {
        let clock = PollClock::new();
        let mut state = restart_fixture(&prepared, &clock);
        clock.polls.set(0);
        clock.expire_at.set(Some(index));
        assert_eq!(
            state.restart(RestartMode::Warm),
            Err(deadline_error()),
            "poll {index}: the expired deadline must refuse the warm restart"
        );
        assert_eq!(
            state.storage().get_global("probe"),
            Some(&Value::DInt(99)),
            "poll {index}: globals of the previous runtime must be unchanged"
        );
        assert_eq!(
            state.outputs().to_vec(),
            1i32.to_le_bytes().to_vec(),
            "poll {index}: the published image must be unchanged"
        );
        assert!(
            state.fault().is_none(),
            "poll {index}: a refused restart must not latch a fault on the previous runtime"
        );
        clock.expire_at.set(None);
        state
            .execute_cycle(Duration::from_millis(20))
            .unwrap_or_else(|error| {
                panic!("poll {index}: the previous runtime must still execute: {error:?}")
            });
        assert_eq!(
            state.outputs().to_vec(),
            2i32.to_le_bytes().to_vec(),
            "poll {index}: the retained value of the previous runtime must advance"
        );
    }
}
