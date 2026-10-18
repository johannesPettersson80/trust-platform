//! The two compositions of the transcript parity test, both running `portability/f401/gpio.stbc`.
//!
//! The host composition is trust-runtime's threaded one, built as the in-process control tests
//! build it (`ControlState` field by field, a real `ResourceRunner` scan thread, requests through
//! the shared pipeline as `handle_request_line` sends them), with the runtime's metrics sink and
//! event sender wired as the product binary wires them (`run/runtime/entry.rs`).
//!
//! The device composition is the single-thread resource loop of `trust-runtime-services` on the
//! chips' shared engine cycle (`runner::engine`: the chips' admission limits, their execution
//! services and the cycle over the engine), with an in-memory link, a clock that the test sets,
//! process I/O without drivers, and a hardware watchdog that only counts its feeds.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use indexmap::IndexMap;
use serde_json::Value;
use smol_str::SmolStr;
use trust_runtime::bytecode::BytecodeModule;
use trust_runtime::config::{ControlMode, MeshRole};
use trust_runtime::control::{
    AdsDoctorJobStore, ControlState, HmiRuntimeDescriptor, SourceRegistry,
};
use trust_runtime::debug::{DebugVariableHandles, RuntimeEvent};
use trust_runtime::harness::CompileSession;
use trust_runtime::linux_rt::{LinuxRtConfig, LinuxRtRuntimeStatus};
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceHandle, ResourceRunner, ResourceState, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::ports::{
    ControlLink, HardwareWatchdog, LinkFrame, MonotonicClock, MonotonicInstant, PeerClass,
    PortError, RunnerWait, TimeSpan, WakeReason, WakeToken, WallClock,
};
use trust_runtime_core::value::Duration;
use trust_runtime_core::value::Value as RuntimeValue;
use trust_runtime_core::vm::PreparedModule;
use trust_runtime_services::access::{AccessRole, AuthFailure};
use trust_runtime_services::dispatch::prepare_line;
use trust_runtime_services::events::{store_event, EVENT_STORE_CAPACITY};
use trust_runtime_services::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use trust_runtime_services::live_values::queues::{
    ForcedSnapshot, ForcedVarTarget, PendingVarTarget,
};
use trust_runtime_services::live_values::var_requests::VariableQueuePort;
use trust_runtime_services::live_values::RequestQueueFull;
use trust_runtime_services::ports::{
    AccessPort, AuditRecord, AuditSink, ExtensionDispatch, StatusPort,
};
use trust_runtime_services::protocol::{ControlAfterResponse, ControlRequest, ControlResponse};
use trust_runtime_services::runner::engine::{
    device_limits, CycleSettings, DeviceMemory, DeviceServices, EngineCycle, ProcessIo,
};
use trust_runtime_services::runner::{EngineLiveValues, Runner, RunnerPlatform};
use trust_runtime_services::status::{
    absent_realtime_profile, disabled_openot_persistence, IoDriverStatus, StatusSettings,
};

/// The board probe: `led := button` in task `Scan` every 10 ms on resource `Controller`.
pub const GPIO: &[u8] = include_bytes!("../fixtures/portability/f401/gpio.stbc");
const RESOURCE: &str = "Controller";
const CYCLE_INTERVAL_MS: i64 = 10;
/// The cycle interval of both compositions.
pub const CYCLE_INTERVAL: StdDuration = StdDuration::from_millis(10);
const WAIT_LIMIT: StdDuration = StdDuration::from_secs(20);
/// The device link's frame: large enough that no reply of the transcript is refused.
const FRAME_LEN: usize = 64 * 1024;

/// One scan the host's scan thread completed: its logical start time and whether a task ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostScan {
    pub at: StdDuration,
    pub task: bool,
}

