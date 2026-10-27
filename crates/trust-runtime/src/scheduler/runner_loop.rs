fn scaled_time(now: Duration, scale: u32) -> Duration {
    if scale <= 1 {
        return now;
    }
    let scaled = now.as_nanos().saturating_mul(i64::from(scale));
    Duration::from_nanos(scaled)
}

fn scaled_sleep_interval(interval: Duration, scale: u32) -> Duration {
    if scale <= 1 {
        return interval;
    }
    let nanos = interval.as_nanos();
    if nanos <= 0 {
        return Duration::ZERO;
    }
    let scaled = (nanos / i64::from(scale)).max(1);
    Duration::from_nanos(scaled)
}

/// Start of the current cycle on the fixed cycle grid: the scheduled instant while the cycle starts within one interval
/// of it, so a late wake-up does not delay every later cycle; otherwise `now`, so a stall or a paused debugger moves the
/// grid instead of replaying the missed cycles in a burst.
fn grid_cycle_start(scheduled: Option<i64>, now: i64, interval: i64) -> i64 {
    match scheduled {
        Some(due) if now <= due.saturating_add(interval) => due,
        _ => now,
    }
}

/// The nominal start of a cycle on the wall clock's fixed grid (specification 10 §4.3): the
/// boundary `boundary` it was due at, or, for a cycle that starts a whole `interval` or more after
/// it, the last boundary of the grid at or before `now`. A delayed wake or a cycle that overran
/// thus skips the boundaries it missed, and the grid stays on the tasks' deadlines.
fn wall_grid_start(boundary: i64, now: i64, interval: i64) -> i64 {
    let late = now.saturating_sub(boundary);
    if interval <= 0 || late < interval {
        return boundary;
    }
    boundary.saturating_add((late / interval).saturating_mul(interval))
}

fn cycle_deadline_from(start: std::time::Instant, timeout: Duration) -> Option<std::time::Instant> {
    if timeout.as_nanos() <= 0 {
        return Some(start);
    }
    let nanos = u64::try_from(timeout.as_nanos()).unwrap_or(u64::MAX);
    Some(start + std::time::Duration::from_nanos(nanos))
}

#[derive(Debug, Clone, Copy)]
struct CycleDeadlineSnapshot {
    execution_deadline: Option<std::time::Instant>,
    output_deadline: Option<std::time::Instant>,
}

fn arm_cycle_deadlines(
    runtime: &mut CompiledRuntime,
    watchdog: crate::watchdog::WatchdogPolicy,
    wall_start: std::time::Instant,
) -> CycleDeadlineSnapshot {
    let snapshot = CycleDeadlineSnapshot {
        execution_deadline: runtime.effective_execution_deadline(),
        output_deadline: runtime.effective_output_commit_deadline(),
    };
    if watchdog.enabled {
        let cycle_deadline = cycle_deadline_from(wall_start, watchdog.timeout);
        runtime.set_execution_deadline(cycle_deadline);
        runtime.set_output_commit_deadline(cycle_deadline);
    }
    snapshot
}

fn restore_cycle_deadlines(runtime: &mut CompiledRuntime, snapshot: CycleDeadlineSnapshot) {
    runtime.set_execution_deadline(snapshot.execution_deadline);
    runtime.set_output_commit_deadline(snapshot.output_deadline);
}

fn run_resource_loop<C: Clock + Clone>(
    runner: ResourceRunner<C>,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
    supervision: Arc<supervisor::ScanSupervision>,
) {
    run_resource_loop_core(runner, stop, state, last_error, supervision, |runtime| {
        runtime.execute_cycle()
    });
}

