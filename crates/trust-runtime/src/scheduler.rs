//! Resource scheduling utilities and clocks.

#![allow(missing_docs)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::value::Duration;
use crate::value::Value;
use crate::Runtime;
use crate::RuntimeMetadata;

/// Clock interface for resource scheduling.
pub trait Clock: Send + Sync + 'static {
    /// Return the current time for scheduling.
    fn now(&self) -> Duration;

    /// Sleep until the given deadline.
    fn sleep_until(&self, deadline: Duration);

    /// Wake any sleepers (best-effort).
    fn wake(&self) {
        // Default: no-op for clocks without a wait mechanism.
    }
}

/// Monotonic clock based on `std::time::Instant`.
#[derive(Debug, Clone)]
pub struct StdClock {
    start: std::time::Instant,
}

impl StdClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }
}

impl Default for StdClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for StdClock {
    fn now(&self) -> Duration {
        let elapsed = self.start.elapsed();
        let nanos = i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX);
        Duration::from_nanos(nanos)
    }

    fn sleep_until(&self, deadline: Duration) {
        let now = self.now();
        let delta = deadline.as_nanos() - now.as_nanos();
        if delta <= 0 {
            return;
        }
        let delta = u64::try_from(delta).unwrap_or(u64::MAX);
        thread::sleep(std::time::Duration::from_nanos(delta));
    }
}

/// Monotonic clock with simulation time acceleration.
#[derive(Debug, Clone)]
pub struct ScaledClock {
    start: std::time::Instant,
    scale: u32,
}

impl ScaledClock {
    #[must_use]
    pub fn new(scale: u32) -> Self {
        Self {
            start: std::time::Instant::now(),
            scale: scale.max(1),
        }
    }

    #[must_use]
    pub fn scale(&self) -> u32 {
        self.scale
    }
}

impl Clock for ScaledClock {
    fn now(&self) -> Duration {
        let elapsed = self.start.elapsed();
        let scaled_nanos = elapsed
            .as_nanos()
            .saturating_mul(self.scale as u128)
            .min(i64::MAX as u128);
        Duration::from_nanos(scaled_nanos as i64)
    }

    fn sleep_until(&self, deadline: Duration) {
        let now = self.now();
        let delta_sim = deadline.as_nanos().saturating_sub(now.as_nanos());
        if delta_sim <= 0 {
            return;
        }
        let scale = i64::from(self.scale.max(1));
        let delta_real = (delta_sim + scale - 1) / scale;
        if delta_real <= 0 {
            thread::yield_now();
            return;
        }
        let nanos = u64::try_from(delta_real).unwrap_or(u64::MAX);
        thread::sleep(std::time::Duration::from_nanos(nanos));
    }
}

#[derive(Debug)]
struct ManualClockState {
    now: Duration,
    sleep_calls: u64,
    interrupted: bool,
}

/// Deterministic clock for tests and simulations.
#[derive(Debug, Clone)]
pub struct ManualClock {
    inner: Arc<(Mutex<ManualClockState>, Condvar)>,
}