fn runtime_settings() -> RuntimeSettings {
    RuntimeSettings::new(
        Duration::from_millis(CYCLE_INTERVAL_MS),
        BaseSettings {
            log_level: SmolStr::new("info"),
            watchdog: WatchdogPolicy::default(),
            fault_policy: FaultPolicy::SafeHalt,
            retain_mode: RetainMode::None,
            retain_save_interval: None,
        },
        WebSettings {
            enabled: false,
            listen: SmolStr::new("127.0.0.1:0"),
            auth: SmolStr::new("local"),
            tls: false,
        },
        DiscoverySettings {
            enabled: false,
            service_name: SmolStr::new("truST"),
            advertise: false,
            interfaces: Vec::new(),
            host_group: None,
        },
        MeshSettings {
            enabled: false,
            role: MeshRole::Peer,
            listen: SmolStr::new("127.0.0.1:0"),
            connect: Vec::new(),
            tls: false,
            auth_token: None,
            publish: Vec::new(),
            subscribe: IndexMap::new(),
            zenohd_version: SmolStr::new("1.7.2"),
            plugin_versions: IndexMap::new(),
        },
        SimulationSettings {
            enabled: false,
            time_scale: 1,
            mode_label: SmolStr::new("production"),
            warning: SmolStr::new(""),
        },
    )
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let limit = Instant::now() + WAIT_LIMIT;
    while !done() {
        assert!(Instant::now() < limit, "timed out waiting for {what}");
        std::thread::sleep(StdDuration::from_millis(1));
    }
}

/// trust-runtime's threaded composition.
pub struct HostComposition {
    pub state: Arc<ControlState>,
    handle: ResourceHandle<StdClock>,
    runtime_events: Receiver<RuntimeEvent>,
    io_snapshots: Receiver<trust_runtime::io::IoSnapshot>,
    live_metrics: Arc<Mutex<RuntimeMetrics>>,
    scan_events: Vec<RuntimeEvent>,
    scan_task: bool,
    completed: u64,
    pub watchdog: WatchdogPolicy,
    pub fault_policy: FaultPolicy,
}