/// Runs the scan loop until a stop request ends it. A halting fault does not end the loop: the
/// resource stays `Faulted` in the faulted wait (specification 11 section 6.6) and resumes only
/// after a successful restart or online change. A program stop, the control `shutdown`, does not
/// end the loop either: the resource waits `Stopped` until a `restart` starts the program again
/// (specification 34, STATE-08).
fn run_resource_loop_core<C, F>(
    mut runner: ResourceRunner<C>,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
    supervision: Arc<supervisor::ScanSupervision>,
    mut execute_cycle: F,
) where
    C: Clock + Clone,
    F: FnMut(&mut CompiledRuntime) -> Result<(), RuntimeError>,
{
    let shared = LoopShared {
        stop: &stop,
        state: &state,
        last_error: &last_error,
        supervision: &supervision,
    };
    let mut paused = false;
    let mut restarts = AutomaticRestartLimiter::default();
    let mut hang_supervisor: Option<supervisor::HangSupervisor> = None;
    // The nominal start of the next cycle on the cycle grid, once a cycle has completed. A restart,
    // a start after a stop and a recovery from a fault begin the grid again at the start of their
    // first cycle (specification 10 §4.3), so each of them drops it; a pause keeps it, as the
    // tasks keep their deadlines through it.
    let mut boundary: Option<Duration> = None;
    if let Some(gate) = runner.start_gate.as_ref() {
        set_resource_state(&state, ResourceState::Ready);
        if !gate.wait_open(&stop) {
            set_resource_state(&state, ResourceState::Stopped);
            return;
        }
    }
    set_resource_state(&state, ResourceState::Running);
    loop {
        if let Err(err) =
            supervisor::sync_hang_supervisor(&mut hang_supervisor, &runner, &supervision)
        {
            if halt(&mut runner, &shared, &mut restarts, err) == Next::Exit {
                return;
            }
            paused = false;
            boundary = None;
            continue;
        }

        if stop.load(Ordering::SeqCst) {
            stopping::stop_resource(&mut runner, &shared);
            break;
        }

        if supervision.take_program_stop() {
            if stopping::stop_program(&mut runner, &shared, &mut restarts) == Next::Exit {
                return;
            }
            paused = false;
            boundary = None;
            continue;
        }

        drain_commands(&mut runner, &state, &mut paused);

        if let Some(mode) = faulted::take_pending_restart(&runner) {
            // The restart restarts the task timing now, and the grid with it.
            boundary = None;
            match runner.restart_or_reload(mode) {
                Ok(()) => {
                    restarts.reset();
                    faulted::clear_recovered_fault(&supervision, &last_error);
                }
                Err(err) => {
                    if halt(&mut runner, &shared, &mut restarts, err) == Next::Exit {
                        return;
                    }
                    paused = false;
                    continue;
                }
            }
        }

        if paused {
            // A paused resource scans nothing, so identify blinks from here (specification 11,
            // "Identify"). The program's outputs keep their committed values.
            runner.runtime.drive_identify_while_idle();
            sleep_while_paused(&runner);
            continue;
        }

        let now_raw = runner.clock.now();
        // The cycle's nominal start on the grid (specification 10 §4.3): the boundary it was due
        // at, or the last boundary at or before its start when it starts a whole interval or more
        // after it, as after a wake the operating system delayed that long or a cycle that
        // overran, so that the grid keeps the tasks' deadlines and no cycle finds them not yet due.
        let pace = scaled_sleep_interval(runner.cycle_interval, runner.timing.scale()).as_nanos();
        let due = boundary.take().map_or(now_raw, |boundary| {
            Duration::from_nanos(wall_grid_start(
                boundary.as_nanos(),
                now_raw.as_nanos(),
                pace,
            ))
        });
        // The cycle's process time counts from the program's start (specification 34, STATE-08):
        // the wall clock, or simulation time, one cycle interval for each executed cycle
        // (specification 46, §6.1 item 1).
        let slot = runner
            .timing
            .begin_cycle(runner.program_time(now_raw), runner.cycle_interval);
        let now = slot.process_time;
        runner.runtime.set_current_time(now);
        let wall_start = std::time::Instant::now();
        let watchdog = runner.runtime.watchdog_policy();
        let deadline_snapshot = arm_cycle_deadlines(&mut runner.runtime, watchdog, wall_start);
        let pause_before_cycle = runner.runtime.debug_watchdog_pause_elapsed();
        if watchdog.enabled {
            // Published before the cycle runs, so a call that never returns is still seen.
            supervision.publish(wall_start, watchdog.timeout, pause_before_cycle);
        }
        let result = execute_protected_cycle(&mut runner, now, &mut execute_cycle);
        let paused_during_cycle = runner
            .runtime
            .debug_watchdog_pause_elapsed()
            .saturating_sub(pause_before_cycle);
        restore_cycle_deadlines(&mut runner.runtime, deadline_snapshot);
        supervision.withdraw();
        if let Err(err) = result {
            if handle_cycle_error(&mut runner, &shared, &mut restarts, watchdog, err) == Next::Exit
            {
                return;
            }
            continue;
        }
        let active_elapsed = wall_start.elapsed().saturating_sub(paused_during_cycle);
        if let Some(next) = handle_watchdog_overrun(
            &mut runner,
            &shared,
            &mut restarts,
            watchdog,
            active_elapsed,
        ) {
            if next == Next::Exit {
                return;
            }
            continue;
        }
        restarts.reset();
        boundary = sleep_until_next_boundary(&mut runner, due, slot);
    }
}

/// The handles every step of the scan loop shares with the control plane and the supervisor.
struct LoopShared<'a> {
    stop: &'a AtomicBool,
    state: &'a Arc<Mutex<ResourceState>>,
    last_error: &'a Arc<Mutex<Option<RuntimeError>>>,
    supervision: &'a supervisor::ScanSupervision,
}

/// What the scan loop does after a fault was handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    /// A recovery succeeded, so the loop continues with the next pass.
    Continue,
    /// An exit request ended the wait, after the stop it asks for, so the thread ends.
    Exit,
}

