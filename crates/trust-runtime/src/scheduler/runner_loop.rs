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

fn cycle_deadline_from(
    start: std::time::Instant,
    timeout: Duration,
) -> Option<std::time::Instant> {
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
    runtime: &mut Runtime,
    watchdog: crate::watchdog::WatchdogPolicy,
    wall_start: std::time::Instant,
) -> CycleDeadlineSnapshot {
    let snapshot = CycleDeadlineSnapshot {
        execution_deadline: runtime.execution_deadline(),
        output_deadline: runtime.output_commit_deadline(),
    };
    if watchdog.enabled {
        let cycle_deadline = cycle_deadline_from(wall_start, watchdog.timeout);
        runtime.set_execution_deadline(cycle_deadline);
        runtime.set_output_commit_deadline(cycle_deadline);
    }
    snapshot
}

fn restore_cycle_deadlines(runtime: &mut Runtime, snapshot: CycleDeadlineSnapshot) {
    runtime.set_execution_deadline(snapshot.execution_deadline);
    runtime.set_output_commit_deadline(snapshot.output_deadline);
}

const AUTOMATIC_RESTART_MAX_ATTEMPTS: u32 = 3;
const AUTOMATIC_RESTART_BACKOFF_FLOOR_NANOS: i64 = 1_000_000;
const AUTOMATIC_RESTART_BACKOFF_CAP_NANOS: i64 = 1_000_000_000;

fn run_resource_loop<C: Clock + Clone>(
    runner: ResourceRunner<C>,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
) {
    run_resource_loop_core(runner, stop, state, last_error, |runtime| {
        runtime.execute_cycle()
    });
}

fn run_resource_loop_with_shared<C: Clock + Clone>(
    runner: ResourceRunner<C>,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
    shared: SharedGlobals,
) {
    run_resource_loop_core(runner, stop, state, last_error, move |runtime| {
        shared.with_lock(|globals| {
            shared.sync_into_locked(globals, runtime)?;
            let result = runtime.execute_cycle();
            shared.sync_from_locked(globals, runtime)?;
            result
        })
    });
}