impl HostComposition {
    /// Installs the board probe from its STBC alone, as trust-runtime loads a project without
    /// sources, and starts the scan thread.
    pub fn start() -> Self {
        let module = BytecodeModule::decode(GPIO).expect("decode the board probe");
        let mut runtime = CompileSession::from_sources(Vec::new())
            .build_runtime_from_bytecode(&module, Some(RESOURCE))
            .expect("trust-runtime installs the board probe from its STBC");
        let debug = runtime.enable_debug();
        let live_metrics = Arc::new(Mutex::new(RuntimeMetrics::new()));
        runtime.set_metrics_sink(Arc::clone(&live_metrics));
        let io_health = Arc::new(Mutex::new(Vec::new()));
        runtime.set_io_health_sink(Some(Arc::clone(&io_health)));
        let (event_tx, runtime_events) = mpsc::channel();
        debug.set_runtime_sender(event_tx);
        // The product publishes the host's I/O snapshots to its control state from a logger
        // thread; the test moves them into the slot at each scan boundary instead, so `io.list`
        // and `io.read` answer the last completed scan exactly as the device's do.
        let (io_tx, io_snapshots) = mpsc::channel();
        debug.set_io_sender(io_tx);
        let watchdog = runtime.watchdog_policy();
        let fault_policy = runtime.fault_policy();
        let metadata = Arc::new(Mutex::new(runtime.metadata_snapshot()));
        let pending_restart = Arc::new(Mutex::new(None));
        let handle = ResourceRunner::new(
            runtime,
            StdClock::new(),
            Duration::from_millis(CYCLE_INTERVAL_MS),
        )
        .with_restart_signal(Arc::clone(&pending_restart))
        .spawn("transcript-host")
        .expect("spawn the scan thread");
        let sources = SourceRegistry::new(Vec::new());
        let hmi_descriptor = Arc::new(Mutex::new(HmiRuntimeDescriptor::from_sources(
            None, &sources,
        )));
        let state = ControlState {
            transfers: trust_runtime::control::new_transfer_session(),
            debug,
            resource: handle.control(),
            metadata,
            sources,
            io_snapshot: Arc::new(Mutex::new(None)),
            io_snapshot_seen_ms: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            pending_restart,
            auth_token: Arc::new(Mutex::new(None)),
            control_requires_auth: false,
            control_mode: Arc::new(Mutex::new(ControlMode::Production)),
            audit_tx: None,
            // The replies read the metrics the test publishes at scan boundaries.
            metrics: Arc::new(Mutex::new(RuntimeMetrics::new())),
            events: Arc::new(Mutex::new(VecDeque::new())),
            settings: Arc::new(Mutex::new(runtime_settings())),
            discovery: Arc::new(trust_runtime::discovery::DiscoveryState::new()),
            mesh_topology: Arc::new(Mutex::new(None)),
            realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
                LinuxRtConfig::default(),
            ))),
            web_listener_bound: Arc::new(AtomicBool::new(false)),
            opcua_server_bound: Arc::new(AtomicBool::new(false)),
            project_root: None::<PathBuf>,
            resource_name: SmolStr::new(RESOURCE),
            io_health,
            debug_enabled: Arc::new(AtomicBool::new(true)),
            debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
            hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
            hmi_persistence: None,
            hmi_descriptor,
            historian: None,
            openot_persistence_status: None,
            pairing: None,
            ads_doctor_jobs: Arc::new(Mutex::new(AdsDoctorJobStore::default())),
            ads_client_config: Arc::new(Mutex::new(None)),
            opcua_client_config: Arc::new(Mutex::new(None)),
            ads_server_config: Arc::new(Mutex::new(None)),
            #[cfg(feature = "ads-server")]
            ads_server_runtime: Arc::new(Mutex::new(None)),
        };
        Self {
            state: Arc::new(state),
            handle,
            runtime_events,
            io_snapshots,
            live_metrics,
            scan_events: Vec::new(),
            scan_task: false,
            completed: 0,
            watchdog,
            fault_policy,
        }
    }

    /// Sends one request line through the shared pipeline as the host's transport does: the
    /// reply, then the work after it.
    pub fn request(&self, line: &str) -> Value {
        let prepared = prepare_line(line, &*self.state, None);
        if prepared.after_response == Some(ControlAfterResponse::StopResource) {
            self.state.resource.stop();
        }
        serde_json::from_str(&prepared.response).expect("the host replies with one JSON object")
    }

    pub fn resource_state(&self) -> ResourceState {
        self.state.resource.state()
    }

    /// Waits until the scan thread reports `state`.
    pub fn wait_for_state(&self, state: ResourceState) {
        wait_for(&format!("the host to report {state:?}"), || {
            self.resource_state() == state
        });
    }

    /// Waits until the scan thread has taken the requested restart.
    pub fn wait_for_restart_taken(&self) {
        wait_for("the host to take the restart", || {
            self.state
                .pending_restart
                .lock()
                .expect("restart slot")
                .is_none()
        });
    }

    /// Moves the events of every scan the scan thread has completed into the event store, as
    /// the product's logger thread does, and publishes the metrics of exactly those scans, so a
    /// reply sees whole scans only. Returns the scans that completed since the last call.
    ///
    /// The metrics hold every completed scan when their count of recorded cycles, which unlike
    /// the 512-scan percentile window never stops counting, equals the scans completed.
    pub fn settle(&mut self) -> Vec<HostScan> {
        let limit = Instant::now() + WAIT_LIMIT;
        let mut completed = Vec::new();
        loop {
            while let Ok(event) = self.runtime_events.try_recv() {
                self.take(event, &mut completed);
            }
            if self.scan_events.is_empty() {
                let metrics = self.live_metrics.lock().expect("live metrics").clone();
                let recorded = metrics.cycle.samples();
                match self.runtime_events.try_recv() {
                    Err(TryRecvError::Empty) if recorded == self.completed => {
                        *self.state.metrics.lock().expect("published metrics") = metrics;
                        return completed;
                    }
                    Ok(event) => self.take(event, &mut completed),
                    Err(_) => {}
                }
            }
            assert!(
                Instant::now() < limit,
                "the host's scans did not settle: {} complete",
                self.completed
            );
            std::thread::sleep(StdDuration::from_millis(1));
        }
    }

    /// Settles until `enough` holds for the scans completed since this call began.
    pub fn settle_until(&mut self, mut enough: impl FnMut(&[HostScan]) -> bool) -> Vec<HostScan> {
        let limit = Instant::now() + WAIT_LIMIT;
        let mut scans = Vec::new();
        loop {
            scans.extend(self.settle());
            if enough(&scans) {
                return scans;
            }
            assert!(Instant::now() < limit, "the host did not scan as expected");
            std::thread::sleep(StdDuration::from_millis(1));
        }
    }

    fn take(&mut self, event: RuntimeEvent, completed: &mut Vec<HostScan>) {
        match &event {
            RuntimeEvent::CycleStart { .. } => {
                assert!(self.scan_events.is_empty(), "a scan started inside a scan");
                self.scan_task = false;
            }
            RuntimeEvent::TaskStart { .. } => self.scan_task = true,
            RuntimeEvent::TaskEnd { .. }
            | RuntimeEvent::TaskOverrun { .. }
            | RuntimeEvent::CycleEnd { .. } => {}
            other => panic!("the transcript's host emitted an unexpected event: {other:?}"),
        }
        let end = matches!(event, RuntimeEvent::CycleEnd { .. });
        self.scan_events.push(event);
        if !end {
            return;
        }
        let Some(RuntimeEvent::CycleStart { time, .. }) = self.scan_events.first() else {
            panic!("a scan ended that never started: {:?}", self.scan_events);
        };
        completed.push(HostScan {
            at: StdDuration::from_nanos(u64::try_from(time.as_nanos()).expect("scan time")),
            task: self.scan_task,
        });
        let mut store = self.state.events.lock().expect("event store");
        for event in self.scan_events.drain(..) {
            store_event(&mut store, event, EVENT_STORE_CAPACITY);
        }
        self.completed += 1;
        // The scan's I/O snapshot is pushed before its `cycle_end`, so draining here publishes
        // exactly the last completed scan, as the product's snapshot thread does a scan late.
        let mut latest = None;
        while let Ok(snapshot) = self.io_snapshots.try_recv() {
            latest = Some(snapshot);
        }
        if let Some(snapshot) = latest {
            *self.state.io_snapshot.lock().expect("io snapshot slot") = Some(snapshot);
        }
    }

    /// Waits for the stopped scan thread to end.
    pub fn join(mut self) {
        self.handle.stop();
        self.handle
            .join()
            .expect("the scan thread ends without panicking");
    }
}

