#[derive(Debug)]
struct MockEthercatBus {
    modules: Vec<EthercatModuleConfig>,
    input_frames: VecDeque<Vec<u8>>,
    latency: StdDuration,
    fail_read: bool,
    fail_write: bool,
    last_outputs: Vec<u8>,
}

impl MockEthercatBus {
    fn new(config: &EthercatConfig) -> Self {
        Self {
            modules: config.modules.clone(),
            input_frames: VecDeque::from(config.mock_inputs.clone()),
            latency: config.mock_latency,
            fail_read: config.mock_fail_read,
            fail_write: config.mock_fail_write,
            last_outputs: Vec::new(),
        }
    }
}

impl EthercatBus for MockEthercatBus {
    fn discover(&mut self, _config: &EthercatConfig) -> Result<EthercatDiscovery, RuntimeError> {
        // The simulated bus carries no hardware identity; its SubDevices are named by the
        // configured module chain (specification 11, "Comm.discover"). It is never presented as
        // hardware.
        let subdevices = self
            .modules
            .iter()
            .map(|module| {
                let (input_bits, output_bits) = module_io_bits(module);
                EthercatSubDevice {
                    slot: module.slot,
                    configured_address: module.slot,
                    alias_address: 0,
                    name: module.model.clone(),
                    channels: module.channels,
                    vendor_id: 0,
                    product_id: 0,
                    revision: 0,
                    serial: 0,
                    input_bytes: input_bits.div_ceil(8),
                    output_bytes: output_bits.div_ceil(8),
                }
            })
            .collect();
        let (input_bits, output_bits) =
            self.modules.iter().fold((0usize, 0usize), |acc, module| {
                let (input, output) = module_io_bits(module);
                (acc.0.saturating_add(input), acc.1.saturating_add(output))
            });
        Ok(EthercatDiscovery {
            subdevices,
            input_bytes: input_bits.div_ceil(8),
            output_bytes: output_bits.div_ceil(8),
        })
    }

    fn read_inputs(&mut self, bytes: usize) -> Result<Vec<u8>, RuntimeError> {
        if self.latency > StdDuration::ZERO {
            std::thread::sleep(self.latency);
        }
        if self.fail_read {
            return Err(RuntimeError::IoDriver("mock ethercat read failure".into()));
        }
        let mut data = self.input_frames.pop_front().unwrap_or_default();
        if !data.is_empty() {
            self.input_frames.push_back(data.clone());
        }
        if data.len() < bytes {
            data.resize(bytes, 0);
        } else if data.len() > bytes {
            data.truncate(bytes);
        }
        Ok(data)
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        if self.latency > StdDuration::ZERO {
            std::thread::sleep(self.latency);
        }
        if self.fail_write {
            return Err(RuntimeError::IoDriver("mock ethercat write failure".into()));
        }
        self.last_outputs.clear();
        self.last_outputs.extend_from_slice(outputs);
        Ok(())
    }
}
