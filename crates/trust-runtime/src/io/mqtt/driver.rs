const MQTT_READY_TIMEOUT: StdDuration = StdDuration::from_millis(100);
const MQTT_READY_POLL: StdDuration = StdDuration::from_millis(1);

pub struct MqttIoDriver {
    config: MqttIoConfig,
    factory: Arc<dyn MqttSessionFactory>,
    session: Option<Box<dyn MqttSession>>,
    health: IoDriverHealth,
    next_reconnect: Instant,
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
        Ok(Self {
            config,
            factory,
            session: None,
            health: IoDriverHealth::Degraded {
                error: SmolStr::new("mqtt initializing"),
            },
            next_reconnect: Instant::now(),
        })
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

    fn take_fresh_payload(&mut self) -> Result<Vec<u8>, RuntimeError> {
        let deadline = Instant::now() + MQTT_READY_TIMEOUT;
        loop {
            let (payload, connected, last_error) = {
                let Some(session) = self.session.as_mut() else {
                    return Err(RuntimeError::IoFreshness("mqtt session unavailable".into()));
                };
                (
                    session.take_payload(),
                    session.is_connected(),
                    session.last_error(),
                )
            };
            if let Some(payload) = payload {
                return Ok(payload);
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
}

impl IoDriver for MqttIoDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        if let Err(err) = self.ensure_session() {
            return Err(RuntimeError::IoFreshness(err.to_string().into()));
        }
        let payload = self.take_fresh_payload()?;
        inputs.fill(0);
        for (dst, src) in inputs.iter_mut().zip(payload.iter()) {
            *dst = *src;
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
        self.ensure_session()?;
        if let Some(session) = self.session.as_mut() {
            if let Err(err) = session.publish(self.config.topic_out.as_str(), outputs) {
                self.set_degraded(err.to_string());
                self.session = None;
                self.next_reconnect = Instant::now() + self.config.reconnect;
                return Err(RuntimeError::IoTransport(err.to_string().into()));
            } else if session.is_connected() {
                self.health = IoDriverHealth::Ok;
            }
        } else {
            return Err(RuntimeError::IoTransport("mqtt session unavailable".into()));
        }
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        self.health.clone()
    }
}