/// The loop's clock; the test sets it.
#[derive(Clone, Default)]
pub struct ReplayClock(Rc<Cell<StdDuration>>);

impl ReplayClock {
    pub fn set(&self, now: StdDuration) {
        self.0.set(now);
    }
}

impl MonotonicClock for ReplayClock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(self.0.get())
    }
}

/// The loop's wait: the test owns the time, so a wait returns at once.
#[derive(Clone, Default)]
pub struct ReplayWait {
    wakes: Rc<Cell<u32>>,
}

impl RunnerWait for ReplayWait {
    fn wake_token(&self) -> WakeToken {
        WakeToken::new(self.wakes.get())
    }

    fn wait_until(
        &mut self,
        _deadline: MonotonicInstant,
        _observed: WakeToken,
    ) -> Result<WakeReason, PortError> {
        Ok(WakeReason::Deadline)
    }
}

/// The in-memory link: one local client, as a client on the device's USB.
#[derive(Clone, Default)]
pub struct MemoryLink {
    requests: Rc<RefCell<VecDeque<Vec<u8>>>>,
    replies: Rc<RefCell<Vec<String>>>,
}

impl MemoryLink {
    pub fn send(&self, line: &str) {
        self.requests
            .borrow_mut()
            .push_back(line.as_bytes().to_vec());
    }