/// Records a halting fault and waits in the faulted state (spec 11 section 6.6).
fn halt<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    shared: &LoopShared<'_>,
    restarts: &mut AutomaticRestartLimiter,
    fault: RuntimeError,
) -> Next {
    let wait = faulted::halt_and_wait(
        runner,
        shared.stop,
        shared.state,
        shared.last_error,
        shared.supervision,
        restarts,
        fault,
    );
    stopping::follow_faulted_wait(wait, runner, shared, restarts)
}

/// Tries an automatic warm restart; when the retry budget is spent or the restart fails, the
/// resource is faulted and waits for recovery.
fn retry_or_wait<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    shared: &LoopShared<'_>,
    restarts: &mut AutomaticRestartLimiter,
    reason: &RuntimeError,
) -> Next {
    if try_automatic_restart(
        runner,
        restarts,
        reason,
        shared.state,
        shared.last_error,
        shared.supervision,
    ) {
        return Next::Continue;
    }
    let wait = faulted::wait_while_faulted(
        runner,
        shared.stop,
        shared.state,
        shared.last_error,
        shared.supervision,
        restarts,
    );
    stopping::follow_faulted_wait(wait, runner, shared, restarts)
}

/// Follows the shared decision for a cycle error: the watchdog action for a timeout or limit
/// error, the fault policy's automatic restart for other errors (never for a contained panic),
/// and otherwise a halt.
fn handle_cycle_error<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    shared: &LoopShared<'_>,
    restarts: &mut AutomaticRestartLimiter,
    watchdog: crate::watchdog::WatchdogPolicy,
    err: RuntimeError,
) -> Next {
    match cycle_error_action(watchdog, runner.runtime.fault_policy(), err) {
        CycleErrorAction::Restart(reason) => retry_or_wait(runner, shared, restarts, &reason),
        CycleErrorAction::WatchdogHalt(watchdog_err) => {
            let err = runner.runtime.watchdog_fault(watchdog_err);
            halt(runner, shared, restarts, err)
        }
        CycleErrorAction::Halt(err) => halt(runner, shared, restarts, err),
    }
}

/// Applies the watchdog action when a cycle that returned still overran its active-execution
/// budget. None when the watchdog is disabled or the cycle stayed within it.
fn handle_watchdog_overrun<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    shared: &LoopShared<'_>,
    restarts: &mut AutomaticRestartLimiter,
    watchdog: crate::watchdog::WatchdogPolicy,
    active_elapsed: std::time::Duration,
) -> Option<Next> {
    match watchdog_overrun_action(watchdog, active_elapsed)? {
        OverrunAction::Restart => Some(retry_or_wait(
            runner,
            shared,
            restarts,
            &RuntimeError::WatchdogTimeout,
        )),
        OverrunAction::Halt => {
            let err = runner.runtime.watchdog_timeout();
            Some(halt(runner, shared, restarts, err))
        }
    }
}

/// Serves every queued command: pause and resume change the loop's state, the others act on the
/// runtime (the shared running-command rules).
fn drain_commands<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    state: &Arc<Mutex<ResourceState>>,
    paused: &mut bool,
) {
    let Some(commands) = runner.command_rx.as_ref() else {
        return;
    };
    while let Ok(command) = commands.try_recv() {
        match running_command_effect(command_class(&command)) {
            // A pause and a resume restart the simulation schedule (specification 46, §6.1 item 4).
            RunningCommandEffect::Pause => {
                *paused = true;
                set_resource_state(state, ResourceState::Paused);
                runner.timing.restart_schedule();
            }
            RunningCommandEffect::Resume => {
                *paused = false;
                set_resource_state(state, ResourceState::Running);
                runner.timing.restart_schedule();
            }
            // The scan loop owns the simulation, so it serves the simulation's commands itself.
            RunningCommandEffect::Apply => match command {
                ResourceCommand::SimulationSnapshot { respond_to } => {
                    let _ = respond_to.send(runner.simulation_snapshot());
                }
                ResourceCommand::SimulationTraceClear {
                    plant_id,
                    active_configuration_revision,
                    respond_to,
                } => {
                    let result = runner
                        .simulation
                        .as_mut()
                        .ok_or(crate::simulation::PlantTraceClearError::StatusUnavailable)
                        .and_then(|simulation| {
                            simulation.clear_plant_trace(&plant_id, &active_configuration_revision)
                        });
                    let _ = respond_to.send(result);
                }
                other => {
                    apply_runner_command(&mut runner.runtime, runner.refuse_mesh_catalogue, other)
                }
            },
        }
    }
}

