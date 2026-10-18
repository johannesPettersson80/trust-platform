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
type EthercrabPduStorage = PduStorage<ETHERCAT_MAX_FRAMES, ETHERCAT_MAX_PDU_DATA>;
/// An opened adapter's runtime, MainDevice and the transport loop's last error.
#[cfg(all(feature = "ethercat-wire", unix))]
type EthercrabTransport = (
    TokioRuntime,
    Arc<MainDevice<'static>>,
    Arc<Mutex<Option<SmolStr>>>,
);
/// One adapter's identification bus, cached for the whole process (O22).
#[cfg(all(feature = "ethercat-wire", unix))]
type CachedIdentificationBus = Arc<Mutex<Box<dyn EthercatBus>>>;

/// Which EtherCAT state an opened bus reaches (specification 11, "Safety"): identification maps the
/// process data in PRE-OP and never reaches SAFE-OP or OP, so no output changes; the driver's own
/// start-up keeps going to OP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EthercatBusState {
    /// PRE-OP with the process data mapped: identification only.
    PreOpPdi,
    /// OP: the driver's cyclic exchange.
    Op,
}

/// Opens `config`'s adapter at `state` (specification 11, "Safety"): the simulated bus answers in
/// software, identification maps the process data in PRE-OP, and only the driver's start-up opens
/// a hardware adapter in OP.
fn open_bus(
    config: &EthercatConfig,
    state: EthercatBusState,
) -> Result<Box<dyn EthercatBus>, RuntimeError> {
    if config.adapter.eq_ignore_ascii_case("mock") {
        return Ok(Box::new(MockEthercatBus::new(config)));
    }
    match state {
        EthercatBusState::PreOpPdi => open_identification_bus(config),
        EthercatBusState::Op => build_bus(config),
    }
}

/// Identifies the SubDevices of `config`'s adapter through `open` (specification 11, "Sources" and
/// "Safety").
///
/// `open` is the seam through which an identification obtains its bus; production opens the adapter
/// at [`EthercatBusState::PreOpPdi`] and never further, and a test replaces it with a fake that
/// records the state it was asked for.
fn identify_ethercat_subdevices(
    config: &EthercatConfig,
    open: impl Fn(&EthercatConfig, EthercatBusState) -> Result<Box<dyn EthercatBus>, RuntimeError>,
) -> Result<EthercatDiscovery, RuntimeError> {
    let mut bus = open(config, EthercatBusState::PreOpPdi)?;
    bus.discover(config)
}

/// Opens `config`'s adapter for identification (specification 11, "Safety"): the process data is
/// mapped in PRE-OP and the bus is never brought to SAFE-OP or OP, so no output changes. The bus is
/// the adapter's cached main device (O22), so a later scan re-enumerates the same bus rather than
/// opening another one. An adapter that cannot be opened is reported as unavailable.
fn open_identification_bus(config: &EthercatConfig) -> Result<Box<dyn EthercatBus>, RuntimeError> {
    #[cfg(all(feature = "ethercat-wire", unix))]
    {
        match cached_identification_bus(config, |adapter_config| {
            let bus: Box<dyn EthercatBus> = Box::new(EthercrabIdentifyBus::new(adapter_config)?);
            Ok(bus)
        }) {
            Ok(inner) => Ok(Box::new(SharedIdentificationBus { inner })),
            Err(error) => Ok(Box::new(UnavailableEthercrabBus::from_initial_error(
                config, error,
            ))),
        }
    }

    #[cfg(not(all(feature = "ethercat-wire", unix)))]
    {
        let _ = config;
        Err(RuntimeError::InvalidConfig(
            "ethercat hardware transport is only supported on unix targets in this build".into(),
        ))
    }
}