impl ManualClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new((
                Mutex::new(ManualClockState {
                    now: Duration::ZERO,
                    sleep_calls: 0,
                    interrupted: false,
                }),
                Condvar::new(),
            )),
        }
    }

    /// Return the current manual time.
    #[must_use]
    pub fn current_time(&self) -> Duration {
        let (lock, _) = &*self.inner;
        let state = lock.lock().expect("manual clock lock poisoned");
        state.now
    }

    /// Advance time by the given delta.
    pub fn advance(&self, delta: Duration) -> Duration {
        let (lock, cvar) = &*self.inner;
        let mut state = lock.lock().expect("manual clock lock poisoned");
        let next = state.now.as_nanos().saturating_add(delta.as_nanos());
        state.now = Duration::from_nanos(next);
        cvar.notify_all();
        state.now
    }

    /// Set the current time explicitly.
    pub fn set_time(&self, time: Duration) {
        let (lock, cvar) = &*self.inner;
        let mut state = lock.lock().expect("manual clock lock poisoned");
        state.now = time;
        cvar.notify_all();
    }

    /// Number of sleep calls issued to this clock.
    #[must_use]
    pub fn sleep_calls(&self) -> u64 {
        let (lock, _) = &*self.inner;
        let state = lock.lock().expect("manual clock lock poisoned");
        state.sleep_calls
    }

    /// Interrupt sleepers so they can exit.
    pub fn interrupt(&self) {
        let (lock, cvar) = &*self.inner;
        let mut state = lock.lock().expect("manual clock lock poisoned");
        state.interrupted = true;
        cvar.notify_all();
    }
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        self.current_time()
    }

    fn sleep_until(&self, deadline: Duration) {
        let (lock, cvar) = &*self.inner;
        let mut state = lock.lock().expect("manual clock lock poisoned");
        state.sleep_calls = state.sleep_calls.saturating_add(1);
        while !state.interrupted && state.now.as_nanos() < deadline.as_nanos() {
            state = cvar.wait(state).expect("manual clock wait poisoned");
        }
    }

    fn wake(&self) {
        self.interrupt();
    }
}

/// Resource execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourceState {
    #[default]
    Boot,
    Ready,
    Running,
    Paused,
    Faulted,
    Stopped,
}

/// Commands applied to a running resource.
#[derive(Debug, Clone)]
pub enum ResourceCommand {
    Pause,
    Resume,
    UpdateWatchdog(crate::watchdog::WatchdogPolicy),
    UpdateFaultPolicy(crate::watchdog::FaultPolicy),
    UpdateRetainSaveInterval(Option<Duration>),
    UpdateIoSafeState(crate::io::IoSafeState),
    ReloadBytecode {
        bytes: Vec<u8>,
        respond_to: std::sync::mpsc::Sender<Result<RuntimeMetadata, RuntimeError>>,
    },
    MeshSnapshot {
        names: Vec<SmolStr>,
        respond_to: std::sync::mpsc::Sender<IndexMap<SmolStr, Value>>,
    },
    MeshApply {
        updates: IndexMap<SmolStr, Value>,
    },
    Snapshot {
        respond_to: std::sync::mpsc::Sender<crate::debug::DebugSnapshot>,
    },
}

/// Gate that blocks resource execution until opened.
#[derive(Debug, Default)]
pub struct StartGate {
    open: Mutex<bool>,
    cvar: Condvar,
}

impl StartGate {
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: Mutex::new(false),
            cvar: Condvar::new(),
        }
    }

    pub fn open(&self) {
        let mut guard = self.open.lock().expect("start gate lock poisoned");
        *guard = true;
        self.cvar.notify_all();
    }

    fn wait_open(&self, stop: &AtomicBool) -> bool {
        let mut guard = self.open.lock().expect("start gate lock poisoned");
        while !*guard {
            if stop.load(Ordering::SeqCst) {
                return false;
            }
            let (next, _) = self
                .cvar
                .wait_timeout(guard, std::time::Duration::from_millis(50))
                .expect("start gate wait poisoned");
            guard = next;
        }
        true
    }
}

/// Drives a runtime with a scheduling clock.
#[derive(Debug)]
pub struct ResourceRunner<C: Clock + Clone> {
    runtime: Runtime,
    clock: C,
    cycle_interval: Duration,
    time_scale: u32,
    restart_signal: Option<Arc<Mutex<Option<crate::RestartMode>>>>,
    start_gate: Option<Arc<StartGate>>,
    command_rx: Option<std::sync::mpsc::Receiver<ResourceCommand>>,
    simulation: Option<crate::simulation::SimulationController>,
}

