use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverErrorPolicy, IoDriverHealth};

use super::{ModbusTcpConfig, ModbusTcpDriver};

const SCAN_WAIT_BOUND: StdDuration = StdDuration::from_millis(40);
const WORKER_IDLE_POLL: StdDuration = StdDuration::from_millis(1);
const RECONNECT_BACKOFF: StdDuration = StdDuration::from_millis(100);

#[derive(Debug)]
pub(super) struct ModbusWorker {
    shared: Arc<Mutex<ModbusWorkerState>>,
    shutdown: Arc<AtomicBool>,
    policy: IoDriverErrorPolicy,
    _handle: thread::JoinHandle<()>,
}

#[derive(Debug)]
struct ModbusWorkerState {
    desired_input_len: usize,
    input_request_seq: u64,
    completed_input_seq: u64,
    latest_inputs: Option<Vec<u8>>,
    last_snapshot_at: Option<Instant>,
    last_error: Option<SmolStr>,
    pending_output: Option<(u64, Vec<u8>)>,
    output_inflight: bool,
    next_output_seq: u64,
    completed_output_seq: u64,
    next_reconnect: Instant,
    health: IoDriverHealth,
}

impl ModbusWorkerState {
    fn new() -> Self {
        Self {
            desired_input_len: 0,
            input_request_seq: 0,
            completed_input_seq: 0,
            latest_inputs: None,
            last_snapshot_at: None,
            last_error: None,
            pending_output: None,
            output_inflight: false,
            next_output_seq: 0,
            completed_output_seq: 0,
            next_reconnect: Instant::now(),
            health: IoDriverHealth::Degraded {
                error: SmolStr::new("modbus worker initializing"),
            },
        }
    }
}

