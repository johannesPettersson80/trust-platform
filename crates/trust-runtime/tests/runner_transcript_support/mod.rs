//! The two compositions of the transcript parity test, both running `portability/f401/gpio.stbc`.
//!
//! The host composition is trust-runtime's threaded one, built as the in-process control tests
//! build it (`ControlState` field by field, a real `ResourceRunner` scan thread, requests through
//! the shared pipeline as `handle_request_line` sends them), with the runtime's metrics sink and
//! event sender wired as the product binary wires them (`run/runtime/entry.rs`).
//!
//! The device composition is the single-thread resource loop of `trust-runtime-services` on the
//! board fixture's engine setup (its preparation limits and execution services), with an
//! in-memory link, a clock that the test sets, and a hardware watchdog that only counts its
//! feeds.

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
use trust_board_fixture::services::{limits, Services};
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
use trust_runtime_core::ports::{
    ControlLink, HardwareWatchdog, LinkFrame, MonotonicClock, MonotonicInstant, PeerClass,
    PortError, RunnerWait, TimeSpan, WakeReason, WakeToken, WallClock,
};
use trust_runtime_core::retain::{RestartMode, RetainSnapshot};
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::{CycleHooks, PreparedModule, RetainApplyEvent, RuntimeState};
use trust_runtime_services::access::{AccessRole, AuthFailure};
use trust_runtime_services::dispatch::prepare_line;
use trust_runtime_services::events::store_event;
use trust_runtime_services::ports::{
    AccessPort, AuditRecord, AuditSink, ExtensionDispatch, ProgramControl, StatusPort,
};
use trust_runtime_services::protocol::{ControlAfterResponse, ControlRequest, ControlResponse};
use trust_runtime_services::runner::{
    resource_tasks, ResourceCycle, RetainSource, Runner, RunnerPlatform, TaskDescriptor,
};
use trust_runtime_services::status::{IoDriverStatus, StatusSettings};

/// The board probe: `led := button` in task `Scan` every 10 ms on resource `Controller`.
pub const GPIO: &[u8] = include_bytes!("../fixtures/portability/f401/gpio.stbc");
const RESOURCE: &str = "Controller";
const CYCLE_INTERVAL_MS: i64 = 10;
/// The platform the device composition reports, standing for the chip `gpio.stbc` is built for.
const BOARD_PLATFORM: &str = "stm32f407";
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
            runtime_id: SmolStr::new("test-runtime-id"),
            io_health,
            debug_enabled: Arc::new(AtomicBool::new(true)),
            debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
            hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
            hmi_persistence: None,
            hmi_descriptor,
            historian: None,
            openot_persistence_status: None,
            pairing: None,
            search: Default::default(),
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
            store_event(&mut store, event);
        }
        self.completed += 1;
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

impl trust_board_fixture::board::Clock for ReplayClock {
    fn micros(&self) -> u64 {
        u64::try_from(self.0.get().as_micros()).unwrap_or(u64::MAX)
    }

    fn wait_until_us(&self, deadline_us: u64) {
        self.0
            .set(self.0.get().max(StdDuration::from_micros(deadline_us)));
    }

    fn cycle_count(&self) -> u32 {
        0
    }

    fn irq_count(&self) -> u32 {
        0
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

/// The board probe on the board fixture's engine setup, behind the loop's cycle port.
pub struct BoardCycle<'a> {
    state: RuntimeState<'a>,
    prepared: &'a PreparedModule,
    services: &'a Services<'a, ReplayClock>,
    checkpoint: Option<RetainSnapshot>,
    scans: Rc<Cell<usize>>,
    watchdog: WatchdogPolicy,
    fault_policy: FaultPolicy,
}

impl<'a> BoardCycle<'a> {
    /// The program of `prepared`, started, with the host composition's policies.
    pub fn start(
        prepared: &'a PreparedModule,
        services: &'a Services<'a, ReplayClock>,
        scans: Rc<Cell<usize>>,
        watchdog: WatchdogPolicy,
        fault_policy: FaultPolicy,
    ) -> Self {
        let state = prepared
            .instantiate_with_services(0, services)
            .expect("the board probe starts on the board fixture's engine setup");
        Self {
            state,
            prepared,
            services,
            checkpoint: None,
            scans,
            watchdog,
            fault_policy,
        }
    }
}

/// The board fixture's preparation limits, which the chips admit programs with.
pub fn prepare_board_probe() -> PreparedModule {
    PreparedModule::from_bytes(GPIO, limits()).expect("the board fixture admits the board probe")
}

impl ResourceCycle for BoardCycle<'_> {
    fn tasks(&self) -> Vec<TaskDescriptor> {
        resource_tasks(self.prepared, 0)
    }