fn run_resource_loop_core<C, F>(
    mut runner: ResourceRunner<C>,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
    mut execute_cycle: F,
) where
    C: Clock + Clone,
    F: FnMut(&mut Runtime) -> Result<(), RuntimeError>,
{
    let mut paused = false;
    let mut automatic_restarts = AutomaticRestartLimiter::default();
    if let Some(gate) = runner.start_gate.as_ref() {
        set_resource_state(&state, ResourceState::Ready);
        if !gate.wait_open(&stop) {
            set_resource_state(&state, ResourceState::Stopped);
            return;
        }
    }
    set_resource_state(&state, ResourceState::Running);
    loop {
        if stop.load(Ordering::SeqCst) {
            let mut stop_error = None;
            if let Err(err) = runner.runtime.apply_io_safe_state() {
                stop_error = Some(err);
            }
            if let Err(err) = runner.runtime.save_retain_store() {
                if stop_error.is_none() {
                    stop_error = Some(err);
                }
            }
            if let Some(err) = stop_error {
                set_last_error(&last_error, err);
                set_resource_state(&state, ResourceState::Faulted);
            } else {
                set_resource_state(&state, ResourceState::Stopped);
            }
            break;
        }

        if let Some(commands) = runner.command_rx.as_ref() {
            while let Ok(command) = commands.try_recv() {
                match command {
                    ResourceCommand::Pause => {
                        paused = true;
                        set_resource_state(&state, ResourceState::Paused);
                    }
                    ResourceCommand::Resume => {
                        paused = false;
                        set_resource_state(&state, ResourceState::Running);
                    }
                    other => apply_resource_command(&mut runner.runtime, other),
                }
            }
        }

        let pending_restart = if let Some(signal) = runner.restart_signal.as_ref() {
            let mut guard = recover_mutex_lock(signal.lock());
            guard.take()
        } else {
            None
        };
        if let Some(mode) = pending_restart {
            if let Err(err) = runner.restart(mode) {
                set_last_error(&last_error, err);
                set_resource_state(&state, ResourceState::Faulted);
                break;
            }
            automatic_restarts.reset();
            if let Err(err) = runner.runtime.load_retain_store() {
                set_last_error(&last_error, err);
                set_resource_state(&state, ResourceState::Faulted);
                break;
            }
        }

        if paused {
            let now_raw = runner.clock.now();
            let interval = runner.cycle_interval.as_nanos();
            if interval <= 0 {
                thread::yield_now();
            } else {
                let sleep_interval =
                    scaled_sleep_interval(runner.cycle_interval, runner.time_scale);
                let deadline = Duration::from_nanos(
                    now_raw.as_nanos().saturating_add(sleep_interval.as_nanos()),
                );
                runner.clock.sleep_until(deadline);
            }
            continue;
        }

        let now_raw = runner.clock.now();
        let now = scaled_time(now_raw, runner.time_scale);
        runner.runtime.set_current_time(now);
        let wall_start = std::time::Instant::now();
        let watchdog = runner.runtime.watchdog_policy();
        let deadline_snapshot = arm_cycle_deadlines(&mut runner.runtime, watchdog, wall_start);
        let result = execute_protected_cycle(&mut runner, now, &mut execute_cycle);
        restore_cycle_deadlines(&mut runner.runtime, deadline_snapshot);
        if let Err(err) = result {
            if watchdog.enabled
                && matches!(
                    err,
                    RuntimeError::WatchdogTimeout | RuntimeError::ExecutionTimeout
                )
            {
                let watchdog_err = RuntimeError::WatchdogTimeout;
                if matches!(watchdog.action, crate::watchdog::WatchdogAction::Restart) {
                    if !try_automatic_restart(
                        &mut runner,
                        &mut automatic_restarts,
                        &watchdog_err,
                        &state,
                        &last_error,
                    ) {
                        break;
                    }
                    continue;
                }
                let err = runner.runtime.watchdog_timeout();
                set_last_error(&last_error, err);
                set_resource_state(&state, ResourceState::Faulted);
                break;
            }
            if matches!(
                runner.runtime.fault_policy(),
                crate::watchdog::FaultPolicy::Restart
            ) && !matches!(err, RuntimeError::ResourcePanic(_))
            {
                if !try_automatic_restart(
                    &mut runner,
                    &mut automatic_restarts,
                    &err,
                    &state,
                    &last_error,
                ) {
                    break;
                }
                continue;
            }
            set_last_error(&last_error, err);
            set_resource_state(&state, ResourceState::Faulted);
            break;
        }
        if watchdog.enabled {
            let elapsed = i64::try_from(wall_start.elapsed().as_nanos()).unwrap_or(i64::MAX);
            if elapsed > watchdog.timeout.as_nanos() {
                if matches!(watchdog.action, crate::watchdog::WatchdogAction::Restart) {
                    let err = RuntimeError::WatchdogTimeout;
                    if !try_automatic_restart(
                        &mut runner,
                        &mut automatic_restarts,
                        &err,
                        &state,
                        &last_error,
                    ) {
                        break;
                    }
                    continue;
                } else {
                    let err = runner.runtime.watchdog_timeout();
                    set_last_error(&last_error, err);
                    set_resource_state(&state, ResourceState::Faulted);
                    break;
                }
            }
        }
        automatic_restarts.reset();

        let interval = runner.cycle_interval.as_nanos();
        if interval <= 0 {
            thread::yield_now();
            continue;
        }
        let sleep_interval = scaled_sleep_interval(runner.cycle_interval, runner.time_scale);
        let deadline =
            Duration::from_nanos(now_raw.as_nanos().saturating_add(sleep_interval.as_nanos()));
        runner.clock.sleep_until(deadline);
    }
}

#[derive(Debug, Default)]
struct AutomaticRestartLimiter {
    attempts: u32,
}