impl ModbusWorker {
    pub(super) fn start(config: ModbusTcpConfig) -> Self {
        let policy = config.on_error;
        let shared = Arc::new(Mutex::new(ModbusWorkerState::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shared = Arc::clone(&shared);
        let worker_shutdown = Arc::clone(&shutdown);
        let handle = thread::spawn(move || run_worker(config, worker_shared, worker_shutdown));
        Self {
            shared,
            shutdown,
            policy,
            _handle: handle,
        }
    }

    pub(super) fn read_inputs(&self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let seq = {
            let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
            state.desired_input_len = inputs.len();
            state.input_request_seq = state.input_request_seq.wrapping_add(1);
            state.input_request_seq
        };

        let deadline = Instant::now() + SCAN_WAIT_BOUND;
        loop {
            {
                let state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
                if state.completed_input_seq >= seq {
                    break;
                }
            }
            if Instant::now() >= deadline {
                break;
            }
            thread::sleep(WORKER_IDLE_POLL);
        }

        if let Some(result) = self.copy_latest_inputs(inputs) {
            return result;
        }

        let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
        if state.latest_inputs.is_none() {
            state.health = policy_health("modbus input snapshot unavailable", self.policy);
            state.last_error = Some(SmolStr::new("modbus input snapshot unavailable"));
        }
        result_from_health(&state.health)
    }

    pub(super) fn write_outputs(&self, outputs: &[u8]) -> Result<(), RuntimeError> {
        let (seq, should_wait) = {
            let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
            let should_wait = !state.output_inflight && state.pending_output.is_none();
            state.next_output_seq = state.next_output_seq.wrapping_add(1);
            let seq = state.next_output_seq;
            state.pending_output = Some((seq, outputs.to_vec()));
            if !should_wait {
                state.health = policy_health("modbus output handoff pending", self.policy);
                state.last_error = Some(SmolStr::new("modbus output handoff pending"));
            }
            (seq, should_wait)
        };

        if !should_wait {
            return result_from_health(&self.health());
        }

        let deadline = Instant::now() + SCAN_WAIT_BOUND;
        loop {
            {
                let state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
                if state.completed_output_seq >= seq {
                    return result_from_health(&state.health);
                }
            }
            if Instant::now() >= deadline {
                let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
                state.health = IoDriverHealth::Degraded {
                    error: SmolStr::new("modbus output handoff pending"),
                };
                state.last_error = Some(SmolStr::new("modbus output handoff pending"));
                return Ok(());
            }
            thread::sleep(WORKER_IDLE_POLL);
        }
    }

    pub(super) fn health(&self) -> IoDriverHealth {
        self.shared
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .health
            .clone()
    }

    fn copy_latest_inputs(&self, inputs: &mut [u8]) -> Option<Result<(), RuntimeError>> {
        let state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
        let latest = state.latest_inputs.as_ref()?;
        let len = inputs.len().min(latest.len());
        inputs[..len].copy_from_slice(&latest[..len]);
        if len < inputs.len() {
            inputs[len..].fill(0);
        }
        Some(result_from_health(&state.health))
    }
}

impl Drop for ModbusWorker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

fn run_worker(
    config: ModbusTcpConfig,
    shared: Arc<Mutex<ModbusWorkerState>>,
    shutdown: Arc<AtomicBool>,
) {
    let mut driver = ModbusTcpDriver::new_sync_for_worker(config);
    while !shutdown.load(Ordering::SeqCst) {
        if let Some((seq, outputs)) = take_pending_output(&shared) {
            let result = driver.write_outputs(&outputs);
            let health = driver.health();
            let mut state = shared.lock().unwrap_or_else(|err| err.into_inner());
            state.output_inflight = false;
            state.completed_output_seq = seq;
            state.health = health;
            if !matches!(state.health, IoDriverHealth::Ok) {
                state.next_reconnect = Instant::now() + RECONNECT_BACKOFF;
                state.last_error = health_error(&state.health);
            }
            if result.is_err() && matches!(state.health, IoDriverHealth::Ok) {
                state.health = IoDriverHealth::Degraded {
                    error: SmolStr::new("modbus output write failed"),
                };
                state.next_reconnect = Instant::now() + RECONNECT_BACKOFF;
                state.last_error = Some(SmolStr::new("modbus output write failed"));
            }
        }

        let input_request = {
            let state = shared.lock().unwrap_or_else(|err| err.into_inner());
            if state.input_request_seq > state.completed_input_seq {
                Some((state.input_request_seq, state.desired_input_len))
            } else {
                None
            }
        };
        if let Some((seq, input_len)) = input_request {
            let now = Instant::now();
            {
                let mut state = shared.lock().unwrap_or_else(|err| err.into_inner());
                if now < state.next_reconnect {
                    state.completed_input_seq = seq;
                    thread::sleep(WORKER_IDLE_POLL);
                    continue;
                }
            }
            let mut inputs = vec![0u8; input_len];
            let result = driver.read_inputs(&mut inputs);
            let health = driver.health();
            let mut state = shared.lock().unwrap_or_else(|err| err.into_inner());
            if result.is_ok() && matches!(health, IoDriverHealth::Ok) {
                state.latest_inputs = Some(inputs);
                state.last_snapshot_at = Some(Instant::now());
                state.last_error = None;
            }
            if !matches!(health, IoDriverHealth::Ok) {
                state.next_reconnect = Instant::now() + RECONNECT_BACKOFF;
                state.last_error = health_error(&health);
            }
            state.completed_input_seq = seq;
            state.health = health;
        }

        thread::sleep(WORKER_IDLE_POLL);
    }
}

fn take_pending_output(shared: &Arc<Mutex<ModbusWorkerState>>) -> Option<(u64, Vec<u8>)> {
    let mut state = shared.lock().unwrap_or_else(|err| err.into_inner());
    let pending = state.pending_output.take();
    if pending.is_some() {
        state.output_inflight = true;
    }
    pending
}

fn result_from_health(health: &IoDriverHealth) -> Result<(), RuntimeError> {
    match health {
        IoDriverHealth::Faulted { error } => Err(RuntimeError::IoTransport(error.clone())),
        IoDriverHealth::Ok | IoDriverHealth::Degraded { .. } => Ok(()),
    }
}

fn health_error(health: &IoDriverHealth) -> Option<SmolStr> {
    match health {
        IoDriverHealth::Ok => None,
        IoDriverHealth::Degraded { error } | IoDriverHealth::Faulted { error } => {
            Some(error.clone())
        }
    }
}

fn policy_health(message: &'static str, policy: IoDriverErrorPolicy) -> IoDriverHealth {
    match policy {
        IoDriverErrorPolicy::Fault => IoDriverHealth::Faulted {
            error: SmolStr::new(message),
        },
        IoDriverErrorPolicy::Warn | IoDriverErrorPolicy::Ignore => IoDriverHealth::Degraded {
            error: SmolStr::new(message),
        },
    }
}
