pub struct EthercatIoDriver {
    config: EthercatConfig,
    bus: Box<dyn EthercatBus>,
    health: IoDriverHealth,
    discovered: bool,
    discovery_message: SmolStr,
}

impl EthercatIoDriver {
    pub fn from_params(value: &toml::Value) -> Result<Self, RuntimeError> {
        let config = EthercatConfig::from_params(value)?;
        let bus = build_bus(&config)?;
        Ok(Self {
            config,
            bus,
            health: IoDriverHealth::Degraded {
                error: SmolStr::new("ethercat discovery pending"),
            },
            discovered: false,
            discovery_message: SmolStr::new("ethercat discovery pending"),
        })
    }

    pub fn validate_params(value: &toml::Value) -> Result<(), RuntimeError> {
        let _ = EthercatConfig::from_params(value)?;
        Ok(())
    }

    fn ensure_discovered(&mut self) -> Result<(), RuntimeError> {
        if self.discovered {
            return Ok(());
        }
        let discovery = self.bus.discover(&self.config).map_err(|err| {
            let message = SmolStr::new(format!("ethercat discover: {err}"));
            self.health = IoDriverHealth::Faulted {
                error: message.clone(),
            };
            RuntimeError::IoTransport(message)
        })?;
        let module_summary = discovery
            .modules
            .iter()
            .map(|module| format!("{}@{}", module.model, module.slot))
            .collect::<Vec<_>>()
            .join(", ");
        self.discovery_message = SmolStr::new(format!(
            "ethercat discovered [{}] on adapter '{}' (I={}B O={}B)",
            module_summary, self.config.adapter, discovery.input_bytes, discovery.output_bytes
        ));
        self.discovered = true;
        if discovery.input_bytes != self.config.expected_input_bytes
            || discovery.output_bytes != self.config.expected_output_bytes
        {
            let message = SmolStr::new(format!(
                "{}; config expects I={}B O={}B",
                self.discovery_message,
                self.config.expected_input_bytes,
                self.config.expected_output_bytes
            ));
            self.health = IoDriverHealth::Faulted {
                error: message.clone(),
            };
            return Err(RuntimeError::IoAddress(message));
        }
        self.health = IoDriverHealth::Ok;
        Ok(())
    }

    fn handle_io_error(&mut self, operation: &str, err: RuntimeError) -> Result<(), RuntimeError> {
        let message = SmolStr::new(format!("ethercat {operation}: {err}"));
        match self.config.on_error {
            IoDriverErrorPolicy::Fault => {
                self.health = IoDriverHealth::Faulted {
                    error: message.clone(),
                };
                Err(RuntimeError::IoTransport(message))
            }
            IoDriverErrorPolicy::Warn | IoDriverErrorPolicy::Ignore => {
                self.health = IoDriverHealth::Degraded {
                    error: message.clone(),
                };
                Ok(())
            }
        }
    }

    fn note_cycle_latency(&mut self, operation: &str, elapsed: StdDuration) {
        if elapsed > self.config.cycle_warn {
            if matches!(self.health, IoDriverHealth::Ok) {
                self.health = IoDriverHealth::Degraded {
                    error: SmolStr::new(format!(
                        "ethercat {operation} cycle {:.3}ms exceeded {:.3}ms",
                        elapsed.as_secs_f64() * 1000.0,
                        self.config.cycle_warn.as_secs_f64() * 1000.0
                    )),
                };
            }
        } else if self.discovered && matches!(self.health, IoDriverHealth::Ok) {
            self.health = IoDriverHealth::Ok;
        }
    }

    fn enforce_timing(
        &mut self,
        operation: &str,
        elapsed: StdDuration,
    ) -> Result<(), RuntimeError> {
        if elapsed > self.config.timeout {
            let err = RuntimeError::IoDriver(
                format!(
                    "ethercat {operation} timeout {:.3}ms exceeded {:.3}ms",
                    elapsed.as_secs_f64() * 1000.0,
                    self.config.timeout.as_secs_f64() * 1000.0
                )
                .into(),
            );
            return self.handle_io_error(operation, err);
        }
        self.note_cycle_latency(operation, elapsed);
        Ok(())
    }
}