    fn cycle_interval(&self) -> Duration {
        Duration::from_millis(CYCLE_INTERVAL_MS)
    }

    fn watchdog(&self) -> WatchdogPolicy {
        self.watchdog
    }

    fn fault_policy(&self) -> FaultPolicy {
        self.fault_policy
    }

    fn run_scan(
        &mut self,
        now: Duration,
        deadline: Option<MonotonicInstant>,
        observer: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        self.services.deadline.set(deadline.map(|deadline| {
            u64::try_from(deadline.since_origin().as_micros()).unwrap_or(u64::MAX)
        }));
        let result = self.state.execute_cycle_with_hooks(now, observer);
        self.services.deadline.set(None);
        self.scans.set(self.scans.get() + 1);
        result
    }

    fn latch_fault(&mut self, fault: RuntimeError) {
        self.state.latch_fault(fault);
    }

    /// The board probe has no `[[io.safe_state]]`, as the host composition has none.
    fn apply_safe_outputs(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn save_retain(&mut self) -> Result<(), RuntimeError> {
        self.checkpoint = Some(self.state.retain_snapshot()?);
        Ok(())
    }

    fn restart(
        &mut self,
        mode: RestartMode,
        source: RetainSource,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        match (source, mode) {
            (RetainSource::Checkpoint, RestartMode::Warm) => {
                let checkpoint = self.checkpoint.clone().unwrap_or_default();
                self.state
                    .restart_with_retain_snapshot(RestartMode::Warm, &checkpoint)
            }
            (RetainSource::Memory | RetainSource::Checkpoint, mode) => {
                self.state.restart(mode)?;
                Ok(Vec::new())
            }
        }
    }

    fn reset_task_timing(&mut self, now: Duration) {
        self.state.reset_task_timing(now);
    }
}

/// The device's platform. Its configuration is the host composition's, so the values the
/// platform knows (settings, realtime posture, `OpenOT` state, I/O drivers, resource name,
/// credentials) are the same in both compositions; it audits and dispatches on its own.
pub struct BoardPlatform {
    host: Arc<ControlState>,
    audits: Cell<u64>,
}

impl BoardPlatform {
    pub fn configured_as(host: &HostComposition) -> Self {
        Self {
            host: Arc::clone(&host.state),
            audits: Cell::new(0),
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

    /// The board the device composition stands for; the transcript parity test normalises it
    /// against the host's `<os>-<arch>`.
    fn platform(&self) -> &'static str {
        BOARD_PLATFORM
    }

    /// A device without an operating system has no project ID; it reports its platform's value
    /// (specification 11, `status`). The transcript parity test normalises it against the host's
    /// stable runtime ID.
    fn runtime_id(&self) -> &'static str {
        BOARD_PLATFORM
    }

    /// A device without an operating system keeps control after `shutdown` (spec 34 STATE-08).
    fn program_control(&self) -> ProgramControl {
        ProgramControl::KeepsControl
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        self.host.status_settings()
    }

    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        self.host.io_drivers()
    }

    fn realtime_status(&self) -> Option<Value> {
        self.host.realtime_status()
    }

    fn realtime_has_errors(&self) -> Option<bool> {
        self.host.realtime_has_errors()
    }

    fn openot_persistence(&self) -> Option<Value> {
        self.host.openot_persistence()
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

impl RunnerPlatform for BoardPlatform {}

/// The single-thread loop of the device composition.
pub type DeviceRunner<'a> =
    Runner<MemoryLink, ReplayWait, BoardCycle<'a>, BoardPlatform, ReplayClock>;