/// Sleeps until the next cycle is due. On the wall clock it returns the nominal start of the next
/// cycle: the boundary one cycle interval after `due`, the nominal start of the cycle that ended,
/// so that the cycles keep a fixed grid and the time a wake takes is never added to the next
/// interval (specification 10 §4.3; specification 34 CYCLE-03). When that boundary has passed,
/// after a cycle longer than its interval, it is returned all the same: the next cycle starts at
/// once, on the last boundary of the grid at or before its start (`wall_grid_start`), without a
/// burst of cycles to catch up, and the tasks count the activations that passed as missed. In
/// simulation the next cycle is due one paced interval, the cycle interval divided by
/// the speed, after the cycle `slot`; a cycle that starts within one paced interval of its due time
/// keeps that schedule, and a later one restarts it from its own start (specification 46, §6.1
/// item 2). Returns `None` in simulation, and yields and returns `None` when the interval is not
/// positive.
fn sleep_until_next_boundary<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    due: Duration,
    slot: cycle_timing::CycleSlot,
) -> Option<Duration> {
    if runner.cycle_interval.as_nanos() <= 0 {
        thread::yield_now();
        return None;
    }
    if runner.timing.simulation_speed().is_some() {
        let deadline = runner.timing.finish_cycle(slot);
        runner.clock.sleep_until(runner.clock_time(deadline));
        return None;
    }
    let sleep_interval = scaled_sleep_interval(runner.cycle_interval, runner.timing.scale());
    let next = Duration::from_nanos(due.as_nanos().saturating_add(sleep_interval.as_nanos()));
    if next <= runner.clock.now() {
        return Some(next);
    }
    runner.clock.sleep_until(next);
    Some(next)
}

/// The pause between two passes of the paused wait: one cycle interval, or at most the poll period
/// of the stopped and faulted waits while `identify` runs, so that a long cycle interval does not
/// make the 250 ms edges of its blink late by up to one interval.
fn sleep_while_paused<C: Clock + Clone>(runner: &ResourceRunner<C>) {
    let from = runner.clock.now();
    if runner.cycle_interval.as_nanos() > 0 && runner.runtime.identify_is_running() {
        let pass = scaled_sleep_interval(runner.cycle_interval, runner.timing.scale()).as_nanos();
        let poll = i64::try_from(faulted::FAULTED_WAIT_POLL.as_nanos()).unwrap_or(i64::MAX);
        let deadline = Duration::from_nanos(from.as_nanos().saturating_add(pass.min(poll)));
        runner.clock.sleep_until(deadline);
        return;
    }
    sleep_one_interval(runner, from);
}

/// Sleeps until one cycle interval after `from`, or yields when the interval is not positive.
fn sleep_one_interval<C: Clock + Clone>(runner: &ResourceRunner<C>, from: Duration) {
    let interval = runner.cycle_interval.as_nanos();
    if interval <= 0 {
        thread::yield_now();
        return;
    }
    let sleep_interval = scaled_sleep_interval(runner.cycle_interval, runner.timing.scale());
    let deadline = Duration::from_nanos(from.as_nanos().saturating_add(sleep_interval.as_nanos()));
    runner.clock.sleep_until(deadline);
}

fn try_automatic_restart<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    automatic_restarts: &mut AutomaticRestartLimiter,
    reason: &RuntimeError,
    state: &Arc<Mutex<ResourceState>>,
    last_error: &Arc<Mutex<Option<RuntimeError>>>,
    supervision: &supervisor::ScanSupervision,
) -> bool {
    let attempt = match automatic_restarts.next_attempt(reason) {
        Ok(attempt) => attempt,
        Err(err) => {
            set_last_error(last_error, err);
            set_resource_state(state, ResourceState::Faulted);
            return false;
        }
    };
    if let Err(restart_err) = runner.restart(crate::RestartMode::Warm) {
        set_last_error(last_error, restart_err);
        set_resource_state(state, ResourceState::Faulted);
        return false;
    }
    // A retry after a hung call starts from Faulted, so the successful restart resumes Running.
    faulted::clear_recovered_fault(supervision, last_error);
    set_resource_state(state, ResourceState::Running);
    apply_automatic_restart_backoff(runner, attempt);
    true
}

fn apply_automatic_restart_backoff<C: Clock + Clone>(runner: &ResourceRunner<C>, attempt: u32) {
    let backoff = automatic_restart_backoff(runner.cycle_interval, attempt);
    if backoff.as_nanos() <= 0 {
        thread::yield_now();
        return;
    }
    let deadline = Duration::from_nanos(
        runner
            .clock
            .now()
            .as_nanos()
            .saturating_add(backoff.as_nanos()),
    );
    runner.clock.sleep_until(deadline);
}

#[derive(Debug)]
struct CyclePanicSentinel {
    entered: bool,
    exited_cleanly: bool,
}

impl CyclePanicSentinel {
    fn entered() -> Self {
        Self {
            entered: true,
            exited_cleanly: false,
        }
    }

    fn mark_exited_cleanly(&mut self) {
        self.exited_cleanly = true;
    }

    fn panicked_mid_cycle(&self) -> bool {
        self.entered && !self.exited_cleanly
    }
}