/// The one process-wide PDU storage of an adapter's identification bus (specification 11,
/// "EtherCAT unavailable-resource contract"; O22): it is allocated once and reused by every later
/// opening of that adapter, so repeated scans never leak another storage.
#[cfg(all(feature = "ethercat-wire", unix))]
fn adapter_pdu_storage(adapter: &str) -> &'static EthercrabPduStorage {
    static STORAGES: OnceLock<Mutex<HashMap<SmolStr, &'static EthercrabPduStorage>>> =
        OnceLock::new();
    let storages = STORAGES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = storages.lock().unwrap_or_else(|poison| poison.into_inner());
    if let Some(storage) = guard.get(adapter).copied() {
        return storage;
    }
    let storage: &'static EthercrabPduStorage = Box::leak(Box::new(EthercrabPduStorage::new()));
    guard.insert(SmolStr::new(adapter), storage);
    storage
}

/// The identification bus of one adapter (specification 11, "Sources"; O22): `open` runs once per
/// adapter and the bus — its runtime, its `MainDevice` with its PDU storage, and the transport
/// loop — stays alive for the whole process, so every later scan reuses it and re-enumerates the
/// bus instead of opening another one and leaking another storage.
#[cfg(all(feature = "ethercat-wire", unix))]
fn cached_identification_bus<F>(
    config: &EthercatConfig,
    open: F,
) -> Result<CachedIdentificationBus, RuntimeError>
where
    F: FnOnce(&EthercatConfig) -> Result<Box<dyn EthercatBus>, RuntimeError>,
{
    static BUSES: OnceLock<Mutex<HashMap<SmolStr, CachedIdentificationBus>>> = OnceLock::new();
    let buses = BUSES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = buses.lock().unwrap_or_else(|poison| poison.into_inner());
    if let Some(bus) = guard.get(config.adapter.as_str()) {
        return Ok(Arc::clone(bus));
    }
    let bus: CachedIdentificationBus = Arc::new(Mutex::new(open(config)?));
    guard.insert(config.adapter.clone(), Arc::clone(&bus));
    Ok(bus)
}

/// Assembles the SubDevices of a group and its mapped process data lengths (specification 11,
/// "Identity" and "Sources").
#[cfg(all(feature = "ethercat-wire", unix))]
fn snapshot_group<'group, 'maindevice, S: HasPdi>(
    group: &'group SubDeviceGroup<ETHERCAT_MAX_SUBDEVICES, ETHERCAT_MAX_PDI, DefaultLock, S>,
    maindevice: &'maindevice MainDevice<'maindevice>,
) -> Result<EthercatDiscovery, RuntimeError>
where
    'maindevice: 'group,
{
    let mut subdevices = Vec::new();
    let mut input_bytes = 0usize;
    let mut output_bytes = 0usize;
    for (slot, subdevice) in group.iter(maindevice).enumerate() {
        let io = subdevice.io_raw();
        let device_input_bytes = io.inputs().len();
        let device_output_bytes = io.outputs().len();
        input_bytes = input_bytes.saturating_add(device_input_bytes);
        output_bytes = output_bytes.saturating_add(device_output_bytes);
        let identity = subdevice.identity();
        subdevices.push(EthercatSubDevice {
            slot: slot as u16,
            configured_address: subdevice.configured_address(),
            alias_address: subdevice.alias_address(),
            name: SmolStr::new(subdevice.name()),
            channels: ((device_input_bytes.max(device_output_bytes) * 8).max(1))
                .min(u16::MAX as usize) as u16,
            vendor_id: identity.vendor_id,
            product_id: identity.product_id,
            revision: identity.revision,
            serial: identity.serial,
            input_bytes: device_input_bytes,
            output_bytes: device_output_bytes,
        });
    }
    if subdevices.is_empty() {
        return Err(RuntimeError::IoDriver(
            "ethercat discovery found no subdevices".into(),
        ));
    }
    Ok(EthercatDiscovery {
        subdevices,
        input_bytes,
        output_bytes,
    })
}

/// Builds a Tokio runtime with the adapter's transport loop and a MainDevice over `storage`
/// (specification 11, "EtherCAT unavailable-resource contract").
#[cfg(all(feature = "ethercat-wire", unix))]
fn open_main_device(
    config: &EthercatConfig,
    storage: &'static EthercrabPduStorage,
) -> Result<EthercrabTransport, RuntimeError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|err| {
            RuntimeError::IoDriver(format!("ethercat tokio runtime init failed: {err}").into())
        })?;
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

    Ok((runtime, maindevice, transport_error))
}

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
        // The driver's own start-up keeps its bytes for OP and its terminal failure rule (spec 11,
        // "EtherCAT unavailable-resource contract"): a failed driver instance is rebuilt, not
        // re-allocated, so it owns its storage rather than the identification cache.
        let storage: &'static EthercrabPduStorage = Box::leak(Box::new(EthercrabPduStorage::new()));
        let (runtime, maindevice, transport_error) = open_main_device(config, storage)?;
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
        snapshot_group(&self.group, self.maindevice.as_ref())
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