impl AutomaticRestartLimiter {
    fn reset(&mut self) {
        self.attempts = 0;
    }

    fn next_attempt(&mut self, reason: &RuntimeError) -> Result<u32, RuntimeError> {
        if self.attempts >= AUTOMATIC_RESTART_MAX_ATTEMPTS {
            return Err(RuntimeError::RestartLimitExceeded {
                attempts: self.attempts,
                reason: SmolStr::new(reason.to_string()),
            });
        }
        self.attempts += 1;
        Ok(self.attempts)
    }
}

fn try_automatic_restart<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    automatic_restarts: &mut AutomaticRestartLimiter,
    reason: &RuntimeError,
    state: &Arc<Mutex<ResourceState>>,
    last_error: &Arc<Mutex<Option<RuntimeError>>>,
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
    apply_automatic_restart_backoff(runner, attempt);
    true
}

fn apply_automatic_restart_backoff<C: Clock + Clone>(
    runner: &ResourceRunner<C>,
    attempt: u32,
) {
    let backoff = automatic_restart_backoff(runner.cycle_interval, attempt);
    if backoff.as_nanos() <= 0 {
        thread::yield_now();
        return;
    }
    let deadline = Duration::from_nanos(runner.clock.now().as_nanos().saturating_add(backoff.as_nanos()));
    runner.clock.sleep_until(deadline);
}