fn execute_protected_cycle<C, F>(
    runner: &mut ResourceRunner<C>,
    now: Duration,
    execute_cycle: &mut F,
) -> Result<(), RuntimeError>
where
    C: Clock + Clone,
    F: FnMut(&mut CompiledRuntime) -> Result<(), RuntimeError>,
{
    let mut sentinel = CyclePanicSentinel::entered();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(simulation) = runner.simulation.as_mut() {
            simulation.apply_pre_cycle(now, &mut runner.runtime)?;
        }
        let mut result = execute_cycle(&mut runner.runtime);
        if result.is_ok()
            && let Some(simulation) = runner.simulation.as_mut()
        {
            result = simulation.apply_post_cycle(now, &runner.runtime);
        }
        result
    }));

    match result {
        Ok(result) => {
            sentinel.mark_exited_cleanly();
            result
        }
        Err(payload) => {
            debug_assert!(sentinel.panicked_mid_cycle());
            Err(runner
                .runtime
                .resource_panic(panic_payload_message(payload)))
        }
    }
}

fn panic_payload_message(payload: Box<dyn std::any::Any + Send>) -> SmolStr {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return SmolStr::new(*message);
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return SmolStr::new(message.as_str());
    }
    SmolStr::new("non-string panic payload")
}

fn recover_mutex_lock<T>(
    result: std::sync::LockResult<std::sync::MutexGuard<'_, T>>,
) -> std::sync::MutexGuard<'_, T> {
    result.unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn set_resource_state(state: &Arc<Mutex<ResourceState>>, next: ResourceState) {
    *recover_mutex_lock(state.lock()) = next;
}

fn set_last_error(last_error: &Arc<Mutex<Option<RuntimeError>>>, err: RuntimeError) {
    *recover_mutex_lock(last_error.lock()) = Some(err);
}

#[cfg(test)]
#[path = "runner_loop/restart_storage_trace_cases.rs"]
mod restart_storage_trace_cases;

#[cfg(test)]
#[path = "runner_loop/pacing_tests.rs"]
mod pacing_tests;

#[cfg(test)]
mod runner_loop_poison_tests {
    use super::*;
    use crate::harness::TestHarness;
    use crate::value::Value;

    /// An empty compiled program, which the scheduler runs; the build needs the toolchain of
    /// specification 44 §4.6 at `TRUST_TOOLCHAIN`.
    fn empty_program() -> CompiledRuntime {
        TestHarness::from_source("PROGRAM Main\nEND_PROGRAM\n")
            .expect("build the runner fixture")
            .into_runtime()
    }

    #[derive(Clone, Debug)]
    struct TestClock;

    impl Clock for TestClock {
        fn now(&self) -> Duration {
            Duration::ZERO
        }

        fn sleep_until(&self, _deadline: Duration) {}

        fn wake(&self) {}
    }

