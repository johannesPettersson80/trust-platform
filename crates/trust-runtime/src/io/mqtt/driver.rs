const MQTT_READY_TIMEOUT: StdDuration = StdDuration::from_millis(100);
const MQTT_READY_POLL: StdDuration = StdDuration::from_millis(1);

pub struct MqttIoDriver {
    config: MqttIoConfig,
    factory: Arc<dyn MqttSessionFactory>,
    session: Option<Box<dyn MqttSession>>,
    health: IoDriverHealth,
    next_reconnect: Instant,
    mapped_inputs: Vec<u8>,
    sparkplug_birth_published: bool,
    sparkplug_sequence: u64,
    worker: Option<MqttWorker>,
}

impl MqttIoDriver {
    pub fn from_params(value: &toml::Value) -> Result<Self, RuntimeError> {
        Self::from_params_with_factory(value, Arc::new(RumqttSessionFactory))
    }

    fn from_params_with_factory(
        value: &toml::Value,
        factory: Arc<dyn MqttSessionFactory>,
    ) -> Result<Self, RuntimeError> {
        let config = MqttIoConfig::from_params(value)?;
        let worker = Some(MqttWorker::start(config.clone(), Arc::clone(&factory)));
        Ok(Self::from_config(config, factory, worker))
    }

    fn new_sync_for_worker(config: MqttIoConfig, factory: Arc<dyn MqttSessionFactory>) -> Self {
        Self::from_config(config, factory, None)
    }

    fn from_config(
        config: MqttIoConfig,
        factory: Arc<dyn MqttSessionFactory>,
        worker: Option<MqttWorker>,
    ) -> Self {
        Self {
            config,
            factory,
            session: None,
            health: IoDriverHealth::Degraded {
                error: SmolStr::new("mqtt initializing"),
            },
            next_reconnect: Instant::now(),
            mapped_inputs: Vec::new(),
            sparkplug_birth_published: false,
            sparkplug_sequence: 0,
            worker,
        }
    }

    pub fn validate_params(value: &toml::Value) -> Result<(), RuntimeError> {
        let _ = MqttIoConfig::from_params(value)?;
        Ok(())
    }

    fn set_degraded(&mut self, message: impl AsRef<str>) {
        self.health = IoDriverHealth::Degraded {
            error: SmolStr::new(message.as_ref()),
        };
    }

    fn apply_error_policy(&mut self, err: RuntimeError) -> Result<(), RuntimeError> {
        let message = SmolStr::new(err.to_string());
        match self.config.on_error {
            IoDriverErrorPolicy::Fault => {
                self.health = IoDriverHealth::Faulted {
                    error: message.clone(),
                };
                Err(err)
            }
            IoDriverErrorPolicy::Warn | IoDriverErrorPolicy::Ignore => {
                self.health = IoDriverHealth::Degraded {
                    error: message.clone(),
                };
                Ok(())
            }
        }
    }

    fn ensure_session(&mut self) -> Result<(), RuntimeError> {
        let now = Instant::now();
        if let Some(session) = self.session.as_mut() {
            if session.is_connected() {
                self.health = IoDriverHealth::Ok;
                return Ok(());
            }
            if let Some(error) = session.last_error() {
                self.set_degraded(format!("mqtt disconnected: {error}"));
                if now < self.next_reconnect {
                    return Err(RuntimeError::IoTransport(
                        format!("mqtt disconnected: {error}").into(),
                    ));
                }
                self.session = None;
                self.sparkplug_birth_published = false;
            } else {
                self.set_degraded("mqtt connecting");
                return Err(RuntimeError::IoTransport("mqtt connecting".into()));
            }
        }

        if now < self.next_reconnect {
            return Err(RuntimeError::IoTransport("mqtt reconnect backoff active".into()));
        }
        let deadline = Instant::now() + MQTT_READY_TIMEOUT;
        let mut last_connect_error: Option<SmolStr>;
        loop {
            match self.factory.connect(&self.config) {
                Ok(session) => {
                    let session_deadline = Instant::now() + MQTT_READY_TIMEOUT;
                    while !session.is_connected() && Instant::now() < session_deadline {
                        if session.last_error().is_some() {
                            break;
                        }
                        thread::sleep(MQTT_READY_POLL);
                    }
                    let connected = session.is_connected();
                    let last_error = session.last_error();
                    self.session = Some(session);
                    if connected {
                        self.health = IoDriverHealth::Ok;
                        self.publish_sparkplug_birth_if_needed()?;
                        return Ok(());
                    }
                    if let Some(error) = last_error {
                        last_connect_error = Some(error);
                        self.session = None;
                        if Instant::now() < deadline {
                            thread::sleep(MQTT_READY_POLL);
                            continue;
                        }
                    } else {
                        self.set_degraded("mqtt connecting");
                        return Err(RuntimeError::IoTransport("mqtt connecting".into()));
                    }
                }
                Err(err) => {
                    self.session = None;
                    self.set_degraded(format!("mqtt connect failed: {err}"));
                    self.next_reconnect = now + self.config.reconnect;
                    return Err(RuntimeError::IoTransport(
                        format!("mqtt connect failed: {err}").into(),
                    ));
                }
            }
            let detail = last_connect_error
                .map_or_else(|| "mqtt connect timed out".to_string(), |err| err.to_string());
            self.set_degraded(format!("mqtt connect failed: {detail}"));
            self.next_reconnect = Instant::now() + self.config.reconnect;
            return Err(RuntimeError::IoTransport(
                format!("mqtt connect failed: {detail}").into(),
            ));
        }
    }

