#[cfg(all(feature = "ethercat-wire", unix))]
const ETHERCAT_MAX_SUBDEVICES: usize = 64;
#[cfg(all(feature = "ethercat-wire", unix))]
const ETHERCAT_MAX_PDI: usize = 4096;
#[cfg(all(feature = "ethercat-wire", unix))]
const ETHERCAT_MAX_FRAMES: usize = 32;
#[cfg(all(feature = "ethercat-wire", unix))]
const ETHERCAT_MAX_PDU_DATA: usize = PduStorage::element_size(ETHERCAT_MAX_PDI);

#[cfg(all(feature = "ethercat-wire", unix))]
type EthercrabGroup = SubDeviceGroup<ETHERCAT_MAX_SUBDEVICES, ETHERCAT_MAX_PDI, DefaultLock, Op>;

#[cfg(all(feature = "ethercat-wire", unix))]
struct EthercrabBus {
    runtime: TokioRuntime,
    maindevice: Arc<MainDevice<'static>>,
    group: EthercrabGroup,
    transport_error: Arc<Mutex<Option<SmolStr>>>,
}

#[cfg(all(feature = "ethercat-wire", unix))]
impl EthercrabBus {
    fn new(config: &EthercatConfig) -> Result<Self, RuntimeError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|err| {
                RuntimeError::IoDriver(format!("ethercat tokio runtime init failed: {err}").into())
            })?;
        let storage = Box::leak(Box::new(PduStorage::<
            ETHERCAT_MAX_FRAMES,
            ETHERCAT_MAX_PDU_DATA,
        >::new()));
        let (tx, rx, pdu_loop) = storage
            .try_split()
            .map_err(|_| RuntimeError::IoDriver("ethercat PDU storage split failed".into()))?;

        let timeouts = Timeouts {
            pdu: config.timeout,
            state_transition: config.timeout.max(StdDuration::from_secs(1)),
            mailbox_response: config.timeout.max(StdDuration::from_millis(250)),
            ..Timeouts::default()
        };

        let maindevice = Arc::new(MainDevice::new(
            pdu_loop,
            timeouts,
            MainDeviceConfig::default(),
        ));
        let transport_error = Arc::new(Mutex::new(None));

        let tx_rx_future = tx_rx_task(config.adapter.as_str(), tx, rx).map_err(|err| {
            RuntimeError::IoDriver(
                format!("ethercat transport '{}' open failed: {err}", config.adapter).into(),
            )
        })?;
        let transport_error_ref = Arc::clone(&transport_error);
        runtime.spawn(async move {
            let message = match tx_rx_future.await {
                Ok(_) => SmolStr::new("ethercat transport loop exited"),
                Err(err) => SmolStr::new(format!("ethercat transport loop failed: {err}")),
            };
            let mut guard = transport_error_ref
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            *guard = Some(message);
        });

        let group = runtime
            .block_on(
                maindevice
                    .init_single_group::<ETHERCAT_MAX_SUBDEVICES, ETHERCAT_MAX_PDI>(ethercat_now),
            )
            .map_err(|err| {
                RuntimeError::IoDriver(
                    format!(
                        "ethercat discovery/init failed on '{}': {err}",
                        config.adapter
                    )
                    .into(),
                )
            })?;
        let group = runtime
            .block_on(group.into_op(maindevice.as_ref()))
            .map_err(|err| {
                RuntimeError::IoDriver(
                    format!(
                        "ethercat PRE-OP -> OP failed on '{}': {err}",
                        config.adapter
                    )
                    .into(),
                )
            })?;

        Ok(Self {
            runtime,
            maindevice,
            group,
            transport_error,
        })
    }

    fn check_transport_error(&self) -> Result<(), RuntimeError> {
        let guard = self
            .transport_error
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(message) = guard.as_ref() {
            return Err(RuntimeError::IoDriver(message.clone()));
        }
        Ok(())
    }

    fn tx_rx(&self) -> Result<(), RuntimeError> {
        self.check_transport_error()?;
        self.runtime
            .block_on(self.group.tx_rx(self.maindevice.as_ref()))
            .map(|_| ())
            .map_err(|err| {
                RuntimeError::IoDriver(format!("ethercat tx/rx failed: {err}").into())
            })?;
        self.check_transport_error()
    }

    fn collect_inputs(&self, bytes: usize) -> Vec<u8> {
        let mut data = Vec::with_capacity(bytes);
        for subdevice in self.group.iter(self.maindevice.as_ref()) {
            let io = subdevice.io_raw();
            data.extend_from_slice(io.inputs());
        }
        if data.len() < bytes {
            data.resize(bytes, 0);
        } else if data.len() > bytes {
            data.truncate(bytes);
        }
        data
    }

    fn write_outputs_to_pdi(&self, outputs: &[u8]) {
        let mut offset = 0usize;
        for subdevice in self.group.iter(self.maindevice.as_ref()) {
            let mut io = subdevice.io_raw_mut();
            let out = io.outputs();
            out.fill(0);
            if offset < outputs.len() {
                let copy_len = out.len().min(outputs.len() - offset);
                out[..copy_len].copy_from_slice(&outputs[offset..offset + copy_len]);
                offset += copy_len;
            }
        }
    }

    fn discovery_snapshot(&self) -> Result<EthercatDiscovery, RuntimeError> {
        let mut modules = Vec::new();
        let mut input_bytes = 0usize;
        let mut output_bytes = 0usize;
        for (slot, subdevice) in self.group.iter(self.maindevice.as_ref()).enumerate() {
            let io = subdevice.io_raw();
            input_bytes = input_bytes.saturating_add(io.inputs().len());
            output_bytes = output_bytes.saturating_add(io.outputs().len());
            let channels = (io.inputs().len().max(io.outputs().len()) * 8).max(1);
            modules.push(EthercatModuleConfig {
                model: SmolStr::new(subdevice.name()),
                slot: slot as u16,
                channels: channels.min(u16::MAX as usize) as u16,
            });
        }
        if modules.is_empty() {
            return Err(RuntimeError::IoDriver(
                "ethercat discovery found no subdevices".into(),
            ));
        }
        Ok(EthercatDiscovery {
            modules,
            input_bytes,
            output_bytes,
        })
    }
}

#[cfg(all(feature = "ethercat-wire", unix))]
impl EthercatBus for EthercrabBus {
    fn discover(&mut self, _config: &EthercatConfig) -> Result<EthercatDiscovery, RuntimeError> {
        self.tx_rx()?;
        self.discovery_snapshot()
    }

    fn read_inputs(&mut self, bytes: usize) -> Result<Vec<u8>, RuntimeError> {
        self.tx_rx()?;
        Ok(self.collect_inputs(bytes))
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.write_outputs_to_pdi(outputs);
        self.tx_rx()
    }
}