    /// Stops the loop once the injected fault has been recorded, so the faulted wait can end.
    /// Asks for the exit once the resource reads `Faulted`, or after 5 s; the thread returns whether
    /// it saw the resource faulted.
    fn stop_once_faulted(
        state: Arc<Mutex<ResourceState>>,
        stop: Arc<AtomicBool>,
    ) -> std::thread::JoinHandle<bool> {
        std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while *recover_mutex_lock(state.lock()) != ResourceState::Faulted
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let faulted = *recover_mutex_lock(state.lock()) == ResourceState::Faulted;
            stop.store(true, Ordering::SeqCst);
            faulted
        })
    }

    #[test]
    fn state_and_error_helpers_recover_poisoned_mutexes() {
        let state = Arc::new(Mutex::new(ResourceState::Ready));
        let poisoned_state = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let mut guard = poisoned_state.lock().expect("test state lock");
            *guard = ResourceState::Paused;
            panic!("poison resource state");
        })
        .join();

        set_resource_state(&state, ResourceState::Running);
        assert_eq!(*recover_mutex_lock(state.lock()), ResourceState::Running);

        let last_error = Arc::new(Mutex::new(None));
        let poisoned_error = Arc::clone(&last_error);
        let _ = std::thread::spawn(move || {
            let mut guard = poisoned_error.lock().expect("test error lock");
            *guard = Some(RuntimeError::TypeMismatch);
            panic!("poison resource error");
        })
        .join();

        set_last_error(&last_error, RuntimeError::DivisionByZero);
        assert_eq!(
            *recover_mutex_lock(last_error.lock()),
            Some(RuntimeError::DivisionByZero)
        );
    }

    /// A clock whose every wake-up returns 0.5 ms after the instant it was asked to wake.
    #[derive(Clone, Debug)]
    struct LateWakeClock {
        now: Arc<Mutex<Duration>>,
    }

    impl Clock for LateWakeClock {
        fn now(&self) -> Duration {
            *self.now.lock().expect("late clock lock")
        }

        fn sleep_until(&self, deadline: Duration) {
            let mut now = self.now.lock().expect("late clock lock");
            *now = Duration::from_nanos(deadline.as_nanos().max(now.as_nanos()) + 500_000);
        }

        fn wake(&self) {}
    }

    #[test]
    fn late_wake_ups_do_not_shift_the_cycle_grid() {
        let clock = LateWakeClock {
            now: Arc::new(Mutex::new(Duration::ZERO)),
        };
        let runner = ResourceRunner::new(empty_program(), clock, Duration::from_millis(10));
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(ResourceState::Boot));
        let last_error = Arc::new(Mutex::new(None));
        let supervision = Arc::new(supervisor::ScanSupervision::new(
            Arc::clone(&state),
            Arc::clone(&last_error),
        ));
        let stopper = stop_once_faulted(Arc::clone(&state), Arc::clone(&stop));
        let starts = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&starts);
        run_resource_loop_core(
            runner,
            stop,
            state,
            last_error,
            supervision,
            move |runtime| {
                let mut starts = recorded.lock().expect("cycle starts lock");
                starts.push(runtime.current_time().as_nanos());
                if starts.len() == 101 {
                    return Err(RuntimeError::SimulationFault(
                        "stop after 101 cycles".into(),
                    ));
                }
                Ok(())
            },
        );
        stopper.join().expect("stop helper");
        let starts = starts.lock().expect("cycle starts lock");
        assert_eq!(starts.len(), 101);
        assert_eq!(
            starts[100], 1_000_500_000,
            "100 cycles of 10 ms keep the 1 s grid; one late wake-up is not added per cycle"
        );
    }

    #[test]
    fn scheduler_arms_execution_deadline_during_cycle() {
        let mut runtime = empty_program();
        runtime.set_watchdog_policy(crate::watchdog::WatchdogPolicy {
            enabled: true,
            timeout: Duration::from_millis(10),
            action: crate::watchdog::WatchdogAction::Halt,
        });
        let runner = ResourceRunner::new(runtime, TestClock, Duration::from_millis(1));
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(ResourceState::Boot));
        let last_error = Arc::new(Mutex::new(None));
        let supervision = Arc::new(supervisor::ScanSupervision::new(
            Arc::clone(&state),
            Arc::clone(&last_error),
        ));
        let observed_deadline = Arc::new(Mutex::new(None));
        let observed_deadline_for_cycle = Arc::clone(&observed_deadline);
        let stopper = stop_once_faulted(Arc::clone(&state), Arc::clone(&stop));

        run_resource_loop_core(
            runner,
            stop,
            state,
            last_error,
            supervision,
            move |runtime| {
                *observed_deadline_for_cycle
                    .lock()
                    .expect("observed deadline lock") = Some(runtime.execution_deadline());
                Err(RuntimeError::SimulationFault("stop after one cycle".into()))
            },
        );
        stopper.join().expect("stop helper");

        assert!(
            observed_deadline
                .lock()
                .expect("observed deadline lock")
                .expect("cycle should run")
                .is_some(),
            "scheduler must arm the execution deadline before cycle execution"
        );
    }

    #[test]
    fn cycle_deadline_from_zero_timeout_is_immediately_expired() {
        let start = std::time::Instant::now() - std::time::Duration::from_millis(1);
        let deadline = cycle_deadline_from(start, Duration::ZERO)
            .expect("zero timeout still creates a deadline");

        assert!(
            deadline <= std::time::Instant::now(),
            "zero timeout must be treated as already expired"
        );
    }

    #[test]
    fn cycle_deadline_guard_restores_previous_deadlines_after_success() {
        let mut runtime = empty_program();
        let previous = std::time::Instant::now() + std::time::Duration::from_secs(30);
        runtime.set_execution_deadline(Some(previous));
        runtime.set_output_commit_deadline(Some(previous));

        let snapshot = arm_cycle_deadlines(
            &mut runtime,
            crate::watchdog::WatchdogPolicy {
                enabled: true,
                timeout: Duration::from_millis(5),
                action: crate::watchdog::WatchdogAction::Halt,
            },
            std::time::Instant::now(),
        );
        assert_ne!(
            runtime.execution_deadline(),
            Some(previous),
            "enabled watchdog should arm a per-cycle execution deadline"
        );

        restore_cycle_deadlines(&mut runtime, snapshot);

        assert_eq!(runtime.execution_deadline(), Some(previous));
        assert_eq!(runtime.output_commit_deadline(), Some(previous));
    }

    #[test]
    fn cycle_deadline_guard_restores_after_runtime_error_and_contained_panic() {
        for outcome in ["runtime-error", "panic"] {
            let mut runtime = empty_program();
            let snapshot = arm_cycle_deadlines(
                &mut runtime,
                crate::watchdog::WatchdogPolicy {
                    enabled: true,
                    timeout: Duration::from_millis(5),
                    action: crate::watchdog::WatchdogAction::Halt,
                },
                std::time::Instant::now(),
            );
            assert!(runtime.execution_deadline().is_some());

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if outcome == "panic" {
                    panic!("contained panic before deadline restore");
                }
                Err::<(), RuntimeError>(RuntimeError::SimulationFault(
                    "ordinary cycle error".into(),
                ))
            }));
            restore_cycle_deadlines(&mut runtime, snapshot);

            assert!(
                result.is_ok() || outcome == "panic",
                "ordinary runtime error should not panic"
            );
            assert_eq!(
                runtime.execution_deadline(),
                None,
                "{outcome} must not leak execution deadline"
            );
            assert_eq!(
                runtime.output_commit_deadline(),
                None,
                "{outcome} must not leak output deadline"
            );
        }
    }

    #[test]
    fn disabled_watchdog_does_not_arm_cycle_deadlines() {
        let mut runtime = empty_program();
        let snapshot = arm_cycle_deadlines(
            &mut runtime,
            crate::watchdog::WatchdogPolicy {
                enabled: false,
                timeout: Duration::ZERO,
                action: crate::watchdog::WatchdogAction::Halt,
            },
            std::time::Instant::now(),
        );

        assert_eq!(runtime.execution_deadline(), None);
        assert_eq!(runtime.output_commit_deadline(), None);
        restore_cycle_deadlines(&mut runtime, snapshot);
        assert_eq!(runtime.execution_deadline(), None);
        assert_eq!(runtime.output_commit_deadline(), None);
    }

    #[test]
    fn cycle_execution_panic_faults_resource_visibly() {
        let runtime = empty_program();
        let runner = ResourceRunner::new(runtime, TestClock, Duration::from_millis(1));
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(ResourceState::Boot));
        let last_error = Arc::new(Mutex::new(None));
        let supervision = Arc::new(supervisor::ScanSupervision::new(
            Arc::clone(&state),
            Arc::clone(&last_error),
        ));
        let stopper = stop_once_faulted(Arc::clone(&state), Arc::clone(&stop));

        run_resource_loop_core(
            runner,
            stop,
            Arc::clone(&state),
            Arc::clone(&last_error),
            supervision,
            |_runtime| -> Result<(), RuntimeError> {
                panic!("intentional VM/program execution panic");
            },
        );
        assert!(
            stopper.join().expect("stop helper"),
            "the cycle panic faults the resource"
        );

        // The exit request then stops the faulted resource as a running one is stopped, and the
        // root fault stays reported (specification 34 STATE-08; specification 11 section 6.6).
        assert_eq!(*recover_mutex_lock(state.lock()), ResourceState::Stopped);
        let error = recover_mutex_lock(last_error.lock())
            .clone()
            .expect("panic must set last_error");
        assert!(
            matches!(error, RuntimeError::ResourcePanic(_)),
            "cycle panic should surface as ResourcePanic, got {error:?}"
        );
        assert!(
            error.to_string().contains("VM/program"),
            "panic payload should be diagnosable, got {error}"
        );
    }

    #[test]
    fn automatic_fault_restart_uses_warm_storage_semantics() {
        let source = r#"
VAR_GLOBAL RETAIN
    retained_value : DINT := DINT#1;
END_VAR
VAR_GLOBAL NON_RETAIN
    transient_value : DINT := DINT#2;
END_VAR
VAR_GLOBAL
    ordinary_value : DINT := DINT#3;
END_VAR

PROGRAM Main
END_PROGRAM
"#;
        let mut harness = TestHarness::from_source(source).expect("automatic restart fixture");
        harness.set_input("retained_value", Value::DInt(42));
        harness.set_input("transient_value", Value::DInt(70));
        harness.set_input("ordinary_value", Value::DInt(80));

        let mut runner =
            ResourceRunner::new(harness.into_runtime(), TestClock, Duration::from_millis(1));
        let mut limiter = AutomaticRestartLimiter::default();
        let state = Arc::new(Mutex::new(ResourceState::Running));
        let last_error = Arc::new(Mutex::new(None));
        let supervision =
            supervisor::ScanSupervision::new(Arc::clone(&state), Arc::clone(&last_error));

        assert!(try_automatic_restart(
            &mut runner,
            &mut limiter,
            &RuntimeError::DivisionByZero,
            &state,
            &last_error,
            &supervision,
        ));
        assert_eq!(
            runner.program().read_variable("retained_value"),
            Ok(Value::DInt(42)),
        );
        assert_eq!(
            runner.program().read_variable("transient_value"),
            Ok(Value::DInt(2)),
        );
        assert_eq!(
            runner.program().read_variable("ordinary_value"),
            Ok(Value::DInt(3)),
        );
        assert_eq!(*recover_mutex_lock(state.lock()), ResourceState::Running);
        assert!(recover_mutex_lock(last_error.lock()).is_none());
    }

    #[test]
    fn update_watchdog_zero_timeout_is_normalized_before_use() {
        let mut runtime = empty_program();

        let fault_policy = runtime.fault_policy();
        apply_resource_command(
            &mut runtime,
            ResourceCommand::UpdateSchedulerPolicies(SchedulerPolicies {
                watchdog: crate::watchdog::WatchdogPolicy {
                    enabled: true,
                    timeout: Duration::ZERO,
                    action: crate::watchdog::WatchdogAction::Halt,
                },
                fault_policy,
                retain_save_interval: None,
            }),
        );

        let policy = runtime.watchdog_policy();
        assert!(policy.enabled);
        assert!(
            policy.timeout.as_nanos() > 0,
            "enabled watchdog timeout must not remain zero"
        );
    }
}

