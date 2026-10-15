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
    /// A driver without outputs keeps every output condition `Ok` (specification 32 section 8).
    output_enabled: bool,
    /// Reported health: `input_health` combined with `output_health`, kept by `recombine_health`.
    health: IoDriverHealth,
    /// Input path: stale or missing input, decoding, and the link as an input read sees it.
    input_health: IoDriverHealth,
    /// Output path, link condition: "mqtt worker initializing" until the link is first observed,
    /// then link loss and reconnect backoff. Any operation that observes the link up clears it.
    output_link_health: IoDriverHealth,
    /// Output path, publish condition: typed-output preflight failure, handoff pending, and a
    /// publish error on a connected session. It starts `Ok`; only a completed successful publish
    /// clears it, and reads never touch it.
    output_publish_health: IoDriverHealth,
    /// Output path: the link condition combined with the publish condition (publish on a tie).
    output_health: IoDriverHealth,
}

impl MqttWorkerState {
    fn new(input_enabled: bool, output_enabled: bool) -> Self {
        let initializing = IoDriverHealth::Degraded {
            error: SmolStr::new("mqtt worker initializing"),
        };
        // A direction the configuration disables is always healthy (specification 32 section 8).
        let input_health = if input_enabled {
            initializing.clone()
        } else {
            IoDriverHealth::Ok
        };
        let output_link_health = if output_enabled {
            initializing
        } else {
            IoDriverHealth::Ok
        };
        let output_publish_health = IoDriverHealth::Ok;
        let output_health = combine(&output_link_health, &output_publish_health);
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
            output_enabled,
            health: combine(&input_health, &output_health),
            input_health,
            output_link_health,
            output_publish_health,
            output_health,
        }
    }

    /// Recomputes the output-path and the reported health after a condition changes.
    fn recombine_health(&mut self) {
        self.output_health = combine(&self.output_link_health, &self.output_publish_health);
        self.health = combine(&self.input_health, &self.output_health);
    }

    fn set_input_condition(&mut self, message: &str, policy: IoDriverErrorPolicy) {
        self.input_health = mqtt_policy_health(message, policy);
        self.last_error = Some(SmolStr::new(message));
        self.recombine_health();
    }

    /// The link condition. A driver without outputs has none, so it stays `Ok`.
    fn set_output_link(&mut self, condition: IoDriverHealth) {
        if self.output_enabled {
            self.output_link_health = condition;
            self.recombine_health();
        }
    }

    fn set_output_publish(&mut self, condition: IoDriverHealth) {
        self.output_publish_health = condition;
        self.recombine_health();
    }
}

impl MqttWorker {
    fn start(config: MqttIoConfig, factory: Arc<dyn MqttSessionFactory>) -> Self {
        let policy = config.on_error;
        let shared = Arc::new(Mutex::new(MqttWorkerState::new(
            config.input_enabled,
            config.output_enabled,
        )));
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

    /// A health condition under this driver's error policy.
    fn policy_health(&self, message: &str) -> IoDriverHealth {
        mqtt_policy_health(message, self.policy)
    }

    fn read_inputs(&self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let seq = {
            let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
            state.desired_input_len = inputs.len();
            state.input_request_seq = state.input_request_seq.wrapping_add(1);
            if Instant::now() < state.next_reconnect {
                state.completed_input_seq = state.input_request_seq;
                state.set_input_condition("mqtt reconnect backoff active", self.policy);
                state.set_output_link(self.policy_health("mqtt reconnect backoff active"));
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
                state.set_input_condition("mqtt input refresh pending", self.policy);
            }
            if state.completed_input_seq >= seq
                && let Some(error) = state.input_error.clone()
            {
                return Err(error);
            }
        }
        if let Some(result) = self.copy_latest_inputs(inputs) {
            return result;
        }

        let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
        if state.latest_inputs.is_none() {
            state.set_input_condition("mqtt input snapshot unavailable", self.policy);
        }
        mqtt_read_result_from_health(&state.input_health)
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
                state.set_output_link(self.policy_health("mqtt reconnect backoff active"));
            } else if !should_wait {
                state.set_output_publish(self.policy_health("mqtt output handoff pending"));
            }
            (seq, should_wait)
        };

        // A write reports the output path only: the link and publish conditions. Input conditions
        // never fail a handoff.
        if !should_wait {
            return mqtt_result_from_health(&self.output_health());
        }