    fn take_fresh_payloads(&mut self) -> Result<Vec<MqttInboundPayload>, RuntimeError> {
        let deadline = Instant::now() + MQTT_READY_TIMEOUT;
        loop {
            let (payloads, connected, last_error) = {
                let Some(session) = self.session.as_mut() else {
                    return Err(RuntimeError::IoFreshness("mqtt session unavailable".into()));
                };
                (
                    session.take_payloads(),
                    session.is_connected(),
                    session.last_error(),
                )
            };
            if !payloads.is_empty() {
                return Ok(payloads);
            }
            if !connected {
                let detail =
                    last_error.map_or_else(|| "mqtt disconnected".to_string(), |err| err.to_string());
                self.set_degraded(&detail);
                return Err(RuntimeError::IoFreshness(detail.into()));
            }
            if Instant::now() >= deadline {
                self.set_degraded("mqtt input not fresh");
                return Err(RuntimeError::IoFreshness("mqtt input not fresh".into()));
            }
            thread::sleep(MQTT_READY_POLL);
        }
    }

    fn next_sparkplug_sequence(&mut self) -> u64 {
        let sequence = self.sparkplug_sequence % 256;
        self.sparkplug_sequence = (self.sparkplug_sequence + 1) % 256;
        sequence
    }

    fn publish_sparkplug_birth_if_needed(&mut self) -> Result<(), RuntimeError> {
        if self.sparkplug_birth_published || self.config.sparkplug.is_none() {
            return Ok(());
        }
        let sequence = self.next_sparkplug_sequence();
        let sparkplug = self
            .config
            .sparkplug
            .as_ref()
            .expect("sparkplug was checked above");
        let topic = sparkplug.nbirth_topic();
        let payload =
            encode_sparkplug_nbirth(sparkplug, &self.config.output_points, sequence);
        let result = if let Some(session) = self.session.as_mut() {
            session.publish(&topic, &payload)
        } else {
            Err(RuntimeError::IoTransport("mqtt session unavailable".into()))
        };
        if let Err(err) = result {
            self.set_degraded(err.to_string());
            self.session = None;
            self.sparkplug_birth_published = false;
            self.next_reconnect = Instant::now() + self.config.reconnect;
            return Err(RuntimeError::IoTransport(err.to_string().into()));
        }
        self.sparkplug_birth_published = true;
        Ok(())
    }
}

impl IoDriver for MqttIoDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        if let Some(worker) = &self.worker {
            return worker.read_inputs(inputs);
        }
        if let Err(err) = self.ensure_session() {
            return self.apply_error_policy(RuntimeError::IoFreshness(err.to_string().into()));
        }
        let payloads = match self.take_fresh_payloads() {
            Ok(payloads) => payloads,
            Err(err) => return self.apply_error_policy(err),
        };
        if self.config.input_points.is_empty() {
            let payload = &payloads
                .first()
                .expect("take_fresh_payloads only returns a non-empty vector")
                .payload;
            inputs.fill(0);
            for (dst, src) in inputs.iter_mut().zip(payload.iter()) {
                *dst = *src;
            }
        } else {
            let mut candidate = if self.mapped_inputs.len() == inputs.len() {
                self.mapped_inputs.clone()
            } else {
                vec![0; inputs.len()]
            };
            for inbound in &payloads {
                for point in self
                    .config
                    .input_points
                    .iter()
                    .filter(|point| point.topic.as_str() == inbound.topic.as_str())
                {
                    if let Err(err) = decode_mqtt_point(point, &inbound.payload, &mut candidate) {
                        return self.apply_error_policy(err);
                    }
                }
            }
            self.mapped_inputs = candidate;
            inputs.copy_from_slice(&self.mapped_inputs);
        }
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.is_connected())
        {
            self.health = IoDriverHealth::Ok;
        }
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        if let Some(worker) = &self.worker {
            return worker.write_outputs(outputs);
        }
        if let Err(err) = self.ensure_session() {
            return self.apply_error_policy(err);
        }
        let output_publications = if self.config.output_points.is_empty() {
            vec![(self.config.topic_out.clone(), outputs.to_vec())]
        } else if self.config.sparkplug.is_some() {
            let sequence = self.next_sparkplug_sequence();
            let sparkplug = self
                .config
                .sparkplug
                .as_ref()
                .expect("sparkplug was checked above");
            vec![(
                SmolStr::new(sparkplug.ndata_topic()),
                encode_sparkplug_ndata(&self.config.output_points, outputs, sequence)?,
            )]
        } else {
            self.config
                .output_points
                .iter()
                .map(|point| {
                    encode_mqtt_point(point, outputs).map(|payload| (point.topic.clone(), payload))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        if let Some(session) = self.session.as_mut() {
            for (topic, payload) in &output_publications {
                if let Err(err) = session.publish(topic.as_str(), payload) {
                    self.set_degraded(err.to_string());
                    self.session = None;
                    self.sparkplug_birth_published = false;
                    self.next_reconnect = Instant::now() + self.config.reconnect;
                    return self.apply_error_policy(RuntimeError::IoTransport(
                        err.to_string().into(),
                    ));
                }
            }
            if session.is_connected() {
                self.health = IoDriverHealth::Ok;
            }
        } else {
            return Err(RuntimeError::IoTransport("mqtt session unavailable".into()));
        }
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        if let Some(worker) = &self.worker {
            return worker.health();
        }
        self.health.clone()
    }
}