impl<C: Clock + Clone> ResourceRunner<C> {
    #[must_use]
    pub fn new(runtime: Runtime, clock: C, cycle_interval: Duration) -> Self {
        Self {
            runtime,
            clock,
            cycle_interval,
            time_scale: 1,
            restart_signal: None,
            start_gate: None,
            command_rx: None,
            simulation: None,
        }
    }

    /// Attach a restart signal for external control.
    #[must_use]
    pub fn with_restart_signal(mut self, signal: Arc<Mutex<Option<crate::RestartMode>>>) -> Self {
        self.restart_signal = Some(signal);
        self
    }

    /// Attach a start gate that must be opened before the scheduler runs.
    #[must_use]
    pub fn with_start_gate(mut self, gate: Arc<StartGate>) -> Self {
        self.start_gate = Some(gate);
        self
    }

    /// Apply simulation time acceleration (`>= 1`).
    #[must_use]
    pub fn with_time_scale(mut self, scale: u32) -> Self {
        self.time_scale = scale.max(1);
        self
    }

    /// Attach a simulation controller for coupling/disturbance hooks.
    #[must_use]
    pub fn with_simulation(mut self, simulation: crate::simulation::SimulationController) -> Self {
        self.simulation = Some(simulation);
        self
    }

    /// Access the underlying runtime.
    #[must_use]
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Mutate the underlying runtime.
    pub fn runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }

    /// Execute one cycle using the current clock time.
    pub fn tick(&mut self) -> Result<(), RuntimeError> {
        let now = self.clock.now();
        self.runtime.set_current_time(now);
        self.runtime.execute_cycle()
    }

    /// Execute one cycle with shared global synchronization.
    pub fn tick_with_shared(&mut self, shared: &SharedGlobals) -> Result<(), RuntimeError> {
        let now = self.clock.now();
        self.runtime.set_current_time(now);
        shared.with_lock(|globals| {
            shared.sync_into_locked(globals, &mut self.runtime)?;
            let result = self.runtime.execute_cycle();
            shared.sync_from_locked(globals, &self.runtime)?;
            result
        })
    }

    /// Spawn the runner in a dedicated OS thread.
    pub fn spawn(self, name: impl Into<String>) -> Result<ResourceHandle<C>, RuntimeError> {
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(ResourceState::Boot));
        let last_error = Arc::new(Mutex::new(None));
        let clock = self.clock.clone();
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let mut runner = self;
        runner.command_rx = Some(cmd_rx);

        let stop_thread = stop.clone();
        let state_thread = state.clone();
        let last_error_thread = last_error.clone();

        let (id_tx, id_rx) = std::sync::mpsc::channel();
        let builder = thread::Builder::new().name(name.into());
        let join = builder
            .spawn(move || {
                let _ = id_tx.send(thread::current().id());
                run_resource_loop(runner, stop_thread, state_thread, last_error_thread);
            })
            .map_err(|err| RuntimeError::ThreadSpawn(err.to_string().into()))?;

        let thread_id = id_rx.recv().unwrap_or_else(|_| join.thread().id());

        Ok(ResourceHandle {
            stop,
            state,
            last_error,
            thread_id,
            clock,
            join: Some(join),
            cmd_tx: cmd_tx.clone(),
        })
    }

    /// Spawn the runner with shared global synchronization.
    pub fn spawn_with_shared(
        self,
        name: impl Into<String>,
        shared: SharedGlobals,
    ) -> Result<ResourceHandle<C>, RuntimeError> {
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(ResourceState::Boot));
        let last_error = Arc::new(Mutex::new(None));
        let clock = self.clock.clone();
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let mut runner = self;
        runner.command_rx = Some(cmd_rx);

        let stop_thread = stop.clone();
        let state_thread = state.clone();
        let last_error_thread = last_error.clone();
        let shared_thread = shared.clone();

        let (id_tx, id_rx) = std::sync::mpsc::channel();
        let builder = thread::Builder::new().name(name.into());
        let join = builder
            .spawn(move || {
                let _ = id_tx.send(thread::current().id());
                run_resource_loop_with_shared(
                    runner,
                    stop_thread,
                    state_thread,
                    last_error_thread,
                    shared_thread,
                );
            })
            .map_err(|err| RuntimeError::ThreadSpawn(err.to_string().into()))?;

        let thread_id = id_rx.recv().unwrap_or_else(|_| join.thread().id());

        Ok(ResourceHandle {
            stop,
            state,
            last_error,
            thread_id,
            clock,
            join: Some(join),
            cmd_tx: cmd_tx.clone(),
        })
    }
}

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