        let deadline = Instant::now() + MQTT_WORKER_SCAN_WAIT_BOUND;
        loop {
            {
                let state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
                if state.completed_output_seq >= seq {
                    return mqtt_result_from_health(&state.output_health);
                }
            }
            if Instant::now() >= deadline {
                let mut state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
                state.set_output_publish(self.policy_health("mqtt output handoff pending"));
                return mqtt_result_from_health(&state.output_health);
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

    fn output_health(&self) -> IoDriverHealth {
        self.shared
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .output_health
            .clone()
    }

    fn copy_latest_inputs(&self, inputs: &mut [u8]) -> Option<Result<(), RuntimeError>> {
        let state = self.shared.lock().unwrap_or_else(|err| err.into_inner());
        let latest = state.latest_inputs.as_ref()?;
        let result = mqtt_read_result_from_health(&state.input_health);
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
        if reconnect_ready(&shared)
            && let Some((seq, outputs)) = take_mqtt_pending_output(&shared)
        {
            let result = driver.write_outputs(&outputs);
            let link_up = driver.link_connected();
            let health = driver.connection_health();
            let policy = driver.config.on_error;
            let mut state = shared.lock().unwrap_or_else(|err| err.into_inner());
            state.output_inflight = false;
            state.completed_output_seq = seq;
            let handoff_queued = state.pending_output.is_some();
            if link_up {
                // Observing the link up clears a link condition. The publish condition changes
                // only by this write's own outcome.
                state.set_output_link(IoDriverHealth::Ok);
                match &result {
                    // A completed publish clears the publish condition, unless a newer handoff is
                    // queued behind it: that handoff's pending condition stands until it completes.
                    Ok(()) if !handoff_queued => state.set_output_publish(IoDriverHealth::Ok),
                    Ok(()) => {}
                    // A typed-output preflight failure, or a publish error that left the session
                    // connected.
                    Err(error) => {
                        let condition = mqtt_policy_health(error.to_string(), policy);
                        state.set_output_publish(condition);
                    }
                }
            } else {
                // A publish error that dropped the session, or a failed connect, is a link loss.
                // Backoff follows it; a preflight failure on a connected link never does.
                let detail = match &result {
                    Err(error) => error.to_string(),
                    Ok(()) => link_failure_detail(&health).to_string(),
                };
                let condition = mqtt_policy_health(&detail, policy);
                state.set_output_link(condition);
                state.next_reconnect = Instant::now() + driver.config.reconnect;
                state.last_error = Some(SmolStr::new(detail));
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
            let health = driver.connection_health();
            let link_up = driver.link_connected();
            let policy = driver.config.on_error;
            let mut state = shared.lock().unwrap_or_else(|err| err.into_inner());
            if result.is_ok() && matches!(health, IoDriverHealth::Ok) {
                state.latest_inputs = Some(inputs);
                state.last_snapshot_at = Some(Instant::now());
                state.last_error = None;
            }
            if link_up {
                // The read's own outcome: fresh, or stale or undecodable under the policy. A read
                // that finds the link up also clears a link condition on the output path.
                state.input_health = health;
                state.set_output_link(IoDriverHealth::Ok);
            } else {
                // A lost link concerns outputs too, and it starts the reconnect backoff.
                let detail = link_failure_detail(&health);
                let condition = mqtt_policy_health(detail.as_str(), policy);
                state.input_health = condition.clone();
                state.set_output_link(condition);
                state.next_reconnect = Instant::now() + driver.config.reconnect;
                state.last_error = Some(detail);
            }
            state.recombine_health();
            state.input_error = input_error;
            state.completed_input_seq = seq;
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

/// The more severe of two conditions. On a tie `tie_winner` supplies the detail: the publish
/// condition within the output path, and the output path within the reported health.
fn combine(other: &IoDriverHealth, tie_winner: &IoDriverHealth) -> IoDriverHealth {
    if health_rank(other) > health_rank(tie_winner) {
        other.clone()
    } else {
        tie_winner.clone()
    }
}

fn health_rank(health: &IoDriverHealth) -> u8 {
    match health {
        IoDriverHealth::Ok => 0,
        IoDriverHealth::Degraded { .. } => 1,
        IoDriverHealth::Faulted { .. } => 2,
    }
}

/// The detail of a lost link: the driver's reported failure, or a generic disconnect when that
/// health already reads `Ok` because the link dropped after the driver last set it.
fn link_failure_detail(health: &IoDriverHealth) -> SmolStr {
    mqtt_health_error(health).unwrap_or_else(|| SmolStr::new("mqtt disconnected"))
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