    pub fn take_replies(&self) -> Vec<Value> {
        self.replies
            .borrow_mut()
            .drain(..)
            .map(|reply| serde_json::from_str(&reply).expect("the loop replies with JSON"))
            .collect()
    }
}

impl ControlLink for MemoryLink {
    fn max_frame_len(&self) -> usize {
        FRAME_LEN
    }

    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        let frame = self.requests.borrow_mut().pop_front()?;
        let peer = PeerClass::Local;
        let Some(target) = buffer.get_mut(..frame.len()) else {
            return Some(LinkFrame::Oversized { peer });
        };
        target.copy_from_slice(&frame);
        Some(LinkFrame::Frame {
            len: frame.len(),
            peer,
        })
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        let reply = String::from_utf8(frame.to_vec()).expect("UTF-8 reply");
        self.replies.borrow_mut().push(reply);
        Ok(())
    }
}

/// The chips' application region and heap today, 16 KiB and 72 KiB, whose admission limits the
/// device composition prepares the board probe with.
const CHIP_MEMORY: DeviceMemory = DeviceMemory {
    region_bytes: 16 * 1024,
    heap_bytes: 72 * 1024,
};

/// The board probe, admitted as the chips admit programs.
pub fn prepare_board_probe() -> PreparedModule {
    PreparedModule::from_bytes(GPIO, device_limits(CHIP_MEMORY))
        .expect("the chips admit the board probe")
}

/// The device's process I/O. The board probe's input and output are wired to nothing, as the host
/// composition has no I/O driver; the I/O counts the scans by their input reads.
pub struct ReplayIo {
    scans: Rc<Cell<usize>>,
}

impl ProcessIo for ReplayIo {
    fn read_inputs(&mut self, _image: &mut [u8]) -> Result<(), RuntimeError> {
        self.scans.set(self.scans.get() + 1);
        Ok(())
    }

    fn write_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_safe_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        Vec::new()
    }
}

/// The board probe on the chips' shared engine cycle.
pub type DeviceCycle<'a> = EngineCycle<'a, ReplayClock, ReplayIo>;

/// The board probe of `prepared` started on `services`, with the host composition's policies
/// and, as the host composition, no `[[io.safe_state]]`; `scans` counts its scans.
pub fn start_device_cycle<'a>(
    prepared: &'a PreparedModule,
    services: &'a DeviceServices<ReplayClock>,
    scans: Rc<Cell<usize>>,
    watchdog: WatchdogPolicy,
    fault_policy: FaultPolicy,
) -> DeviceCycle<'a> {
    let settings = CycleSettings {
        cycle_interval: Duration::from_millis(CYCLE_INTERVAL_MS),
        watchdog,
        fault_policy,
        safe_outputs: Vec::new(),
    };
    EngineCycle::start(prepared, services, ReplayIo { scans }, settings)
        .expect("the board probe starts on the chips' engine cycle")
}

/// The declared name of every admitted process-image binding, in `prepared.io_bindings()` order,
/// as trust-runtime's hosted snapshot names it.
pub fn binding_names(prepared: &PreparedModule) -> Vec<Option<SmolStr>> {
    prepared
        .io_bindings()
        .iter()
        .map(|binding| {
            Some(
                prepared
                    .persistent_reference_name(binding.ref_idx)
                    .expect("an admitted binding has a declared name"),
            )
        })
        .collect()
}

/// The device's platform. Its configuration is the host composition's, so the values the
/// platform knows (settings, I/O drivers, resource name, credentials) are the same in both
/// compositions. Like a chip, it has no realtime profile and no `OpenOT` persistence and reports
/// them with the shared blocks, which the host reports for its own; it audits and dispatches on
/// its own.
pub struct BoardPlatform {
    host: Arc<ControlState>,
    audits: Cell<u64>,
    /// The device's own Live Values: the queues the runner applies and the snapshot it publishes,
    /// so the device writes, forces and releases as trust-runtime does instead of borrowing the
    /// host's answers.
    live: Rc<EngineLiveValues>,
}

