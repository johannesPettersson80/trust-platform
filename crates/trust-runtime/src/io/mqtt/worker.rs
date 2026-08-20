const MQTT_WORKER_SCAN_WAIT_BOUND: StdDuration = StdDuration::from_millis(40);
const MQTT_WORKER_IDLE_POLL: StdDuration = StdDuration::from_millis(1);

struct MqttWorker {
    shared: Arc<Mutex<MqttWorkerState>>,
    shutdown: Arc<AtomicBool>,
    policy: IoDriverErrorPolicy,
    _handle: thread::JoinHandle<()>,
}

struct MqttWorkerState {
    desired_input_len: usize,
    input_request_seq: u64,
    completed_input_seq: u64,
    latest_inputs: Option<Vec<u8>>,
    last_snapshot_at: Option<Instant>,
    input_error: Option<RuntimeError>,
    last_error: Option<SmolStr>,
    pending_output: Option<(u64, Vec<u8>)>,
    output_inflight: bool,
    next_output_seq: u64,
    completed_output_seq: u64,
    next_reconnect: Instant,
    health: IoDriverHealth,
}

impl MqttWorkerState {
    fn new() -> Self {
        Self {
            desired_input_len: 0,
            input_request_seq: 0,
            completed_input_seq: 0,
            latest_inputs: None,
            last_snapshot_at: None,
            input_error: None,
            last_error: None,
            pending_output: None,
            output_inflight: false,
            next_output_seq: 0,
            completed_output_seq: 0,
            next_reconnect: Instant::now(),
            health: IoDriverHealth::Degraded {
                error: SmolStr::new("mqtt worker initializing"),
            },
        }
    }
}

impl MqttWorker {
    fn start(config: MqttIoConfig, factory: Arc<dyn MqttSessionFactory>) -> Self {
        let policy = config.on_error;
        let shared = Arc::new(Mutex::new(MqttWorkerState::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shared = Arc::clone(&shared);
        let worker_shutdown = Arc::clone(&shutdown);
        let handle =
            thread::spawn(move || run_mqtt_worker(config, factory, worker_shared, worker_shutdown));
        Self {
            shared,
            shutdown,
            policy,
            _handle: handle,
        }
    }

    fn read_inputs(&self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let seq = {
            let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
            state.desired_input_len = inputs.len();
            state.input_request_seq = state.input_request_seq.wrapping_add(1);
            if Instant::now() < state.next_reconnect {
                state.completed_input_seq = state.input_request_seq;
                state.health = mqtt_policy_health("mqtt reconnect backoff active", self.policy);
                state.last_error = Some(SmolStr::new("mqtt reconnect backoff active"));
            }
            state.input_request_seq
        };

        let deadline = Instant::now() + MQTT_WORKER_SCAN_WAIT_BOUND;
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
            thread::sleep(MQTT_WORKER_IDLE_POLL);
        }

        {
            let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
            if state.completed_input_seq < seq && state.latest_inputs.is_some() {
                state.health = mqtt_policy_health("mqtt input refresh pending", self.policy);
                state.last_error = Some(SmolStr::new("mqtt input refresh pending"));
            }
            if state.completed_input_seq >= seq {
                if let Some(error) = state.input_error.clone() {
                    return Err(error);
                }
            }
        }
        if let Some(result) = self.copy_latest_inputs(inputs) {
            return result;
        }

        let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
        if state.latest_inputs.is_none() {
            state.health = mqtt_policy_health("mqtt input snapshot unavailable", self.policy);
            state.last_error = Some(SmolStr::new("mqtt input snapshot unavailable"));
        }
        mqtt_read_result_from_health(&state.health)
    }

    fn write_outputs(&self, outputs: &[u8]) -> Result<(), RuntimeError> {
        let (seq, should_wait) = {
            let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
            let backoff_active = Instant::now() < state.next_reconnect;
            let should_wait =
                !backoff_active && !state.output_inflight && state.pending_output.is_none();
            state.next_output_seq = state.next_output_seq.wrapping_add(1);
            let seq = state.next_output_seq;
            state.pending_output = Some((seq, outputs.to_vec()));
            if backoff_active {
                state.health = mqtt_policy_health("mqtt reconnect backoff active", self.policy);
                state.last_error = Some(SmolStr::new("mqtt reconnect backoff active"));
            } else if !should_wait {
                state.health = mqtt_policy_health("mqtt output handoff pending", self.policy);
                state.last_error = Some(SmolStr::new("mqtt output handoff pending"));
            }
            (seq, should_wait)
        };

        if !should_wait {
            return mqtt_result_from_health(&self.health());
        }

        let deadline = Instant::now() + MQTT_WORKER_SCAN_WAIT_BOUND;
        loop {
            {
                let state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
                if state.completed_output_seq >= seq {
                    return mqtt_result_from_health(&state.health);
                }
            }
            if Instant::now() >= deadline {
                let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
                state.health = mqtt_policy_health("mqtt output handoff pending", self.policy);
                state.last_error = Some(SmolStr::new("mqtt output handoff pending"));
                return mqtt_result_from_health(&state.health);
            }
            thread::sleep(MQTT_WORKER_IDLE_POLL);
        }
    }

    fn health(&self) -> IoDriverHealth {
        self.shared
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .health
            .clone()
    }

    fn copy_latest_inputs(&self, inputs: &mut [u8]) -> Option<Result<(), RuntimeError>> {
        let state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
        let latest = state.latest_inputs.as_ref()?;
        let result = mqtt_read_result_from_health(&state.health);
        if result.is_err() {
            return Some(result);
        }
        let len = inputs.len().min(latest.len());
        inputs[..len].copy_from_slice(&latest[..len]);
        if len < inputs.len() {
            inputs[len..].fill(0);
        }
        Some(result)
    }
}

impl Drop for MqttWorker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

fn run_mqtt_worker(
    config: MqttIoConfig,
    factory: Arc<dyn MqttSessionFactory>,
    shared: Arc<Mutex<MqttWorkerState>>,
    shutdown: Arc<AtomicBool>,
) {
    let mut driver = MqttIoDriver::new_sync_for_worker(config, factory);
    while !shutdown.load(Ordering::SeqCst) {
        if reconnect_ready(&shared) {
            if let Some((seq, outputs)) = take_mqtt_pending_output(&shared) {
                let result = driver.write_outputs(&outputs);
                let health = driver.health();
                let mut state = shared.lock().unwrap_or_else(|err| err.into_inner());
                state.output_inflight = false;
                state.completed_output_seq = seq;
                state.health = health;
                if !matches!(state.health, IoDriverHealth::Ok) {
                    state.next_reconnect = Instant::now() + driver.config.reconnect;
                    state.last_error = mqtt_health_error(&state.health);
                }
                if let Err(error) = result {
                    if matches!(state.health, IoDriverHealth::Ok) {
                        state.health =
                            mqtt_policy_health(error.to_string(), driver.config.on_error);
                        state.next_reconnect = Instant::now() + driver.config.reconnect;
                        state.last_error = mqtt_health_error(&state.health);
                    }
                }
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
                    thread::sleep(MQTT_WORKER_IDLE_POLL);
                    continue;
                }
            }
            let mut inputs = vec![0u8; input_len];
            let result = driver.read_inputs(&mut inputs);
            let input_error = result.as_ref().err().cloned();
            let health = driver.health();
            let mut state = shared.lock().unwrap_or_else(|err| err.into_inner());
            if result.is_ok() && matches!(health, IoDriverHealth::Ok) {
                state.latest_inputs = Some(inputs);
                state.last_snapshot_at = Some(Instant::now());
                state.last_error = None;
            }
            if !matches!(health, IoDriverHealth::Ok) {
                state.next_reconnect = Instant::now() + driver.config.reconnect;
                state.last_error = mqtt_health_error(&health);
            }
            state.input_error = input_error;
            state.completed_input_seq = seq;
            state.health = health;
        }

        thread::sleep(MQTT_WORKER_IDLE_POLL);
    }
}