/// Applies a service or report command as [`apply_resource_command`] does, except that a runner
/// that refuses the mesh catalogue (`refuse_mesh_catalogue`, set by
/// [`ResourceRunner::refusing_mesh_catalogue`]) answers a `MeshCatalogue` request with its refusal.
fn apply_runner_command(
    runtime: &mut CompiledRuntime,
    refuse_mesh_catalogue: bool,
    command: ResourceCommand,
) {
    match command {
        ResourceCommand::MeshCatalogue { respond_to, .. } if refuse_mesh_catalogue => {
            let _ = respond_to.send(Err("mesh catalogue unavailable in debug control".to_string()));
        }
        other => apply_resource_command(runtime, other),
    }
}

fn apply_resource_command(runtime: &mut CompiledRuntime, command: ResourceCommand) {
    match command {
        ResourceCommand::Pause | ResourceCommand::Resume => {}
        // One command boundary applies the whole set: no cycle runs between the three updates.
        ResourceCommand::UpdateSchedulerPolicies(policies) => {
            runtime.set_watchdog_policy(policies.watchdog);
            runtime.set_fault_policy(policies.fault_policy);
            runtime.set_retain_save_interval(policies.retain_save_interval);
        }
        ResourceCommand::UpdateIoSafeState(state) => runtime.set_io_safe_state(state),
        ResourceCommand::ReloadBytecode {
            bytes,
            claim,
            respond_to,
        } => {
            // A withdrawn reload is skipped: its caller has stopped waiting for it.
            if claim.take() {
                let result = runtime.apply_online_change_bytes(&bytes);
                let _ = respond_to.send(result);
            }
        }
        ResourceCommand::MeshSnapshot { names, respond_to } => {
            // A published name may be a nested program or equipment path (`Main.P101.Running`,
            // `Main.Levels[2]`), kept as it was written (specification 39 §7 item 9).
            let snapshot = runtime.snapshot_globals(&names);
            let _ = respond_to.send(snapshot);
        }
        ResourceCommand::MeshCatalogue { names, respond_to } => {
            // The typed publish catalogue (specification 39 §7 item 9), read from the program as the
            // last scan left it, through the image's symbol and type tables (specification 44 §8.8).
            let result = runtime.with_program_view(|program| {
                crate::hmi::variable_tree::published_variables(program, &names)
            });
            let _ = respond_to.send(result);
        }
        ResourceCommand::MeshApply {
            updates,
            source: _,
            sequence: _,
        } => runtime.apply_mesh_updates(&updates),
        // A program's variables are read from its state through its symbol table (specification
        // 44 §8.8, §13.3).
        ResourceCommand::Snapshot { respond_to } => {
            let _ = respond_to.send(crate::debug::DebugSnapshot {
                storage: crate::memory::VariableStorage::new(),
                now: runtime.current_time(),
                compiled: runtime.snapshot().ok().map(std::sync::Arc::new),
            });
        }
        // Served by the scan loop, which owns the simulation; nothing answers them outside it.
        ResourceCommand::SimulationSnapshot { respond_to } => {
            let _ = respond_to.send(None);
        }
        ResourceCommand::SimulationTraceClear { respond_to, .. } => {
            let _ = respond_to.send(Err(
                crate::simulation::PlantTraceClearError::StatusUnavailable,
            ));
        }
        ResourceCommand::AdsStatus { respond_to } => {
            let _ = respond_to.send(runtime.ads_status_report());
        }
        ResourceCommand::OpcUaClientStatus { respond_to } => {
            let _ = respond_to.send(runtime.opcua_client_status_report());
        }
        ResourceCommand::ActiveAdsDevice {
            target,
            local,
            respond_to,
        } => {
            let _ = respond_to.send(runtime.active_ads_device_snapshot(&target, local.as_ref()));
        }
        // Only a stopped resource takes an install; the stopped wait notes it.
        ResourceCommand::ProgramInstalled => {}
        // Only the stopped wait runs a step; the control plane refuses one before it is queued
        // unless the program started meanwhile.
        ResourceCommand::StepCycle { respond_to, .. } => {
            let _ = respond_to.send(Err(StepRefusal::NotStopped(ResourceState::Running)));
        }
    }
}