impl BoardPlatform {
    pub fn configured_as(host: &HostComposition, live: Rc<EngineLiveValues>) -> Self {
        Self {
            host: Arc::clone(&host.state),
            audits: Cell::new(0),
            live,
        }
    }
}

impl AccessPort for BoardPlatform {
    fn request_role(
        &self,
        request: &ControlRequest,
        client: Option<&str>,
    ) -> Result<AccessRole, AuthFailure> {
        self.host.request_role(request, client)
    }

    fn debug_enabled(&self) -> bool {
        AccessPort::debug_enabled(&*self.host)
    }
}

impl AuditSink for BoardPlatform {
    fn record_audit(&self, _record: AuditRecord<'_>) -> Option<SmolStr> {
        self.audits.set(self.audits.get() + 1);
        Some(SmolStr::new(format!("device-audit-{}", self.audits.get())))
    }
}

/// A device whose clock nobody has set yet (spec 34 TARGET-06).
impl WallClock for BoardPlatform {
    fn utc_ms(&self) -> Option<u64> {
        None
    }
}

impl StatusPort for BoardPlatform {
    fn resource_name(&self) -> &str {
        StatusPort::resource_name(&*self.host)
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        self.host.status_settings()
    }

    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        self.host.io_drivers()
    }

    fn realtime_status(&self) -> Option<Value> {
        Some(absent_realtime_profile())
    }

    fn realtime_has_errors(&self) -> Option<bool> {
        Some(false)
    }

    fn openot_persistence(&self) -> Option<Value> {
        Some(disabled_openot_persistence())
    }
}

/// The device answers no platform operation yet.
impl ExtensionDispatch for BoardPlatform {
    fn dispatch_extension(&self, _request: &ControlRequest) -> Option<ControlResponse> {
        None
    }

    fn audit_details(&self, _request: &ControlRequest) -> Option<Value> {
        None
    }
}

/// The device composition has no watchdog hardware; feeding it does nothing.
impl HardwareWatchdog for BoardPlatform {
    fn feed_interval(&self) -> TimeSpan {
        TimeSpan::new(StdDuration::from_secs(1))
    }

    fn feed(&mut self) {}
}

/// The device answers the Live Values operations from its own queues and published snapshot, which
/// the runner applies and publishes, so its `io.*`, `set` and `var.*` answers are trust-runtime's
/// without borrowing them.
impl IoQueuePort for BoardPlatform {
    fn io_snapshot(&self) -> IoSnapshotRead {
        self.live.io_snapshot()
    }

    fn enqueue_io_write(
        &self,
        address: IoAddress,
        value: RuntimeValue,
    ) -> Result<(), RequestQueueFull> {
        self.live.enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: RuntimeValue) -> Result<(), RequestQueueFull> {
        self.live.force_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        self.live.release_io(address);
    }
}

impl VariableQueuePort for BoardPlatform {
    fn generation(&self) -> Generation {
        self.live.generation()
    }

    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: RuntimeValue,
    ) -> Result<(), RequestQueueFull> {
        self.live.enqueue_var_write(target, value)
    }

    fn force_var(
        &self,
        target: ForcedVarTarget,
        value: RuntimeValue,
    ) -> Result<(), RequestQueueFull> {
        self.live.force_var(target, value)
    }

    fn release_global(&self, name: &str) {
        self.live.release_global(name);
    }

    fn release_retain(&self, name: &str) {
        self.live.release_retain(name);
    }

    fn release_instance(&self, instance: InstanceId, name: &str) {
        self.live.release_instance(instance, name);
    }

    fn forced_snapshot(&self) -> ForcedSnapshot {
        self.live.forced_snapshot()
    }
}

impl RunnerPlatform for BoardPlatform {
    fn live_values(&self) -> &EngineLiveValues {
        &self.live
    }
}

/// The single-thread loop of the device composition.
pub type DeviceRunner<'a> =
    Runner<MemoryLink, ReplayWait, DeviceCycle<'a>, BoardPlatform, ReplayClock>;