fn automatic_restart_backoff(cycle_interval: Duration, attempt: u32) -> Duration {
    let base = cycle_interval
        .as_nanos()
        .clamp(
            AUTOMATIC_RESTART_BACKOFF_FLOOR_NANOS,
            AUTOMATIC_RESTART_BACKOFF_CAP_NANOS,
        );
    let multiplier = 1_i64 << attempt.saturating_sub(1).min(8);
    Duration::from_nanos(base.saturating_mul(multiplier).min(AUTOMATIC_RESTART_BACKOFF_CAP_NANOS))
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
    F: FnMut(&mut Runtime) -> Result<(), RuntimeError>,
{
    let mut sentinel = CyclePanicSentinel::entered();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(simulation) = runner.simulation.as_mut() {
            simulation.apply_pre_cycle(now, &mut runner.runtime)?;
        }
        let mut result = execute_cycle(&mut runner.runtime);
        if result.is_ok() {
            if let Some(simulation) = runner.simulation.as_mut() {
                result = simulation.apply_post_cycle(now, &runner.runtime);
            }
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
            Err(runner.runtime.resource_panic(panic_payload_message(payload)))
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
mod runner_loop_poison_tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct TestClock;

    impl Clock for TestClock {
        fn now(&self) -> Duration {
            Duration::ZERO
        }

        fn sleep_until(&self, _deadline: Duration) {}

        fn wake(&self) {}
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

    #[test]
    fn scheduler_arms_execution_deadline_during_cycle() {
        let mut runtime = Runtime::new();
        runtime.set_watchdog_policy(crate::watchdog::WatchdogPolicy {
            enabled: true,
            timeout: Duration::from_millis(10),
            action: crate::watchdog::WatchdogAction::Halt,
        });
        let runner = ResourceRunner::new(runtime, TestClock, Duration::from_millis(1));
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(ResourceState::Boot));
        let last_error = Arc::new(Mutex::new(None));
        let observed_deadline = Arc::new(Mutex::new(None));
        let observed_deadline_for_cycle = Arc::clone(&observed_deadline);

        run_resource_loop_core(runner, stop, state, last_error, move |runtime| {
            *observed_deadline_for_cycle
                .lock()
                .expect("observed deadline lock") = Some(runtime.execution_deadline());
            Err(RuntimeError::SimulationFault("stop after one cycle".into()))
        });

        assert!(
            observed_deadline
                .lock()
                .expect("observed deadline lock")
                .expect("cycle should run")
                .is_some(),
            "scheduler must arm Runtime::execution_deadline before cycle execution"
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
        let mut runtime = Runtime::new();
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
            let mut runtime = Runtime::new();
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
        let mut runtime = Runtime::new();
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
        let runtime = Runtime::new();
        let runner = ResourceRunner::new(runtime, TestClock, Duration::from_millis(1));
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(ResourceState::Boot));
        let last_error = Arc::new(Mutex::new(None));

        run_resource_loop_core(
            runner,
            stop,
            Arc::clone(&state),
            Arc::clone(&last_error),
            |_runtime| -> Result<(), RuntimeError> {
                panic!("intentional VM/program execution panic");
            },
        );

        assert_eq!(*recover_mutex_lock(state.lock()), ResourceState::Faulted);
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
    fn update_watchdog_zero_timeout_is_normalized_before_use() {
        let mut runtime = Runtime::new();

        apply_resource_command(
            &mut runtime,
            ResourceCommand::UpdateWatchdog(crate::watchdog::WatchdogPolicy {
                enabled: true,
                timeout: Duration::ZERO,
                action: crate::watchdog::WatchdogAction::Halt,
            }),
        );

        let policy = runtime.watchdog_policy();
        assert!(policy.enabled);
        assert!(
            policy.timeout.as_nanos() > 0,
            "enabled watchdog timeout must not remain zero"
        );
    }

    #[test]
    fn automatic_restart_limiter_escalates_after_retry_budget() {
        let mut limiter = AutomaticRestartLimiter::default();
        let reason = RuntimeError::IoTransport("persistent failure".into());

        assert_eq!(limiter.next_attempt(&reason).unwrap(), 1);
        assert_eq!(limiter.next_attempt(&reason).unwrap(), 2);
        assert_eq!(limiter.next_attempt(&reason).unwrap(), 3);
        let err = limiter
            .next_attempt(&reason)
            .expect_err("fourth persistent failure should escalate");

        assert!(matches!(
            err,
            RuntimeError::RestartLimitExceeded {
                attempts: 3,
                ..
            }
        ));
    }

    #[test]
    fn automatic_restart_backoff_is_bounded_and_monotonic() {
        let first = automatic_restart_backoff(Duration::from_millis(1), 1);
        let second = automatic_restart_backoff(Duration::from_millis(1), 2);
        let capped = automatic_restart_backoff(Duration::from_secs(60), 10);

        assert!(first.as_nanos() > 0);
        assert!(second >= first);
        assert_eq!(
            capped.as_nanos(),
            AUTOMATIC_RESTART_BACKOFF_CAP_NANOS
        );
    }
}

fn apply_resource_command(runtime: &mut Runtime, command: ResourceCommand) {
    match command {
        ResourceCommand::Pause | ResourceCommand::Resume => {}
        ResourceCommand::UpdateWatchdog(policy) => runtime.set_watchdog_policy(policy),
        ResourceCommand::UpdateFaultPolicy(policy) => runtime.set_fault_policy(policy),
        ResourceCommand::UpdateRetainSaveInterval(interval) => {
            runtime.set_retain_save_interval(interval)
        }
        ResourceCommand::UpdateIoSafeState(state) => runtime.set_io_safe_state(state),
        ResourceCommand::ReloadBytecode { bytes, respond_to } => {
            let result = runtime.apply_online_change_bytes(&bytes, None);
            let _ = respond_to.send(result);
        }
        ResourceCommand::MeshSnapshot { names, respond_to } => {
            let snapshot = runtime.snapshot_globals(&names);
            let _ = respond_to.send(snapshot);
        }
        ResourceCommand::MeshApply {
            updates,
            source: _,
            sequence: _,
        } => runtime.apply_mesh_updates(&updates),
        ResourceCommand::Snapshot { respond_to } => {
            let snapshot = crate::debug::DebugSnapshot {
                storage: runtime.storage().clone(),
                now: runtime.current_time(),
            };
            let _ = respond_to.send(snapshot);
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
            let snapshot = runtime.active_ads_device_snapshot(&target, local.as_ref());
            let _ = respond_to.send(snapshot);
        }
    }
}