/// The bus of one adapter's identification, opened in PRE-OP with the process data mapped
/// (specification 11, "Safety" and "Sources"). It holds the adapter's `MainDevice` between scans
/// and re-enumerates the bus on every [`EthercatBus::discover`], so a device added after an earlier
/// scan is found by the next one. It never exchanges process data and never reaches SAFE-OP or OP.
#[cfg(all(feature = "ethercat-wire", unix))]
struct EthercrabIdentifyBus {
    runtime: TokioRuntime,
    maindevice: Arc<MainDevice<'static>>,
}

#[cfg(all(feature = "ethercat-wire", unix))]
impl EthercrabIdentifyBus {
    fn new(config: &EthercatConfig) -> Result<Self, RuntimeError> {
        let storage = adapter_pdu_storage(config.adapter.as_str());
        let (runtime, maindevice, _transport_error) = open_main_device(config, storage)?;
        Ok(Self {
            runtime,
            maindevice,
        })
    }

    /// Re-enumerates the adapter's bus, maps the process data in PRE-OP, and reads the devices of
    /// that scan (specification 11, "Sources" and "Safety"). The group is dropped here, so a later
    /// scan sees a device added since.
    fn discovery_snapshot(
        &self,
        config: &EthercatConfig,
    ) -> Result<EthercatDiscovery, RuntimeError> {
        let group = self
            .runtime
            .block_on(
                self.maindevice
                    .init_single_group::<ETHERCAT_MAX_SUBDEVICES, ETHERCAT_MAX_PDI>(ethercat_now),
            )
            .map_err(|err| {
                RuntimeError::IoDriver(
                    format!(
                        "ethercat identification init failed on '{}': {err}",
                        config.adapter
                    )
                    .into(),
                )
            })?;
        let group = self
            .runtime
            .block_on(group.into_pre_op_pdi(self.maindevice.as_ref()))
            .map_err(|err| {
                RuntimeError::IoDriver(
                    format!(
                        "ethercat identification PRE-OP PDI mapping failed on '{}': {err}",
                        config.adapter
                    )
                    .into(),
                )
            })?;
        snapshot_group(&group, self.maindevice.as_ref())
    }
}

#[cfg(all(feature = "ethercat-wire", unix))]
impl EthercatBus for EthercrabIdentifyBus {
    fn discover(&mut self, config: &EthercatConfig) -> Result<EthercatDiscovery, RuntimeError> {
        self.discovery_snapshot(config)
    }

    fn read_inputs(&mut self, _bytes: usize) -> Result<Vec<u8>, RuntimeError> {
        Err(RuntimeError::IoDriver(
            "ethercat identification reads no process data".into(),
        ))
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Err(RuntimeError::IoDriver(
            "ethercat identification writes no process data".into(),
        ))
    }
}

/// A bus handle over the shared identification bus of one adapter (specification 11, "Sources").
#[cfg(all(feature = "ethercat-wire", unix))]
struct SharedIdentificationBus {
    inner: CachedIdentificationBus,
}

#[cfg(all(feature = "ethercat-wire", unix))]
impl EthercatBus for SharedIdentificationBus {
    fn discover(&mut self, config: &EthercatConfig) -> Result<EthercatDiscovery, RuntimeError> {
        let mut bus = self
            .inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        bus.discover(config)
    }

    fn read_inputs(&mut self, _bytes: usize) -> Result<Vec<u8>, RuntimeError> {
        Err(RuntimeError::IoDriver(
            "ethercat identification reads no process data".into(),
        ))
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Err(RuntimeError::IoDriver(
            "ethercat identification writes no process data".into(),
        ))
    }
}