fn run_resource_loop<C: Clock + Clone>(
    mut runner: ResourceRunner<C>,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
) {
    let mut paused = false;
    if let Some(gate) = runner.start_gate.as_ref() {
        *state.lock().expect("resource state poisoned") = ResourceState::Ready;
        if !gate.wait_open(&stop) {
            *state.lock().expect("resource state poisoned") = ResourceState::Stopped;
            return;
        }
    }
    *state.lock().expect("resource state poisoned") = ResourceState::Running;
    loop {
        if stop.load(Ordering::SeqCst) {
            let _ = runner.runtime.save_retain_store();
            *state.lock().expect("resource state poisoned") = ResourceState::Stopped;
            break;
        }

        if let Some(commands) = runner.command_rx.as_ref() {
            while let Ok(command) = commands.try_recv() {
                match command {
                    ResourceCommand::Pause => {
                        paused = true;
                        *state.lock().expect("resource state poisoned") = ResourceState::Paused;
                    }
                    ResourceCommand::Resume => {
                        paused = false;
                        *state.lock().expect("resource state poisoned") = ResourceState::Running;
                    }
                    other => apply_resource_command(&mut runner.runtime, other),
                }
            }
        }

        if let Some(signal) = runner.restart_signal.as_ref() {
            if let Ok(mut guard) = signal.lock() {
                if let Some(mode) = guard.take() {
                    if let Err(err) = runner.runtime.restart(mode) {
                        *last_error.lock().expect("resource error poisoned") = Some(err);
                        *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                        break;
                    }
                    if let Err(err) = runner.runtime.load_retain_store() {
                        *last_error.lock().expect("resource error poisoned") = Some(err);
                        *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                        break;
                    }
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
        if let Some(simulation) = runner.simulation.as_mut() {
            if let Err(err) = simulation.apply_pre_cycle(now, &mut runner.runtime) {
                if matches!(
                    runner.runtime.fault_policy(),
                    crate::watchdog::FaultPolicy::Restart
                ) {
                    if let Err(restart_err) = runner.runtime.restart(crate::RestartMode::Warm) {
                        *last_error.lock().expect("resource error poisoned") = Some(restart_err);
                        *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                        break;
                    }
                    continue;
                }
                *last_error.lock().expect("resource error poisoned") = Some(err);
                *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                break;
            }
        }
        let mut result = runner.runtime.execute_cycle();
        if result.is_ok() {
            if let Some(simulation) = runner.simulation.as_mut() {
                result = simulation.apply_post_cycle(now, &runner.runtime);
            }
        }
        if let Err(err) = result {
            if matches!(
                runner.runtime.fault_policy(),
                crate::watchdog::FaultPolicy::Restart
            ) {
                if let Err(restart_err) = runner.runtime.restart(crate::RestartMode::Warm) {
                    *last_error.lock().expect("resource error poisoned") = Some(restart_err);
                    *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                    break;
                }
                continue;
            }
            *last_error.lock().expect("resource error poisoned") = Some(err);
            *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
            break;
        }

        let watchdog = runner.runtime.watchdog_policy();
        if watchdog.enabled {
            let elapsed = i64::try_from(wall_start.elapsed().as_nanos()).unwrap_or(i64::MAX);
            if elapsed > watchdog.timeout.as_nanos() {
                if matches!(watchdog.action, crate::watchdog::WatchdogAction::Restart) {
                    if let Err(restart_err) = runner.runtime.restart(crate::RestartMode::Warm) {
                        *last_error.lock().expect("resource error poisoned") = Some(restart_err);
                        *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                        break;
                    }
                } else {
                    let err = runner.runtime.watchdog_timeout();
                    *last_error.lock().expect("resource error poisoned") = Some(err);
                    *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
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

fn run_resource_loop_with_shared<C: Clock + Clone>(
    mut runner: ResourceRunner<C>,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
    shared: SharedGlobals,
) {
    let mut paused = false;
    if let Some(gate) = runner.start_gate.as_ref() {
        *state.lock().expect("resource state poisoned") = ResourceState::Ready;
        if !gate.wait_open(&stop) {
            *state.lock().expect("resource state poisoned") = ResourceState::Stopped;
            return;
        }
    }
    *state.lock().expect("resource state poisoned") = ResourceState::Running;
    loop {
        if stop.load(Ordering::SeqCst) {
            let _ = runner.runtime.save_retain_store();
            *state.lock().expect("resource state poisoned") = ResourceState::Stopped;
            break;
        }

        if let Some(commands) = runner.command_rx.as_ref() {
            while let Ok(command) = commands.try_recv() {
                match command {
                    ResourceCommand::Pause => {
                        paused = true;
                        *state.lock().expect("resource state poisoned") = ResourceState::Paused;
                    }
                    ResourceCommand::Resume => {
                        paused = false;
                        *state.lock().expect("resource state poisoned") = ResourceState::Running;
                    }
                    other => apply_resource_command(&mut runner.runtime, other),
                }
            }
        }

        if let Some(signal) = runner.restart_signal.as_ref() {
            if let Ok(mut guard) = signal.lock() {
                if let Some(mode) = guard.take() {
                    if let Err(err) = runner.runtime.restart(mode) {
                        *last_error.lock().expect("resource error poisoned") = Some(err);
                        *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                        break;
                    }
                    if let Err(err) = runner.runtime.load_retain_store() {
                        *last_error.lock().expect("resource error poisoned") = Some(err);
                        *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                        break;
                    }
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
        if let Some(simulation) = runner.simulation.as_mut() {
            if let Err(err) = simulation.apply_pre_cycle(now, &mut runner.runtime) {
                if matches!(
                    runner.runtime.fault_policy(),
                    crate::watchdog::FaultPolicy::Restart
                ) {
                    if let Err(restart_err) = runner.runtime.restart(crate::RestartMode::Warm) {
                        *last_error.lock().expect("resource error poisoned") = Some(restart_err);
                        *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                        break;
                    }
                    continue;
                }
                *last_error.lock().expect("resource error poisoned") = Some(err);
                *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                break;
            }
        }
        let mut result = shared.with_lock(|globals| {
            shared.sync_into_locked(globals, &mut runner.runtime)?;
            let result = runner.runtime.execute_cycle();
            shared.sync_from_locked(globals, &runner.runtime)?;
            result
        });
        if result.is_ok() {
            if let Some(simulation) = runner.simulation.as_mut() {
                result = simulation.apply_post_cycle(now, &runner.runtime);
            }
        }
        if let Err(err) = result {
            if matches!(
                runner.runtime.fault_policy(),
                crate::watchdog::FaultPolicy::Restart
            ) {
                if let Err(restart_err) = runner.runtime.restart(crate::RestartMode::Warm) {
                    *last_error.lock().expect("resource error poisoned") = Some(restart_err);
                    *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                    break;
                }
                continue;
            }
            *last_error.lock().expect("resource error poisoned") = Some(err);
            *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
            break;
        }
        let watchdog = runner.runtime.watchdog_policy();
        if watchdog.enabled {
            let elapsed = i64::try_from(wall_start.elapsed().as_nanos()).unwrap_or(i64::MAX);
            if elapsed > watchdog.timeout.as_nanos() {
                if matches!(watchdog.action, crate::watchdog::WatchdogAction::Restart) {
                    if let Err(restart_err) = runner.runtime.restart(crate::RestartMode::Warm) {
                        *last_error.lock().expect("resource error poisoned") = Some(restart_err);
                        *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
                        break;
                    }
                } else {
                    let err = runner.runtime.watchdog_timeout();
                    *last_error.lock().expect("resource error poisoned") = Some(err);
                    *state.lock().expect("resource state poisoned") = ResourceState::Faulted;
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
            let result = runtime
                .apply_bytecode_bytes(&bytes, None)
                .and_then(|_| runtime.restart(crate::RestartMode::Warm))
                .and_then(|_| runtime.load_retain_store())
                .map(|_| runtime.metadata_snapshot());
            let _ = respond_to.send(result);
        }
        ResourceCommand::MeshSnapshot { names, respond_to } => {
            let snapshot = runtime.snapshot_globals(&names);
            let _ = respond_to.send(snapshot);
        }
        ResourceCommand::MeshApply { updates } => runtime.apply_mesh_updates(&updates),
        ResourceCommand::Snapshot { respond_to } => {
            let snapshot = crate::debug::DebugSnapshot {
                storage: runtime.storage().clone(),
                now: runtime.current_time(),
            };
            let _ = respond_to.send(snapshot);
        }
    }
}

/// Handle to a running resource thread.
#[derive(Debug)]
pub struct ResourceHandle<C: Clock + Clone> {
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
    thread_id: thread::ThreadId,
    clock: C,
    join: Option<thread::JoinHandle<()>>,
    cmd_tx: std::sync::mpsc::Sender<ResourceCommand>,
}

impl<C: Clock + Clone> ResourceHandle<C> {
    /// Cloneable control handle for external management.
    #[must_use]
    pub fn control(&self) -> ResourceControl<C> {
        ResourceControl {
            stop: self.stop.clone(),
            state: self.state.clone(),
            last_error: self.last_error.clone(),
            clock: self.clock.clone(),
            cmd_tx: self.cmd_tx.clone(),
        }
    }
    /// Signal the resource thread to stop.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.clock.wake();
    }

    /// Retrieve the last error if the resource faulted.
    #[must_use]
    pub fn last_error(&self) -> Option<RuntimeError> {
        self.last_error
            .lock()
            .expect("resource error poisoned")
            .clone()
    }

    /// Current resource state.
    #[must_use]
    pub fn state(&self) -> ResourceState {
        *self.state.lock().expect("resource state poisoned")
    }

    /// Thread id for the running resource.
    #[must_use]
    pub fn thread_id(&self) -> thread::ThreadId {
        self.thread_id
    }

    /// Join the resource thread.
    pub fn join(&mut self) -> thread::Result<()> {
        if let Some(join) = self.join.take() {
            return join.join();
        }
        Ok(())
    }
}

/// Lightweight control handle for a running resource.
#[derive(Debug, Clone)]
pub struct ResourceControl<C: Clock + Clone> {
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
    clock: C,
    cmd_tx: std::sync::mpsc::Sender<ResourceCommand>,
}

impl<C: Clock + Clone> ResourceControl<C> {
    /// Create a lightweight stub control with a command receiver.
    ///
    /// Intended for debug/control IPC where no scheduler thread is running.
    pub fn stub(clock: C) -> (Self, std::sync::mpsc::Receiver<ResourceCommand>) {
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(ResourceState::Ready));
        let last_error = Arc::new(Mutex::new(None));
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        (
            Self {
                stop,
                state,
                last_error,
                clock,
                cmd_tx,
            },
            cmd_rx,
        )
    }
    /// Pause the runtime cycles.
    pub fn pause(&self) -> Result<(), RuntimeError> {
        self.send_command(ResourceCommand::Pause)?;
        self.clock.wake();
        Ok(())
    }

    /// Resume the runtime cycles.
    pub fn resume(&self) -> Result<(), RuntimeError> {
        self.send_command(ResourceCommand::Resume)?;
        self.clock.wake();
        Ok(())
    }

    /// Signal the resource thread to stop.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.clock.wake();
    }

    /// Current resource state.
    #[must_use]
    pub fn state(&self) -> ResourceState {
        *self.state.lock().expect("resource state poisoned")
    }

    /// Retrieve the last error if the resource faulted.
    #[must_use]
    pub fn last_error(&self) -> Option<RuntimeError> {
        self.last_error
            .lock()
            .expect("resource error poisoned")
            .clone()
    }

    /// Send a command to the running resource.
    pub fn send_command(&self, command: ResourceCommand) -> Result<(), RuntimeError> {
        self.cmd_tx
            .send(command)
            .map_err(|_| RuntimeError::ControlError("command channel closed".into()))
    }
}

/// Shared global variables synchronized across multiple resources.
#[derive(Debug, Clone)]
pub struct SharedGlobals {
    names: Vec<SmolStr>,
    inner: Arc<Mutex<IndexMap<SmolStr, Value>>>,
}

impl SharedGlobals {
    /// Create a shared global set from a runtime snapshot.
    pub fn from_runtime(names: Vec<SmolStr>, runtime: &Runtime) -> Result<Self, RuntimeError> {
        let mut values = IndexMap::new();
        for name in &names {
            let value = runtime
                .storage()
                .get_global(name.as_ref())
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;
            values.insert(name.clone(), value.clone());
        }
        Ok(Self {
            names,
            inner: Arc::new(Mutex::new(values)),
        })
    }

    fn with_lock<T>(&self, f: impl FnOnce(&mut IndexMap<SmolStr, Value>) -> T) -> T {
        let mut guard = self.inner.lock().expect("shared globals poisoned");
        f(&mut guard)
    }

    /// Read a shared global value by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Value> {
        self.with_lock(|globals| globals.get(name).cloned())
    }

    fn sync_into_locked(
        &self,
        globals: &IndexMap<SmolStr, Value>,
        runtime: &mut Runtime,
    ) -> Result<(), RuntimeError> {
        for name in &self.names {
            let value = globals
                .get(name)
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;
            runtime
                .storage_mut()
                .set_global(name.clone(), value.clone());
        }
        Ok(())
    }

    fn sync_from_locked(
        &self,
        globals: &mut IndexMap<SmolStr, Value>,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        for name in &self.names {
            let value = runtime
                .storage()
                .get_global(name.as_ref())
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;
            globals.insert(name.clone(), value.clone());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, ScaledClock, StdClock};
    use crate::value::Duration;

    #[test]
    fn scaled_clock_clamps_zero_to_one() {
        let clock = ScaledClock::new(0);
        assert_eq!(clock.scale(), 1);
    }

    #[test]
    fn scaled_clock_sleeps_faster_than_std_clock() {
        let std_clock = StdClock::new();
        let std_deadline = Duration::from_millis(20);
        let std_start = std::time::Instant::now();
        std_clock.sleep_until(std_deadline);
        let std_elapsed = std_start.elapsed();

        let scaled_clock = ScaledClock::new(20);
        let scaled_deadline = Duration::from_millis(20);
        let scaled_start = std::time::Instant::now();
        scaled_clock.sleep_until(scaled_deadline);
        let scaled_elapsed = scaled_start.elapsed();

        assert!(std_elapsed >= std::time::Duration::from_millis(10));
        assert!(scaled_elapsed < std_elapsed);
    }

    #[test]
    fn scaled_clock_now_is_monotonic() {
        let clock = ScaledClock::new(10);
        let first = clock.now();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = clock.now();
        assert!(second.as_nanos() >= first.as_nanos());
    }
}