fn take_mqtt_pending_output(shared: &Arc<Mutex<MqttWorkerState>>) -> Option<(u64, Vec<u8>)> {
    let mut state = shared.lock().unwrap_or_else(|err| err.into_inner());
    let pending = state.pending_output.take();
    if pending.is_some() {
        state.output_inflight = true;
    }
    pending
}

fn reconnect_ready(shared: &Arc<Mutex<MqttWorkerState>>) -> bool {
    Instant::now()
        >= shared
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .next_reconnect
}

fn mqtt_result_from_health(health: &IoDriverHealth) -> Result<(), RuntimeError> {
    match health {
        IoDriverHealth::Faulted { error } => Err(RuntimeError::IoTransport(error.clone())),
        IoDriverHealth::Ok | IoDriverHealth::Degraded { .. } => Ok(()),
    }
}

fn mqtt_read_result_from_health(health: &IoDriverHealth) -> Result<(), RuntimeError> {
    match health {
        IoDriverHealth::Faulted { error } => Err(RuntimeError::IoFreshness(error.clone())),
        IoDriverHealth::Ok | IoDriverHealth::Degraded { .. } => Ok(()),
    }
}

fn mqtt_health_error(health: &IoDriverHealth) -> Option<SmolStr> {
    match health {
        IoDriverHealth::Ok => None,
        IoDriverHealth::Degraded { error } | IoDriverHealth::Faulted { error } => {
            Some(error.clone())
        }
    }
}

fn mqtt_policy_health(message: impl AsRef<str>, policy: IoDriverErrorPolicy) -> IoDriverHealth {
    match policy {
        IoDriverErrorPolicy::Fault => IoDriverHealth::Faulted {
            error: SmolStr::new(message.as_ref()),
        },
        IoDriverErrorPolicy::Warn | IoDriverErrorPolicy::Ignore => IoDriverHealth::Degraded {
            error: SmolStr::new(message.as_ref()),
        },
    }
}