impl IoDriver for EthercatIoDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        self.ensure_discovered()?;
        if inputs.len() < self.config.expected_input_bytes {
            let message = SmolStr::new(format!(
                "input image too small: got {}B, expected at least {}B",
                inputs.len(),
                self.config.expected_input_bytes
            ));
            self.health = IoDriverHealth::Faulted {
                error: message.clone(),
            };
            return Err(RuntimeError::IoAddress(message));
        }
        let start = Instant::now();
        match self.bus.read_inputs(inputs.len()) {
            Ok(data) => {
                let copy_len = inputs.len().min(data.len());
                inputs[..copy_len].copy_from_slice(&data[..copy_len]);
                self.enforce_timing("read", start.elapsed())
            }
            Err(err) => self.handle_io_error("read", err),
        }
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.ensure_discovered()?;
        if outputs.len() < self.config.expected_output_bytes {
            let message = SmolStr::new(format!(
                "output image too small: got {}B, expected at least {}B",
                outputs.len(),
                self.config.expected_output_bytes
            ));
            self.health = IoDriverHealth::Faulted {
                error: message.clone(),
            };
            return Err(RuntimeError::IoAddress(message));
        }
        let start = Instant::now();
        match self.bus.write_outputs(outputs) {
            Ok(()) => self.enforce_timing("write", start.elapsed()),
            Err(err) => self.handle_io_error("write", err),
        }
    }

    fn health(&self) -> IoDriverHealth {
        self.health.clone()
    }
}

fn build_bus(config: &EthercatConfig) -> Result<Box<dyn EthercatBus>, RuntimeError> {
    if config.adapter.eq_ignore_ascii_case("mock") {
        return Ok(Box::new(MockEthercatBus::new(config)));
    }

    #[cfg(all(feature = "ethercat-wire", unix))]
    {
        match EthercrabBus::new(config) {
            Ok(bus) => Ok(Box::new(bus)),
            Err(error) => Ok(Box::new(UnavailableEthercrabBus::from_initial_error(
                config, error,
            ))),
        }
    }

    #[cfg(all(feature = "ethercat-wire", not(unix)))]
    {
        let _ = config;
        Err(RuntimeError::InvalidConfig(
            "ethercat hardware transport is only supported on unix targets in this build".into(),
        ))
    }

    #[cfg(not(feature = "ethercat-wire"))]
    {
        let _ = config;
        Err(RuntimeError::InvalidConfig(
            "io.params.adapter requires feature 'ethercat-wire' for hardware transport".into(),
        ))
    }
}

#[cfg(all(feature = "ethercat-wire", unix))]
struct UnavailableEthercrabBus {
    config: EthercatConfig,
    error: SmolStr,
}

#[cfg(all(feature = "ethercat-wire", unix))]
impl UnavailableEthercrabBus {
    fn from_initial_error(config: &EthercatConfig, error: RuntimeError) -> Self {
        Self {
            config: config.clone(),
            error: SmolStr::new(error.to_string()),
        }
    }

    fn unavailable(&self) -> RuntimeError {
        RuntimeError::IoDriver(
            format!(
                "ethercat hardware unavailable until driver rebuild for '{}': {}",
                self.config.adapter, self.error
            )
            .into(),
        )
    }
}

#[cfg(all(feature = "ethercat-wire", unix))]
impl EthercatBus for UnavailableEthercrabBus {
    fn discover(&mut self, _config: &EthercatConfig) -> Result<EthercatDiscovery, RuntimeError> {
        Err(self.unavailable())
    }

    fn read_inputs(&mut self, bytes: usize) -> Result<Vec<u8>, RuntimeError> {
        let _ = bytes;
        Err(self.unavailable())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        let _ = outputs;
        Err(self.unavailable())
    }
}
