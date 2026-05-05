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
            let _ = runner.runtime.save_retain_store();
            set_resource_state(&state, ResourceState::Stopped);
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

        if let Some(signal) = runner.restart_signal.as_ref() {
            let mut guard = recover_mutex_lock(signal.lock());
            if let Some(mode) = guard.take() {
                if let Err(err) = runner.runtime.restart(mode) {
                    set_last_error(&last_error, err);
                    set_resource_state(&state, ResourceState::Faulted);
                    break;
                }
                if let Err(err) = runner.runtime.load_retain_store() {
                    set_last_error(&last_error, err);
                    set_resource_state(&state, ResourceState::Faulted);
                    break;
                }
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
        let previous_output_deadline = runner.runtime.output_commit_deadline();
        if watchdog.enabled {
            runner
                .runtime
                .set_output_commit_deadline(cycle_deadline_from(wall_start, watchdog.timeout));
        }
        if let Some(simulation) = runner.simulation.as_mut() {
            if let Err(err) = simulation.apply_pre_cycle(now, &mut runner.runtime) {
                runner
                    .runtime
                    .set_output_commit_deadline(previous_output_deadline);
                if matches!(
                    runner.runtime.fault_policy(),
                    crate::watchdog::FaultPolicy::Restart
                ) {
                    if let Err(restart_err) = runner.runtime.restart(crate::RestartMode::Warm) {
                        set_last_error(&last_error, restart_err);
                        set_resource_state(&state, ResourceState::Faulted);
                        break;
                    }
                    continue;
                }
                set_last_error(&last_error, err);
                set_resource_state(&state, ResourceState::Faulted);
                break;
            }
        }
        let mut result = execute_cycle(&mut runner.runtime);
        if result.is_ok() {
            if let Some(simulation) = runner.simulation.as_mut() {
                result = simulation.apply_post_cycle(now, &runner.runtime);
            }
        }
        runner
            .runtime
            .set_output_commit_deadline(previous_output_deadline);
        if let Err(err) = result {
            if matches!(err, RuntimeError::WatchdogTimeout)
                && matches!(watchdog.action, crate::watchdog::WatchdogAction::Restart)
            {
                if let Err(restart_err) = runner.runtime.restart(crate::RestartMode::Warm) {
                    set_last_error(&last_error, restart_err);
                    set_resource_state(&state, ResourceState::Faulted);
                    break;
                }
                continue;
            }
            if matches!(
                runner.runtime.fault_policy(),
                crate::watchdog::FaultPolicy::Restart
            ) {
                if let Err(restart_err) = runner.runtime.restart(crate::RestartMode::Warm) {
                    set_last_error(&last_error, restart_err);
                    set_resource_state(&state, ResourceState::Faulted);
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
                    if let Err(restart_err) = runner.runtime.restart(crate::RestartMode::Warm) {
                        set_last_error(&last_error, restart_err);
                        set_resource_state(&state, ResourceState::Faulted);
                        break;
                    }
                } else {
                    let err = runner.runtime.watchdog_timeout();
                    set_last_error(&last_error, err);
                    set_resource_state(&state, ResourceState::Faulted);
                    break;
                }
            }
        }

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
    }
}
