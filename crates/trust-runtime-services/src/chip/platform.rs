//! The platform every chip's resource loop runs over (specification 34, ARCH-11 and §3.3): the values
//! the chip reports, the access rights of its USB control link, the Live Values the loop applies, its
//! hardware watchdog and the reset its boot reports. A chip supplies its profile ([`ChipProfile`]: its
//! name, its descriptor, the reason of its watchdog reset and its checkpoint step), and passes its
//! watchdog, its runtime ID and its settings to [`ChipPlatform::new`]; the rest is the same on every
//! chip.
//!
//! A USB client is local (specification 11 §6.9.5), so the link carries no transport label; a local
//! client acts as `Admin` until the chip has a control token, from its device settings or from
//! `config.set`, after which a request must carry it, exactly as on trust-runtime's local socket. A
//! client on the chip's network is labelled with its address, and the chip serves its network only
//! with a token, which grants `Admin` (specification 34 ARCH-12). The platform keeps the settings of the
//! installed program's record as `config.get` reports them and `config.set` changes them
//! (specification 11, the runtime configuration contract). A chip has no audit store and no
//! platform operation yet, so those ports answer as an absent value does, not with an invented one. The chip answers the `io.*`, `set` and `var.*` operations from its own Live
//! Values, the queues the runner applies and the snapshot it publishes (specification 11, "Live
//! Values"; specification 34 §4.3.1).

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::marker::PhantomData;

use smol_str::SmolStr;
use trust_platform_memory::reset::{ResetCause, ResetRecord};
use trust_runtime_core::deployment::{ControlMode, DeviceDescriptor};
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::ports::{
    CheckpointStorage, CheckpointStore, HardwareWatchdog, MonotonicInstant, TimeSpan, WallClock,
};
use trust_runtime_core::retain::checkpoint::CheckpointFlash;
use trust_runtime_core::value::Value;
use trust_runtime_core::watchdog::RetainMode;

use super::network::{ChipNetwork, NetworkFacts};
use super::watchdog::{cycle_timeout_ms, CycleWatchdog};
use crate::access::{resolve_role, AccessRole, AuthFailure};
use crate::capabilities::{LastReset, ResetCauseName};
use crate::device_settings::{DeviceInterfaces, DeviceSettings, SettingsFlash, SettingsStore};
use crate::firmware::FirmwareFacts;
use crate::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use crate::live_values::queues::{ForcedSnapshot, ForcedVarTarget, PendingVarTarget};
use crate::live_values::var_requests::VariableQueuePort;
use crate::live_values::RequestQueueFull;
use crate::ports::{
    AccessPort, AuditRecord, AuditSink, DevicePort, ExtensionDispatch, ProgramControl, StatusPort,
};
use crate::protocol::{ControlRequest, ControlResponse};
use crate::runner::{DeviceLiveValues, RunnerPlatform};
use crate::settings::{RuntimeSettings, TokenChange};
use crate::status::{
    absent_realtime_profile, disabled_openot_persistence, IoDriverStatus, StatusSettings,
};

/// What a chip supplies to the shared platform as its profile (specification 34, ARCH-11): the values
/// that name it and describe it, and the reason of its watchdog reset.
pub trait ChipProfile {
    /// The hardware reset reason as the chip's boot reads it.
    type Reason: Copy + PartialEq;

    /// The reason of a watchdog reset, which writes no reset record.
    fn watchdog_reset() -> Self::Reason;

    /// The chip's name, which `status` reports as the platform (specification 11, `status`).
    fn platform_name() -> &'static str;

    /// The device descriptor admission reads (specification 34 §5.1.1, COMPAT-02 and TARGET-03).
    fn device_descriptor() -> DeviceDescriptor;

    /// The most bytes one checkpoint step programs between two scans (specification 34 §10), or
    /// `None` while the chip keeps no checkpoint store.
    fn checkpoint_step_bytes() -> Option<usize>;

    /// The chip's wall clock in milliseconds since the Unix epoch, or `None` while no time source
    /// set it (specification 34 TARGET-06). The default is a chip without one.
    #[must_use]
    fn utc_ms() -> Option<u64> {
        None
    }

    /// Sets the chip's wall clock to `utc_ms` and returns whether it did; a chip without one sets
    /// nothing.
    #[must_use]
    fn set_utc_ms(_utc_ms: u64) -> bool {
        false
    }

    /// The chip's own work between scans, once per pass of the loop (`RunnerPlatform::idle`); it
    /// must not wait. The default does nothing.
    fn idle() {}

    /// The interfaces the chip's device settings are checked against (specification 34 TARGET-02),
    /// or `None` for a chip that keeps no device settings. The default is none.
    #[must_use]
    fn interfaces() -> Option<DeviceInterfaces> {
        None
    }

    /// Resets the chip into the firmware its boot loader selects (specification 34 TARGET-02 and
    /// TARGET-04); on a chip it does not return. The default does nothing.
    fn reset() {}
}

/// The values of an installed program's record that its platform reports and answers from
/// (specification 34 §5.1.1 and STATE-08): what a chip sets on its platform when it loads a program.
#[derive(Clone)]
pub struct ProgramValues {
    /// `resource.name` of the record, which `status` reports.
    pub resource_name: SmolStr,
    /// The settings of the record that `config.get` reports, among them `runtime.control.mode` and
    /// `runtime.control.debug_enabled`.
    pub settings: RuntimeSettings,
    /// The Live Values of the program, which the runner applies and the `io.*`, `set` and `var.*`
    /// operations answer from.
    pub live: DeviceLiveValues,
}

/// A chip's platform for the single-thread resource loop: the ports of [`ChipPlatform`] over the chip's
/// profile `C` and its hardware watchdog `W`.
pub struct ChipPlatform<C, W> {
    chip: PhantomData<C>,
    watchdog: W,
    resource_name: SmolStr,
    settings: StatusSettings,
    /// The settings of the installed program's record, as `config.set` changed them (specification
    /// 34 §5.1.1, section 7): among them the control mode `status` reports and the path `pause` and
    /// `resume` take. None while the chip has no program, which then reports `production` without a
    /// debug surface, the record's defaults.
    runtime: Option<RuntimeSettings>,
    /// The control token a request must carry: the device settings' (`[control] token`), read at
    /// boot, or the one `config.set` gave the chip since, which a reset forgets, as a computer's
    /// process forgets one set over its control connection; none without either.
    token: Option<SmolStr>,
    /// Whether the chip serves its control connection over a network, which then requires the
    /// token, so that `config.set` cannot clear it (specification 11, the runtime configuration
    /// contract).
    token_required: bool,
    /// The Live Values the runner applies at its cycle phases and the `io.*`, `set` and `var.*`
    /// operations answer from (specification 11, "Live Values"; specification 34 §4.3.1).
    live: DeviceLiveValues,
    /// The reset the boot reports, for the `capabilities` reply (specification 11, the runtime device
    /// capabilities contract), built by [`ChipPlatform::boot_last_reset`].
    last_reset: LastReset,
    /// The chip's stable runtime ID that `status` reports (specification 11, `status`).
    runtime_id: SmolStr,
    /// The chip's checkpoint flash, held until the installed program takes its store (specification 34
    /// §10); `None` unless the record's retain mode is `file`, and after the store is taken.
    checkpoint: Option<Box<dyn CheckpointFlash>>,
    /// The bytes of memory the chip has for a program, measured at boot (specification 34 MEM-12);
    /// `None` until the firmware measures it.
    program_memory: Option<u64>,
    /// What the boot read of the chip's firmware slots (specification 34 TARGET-04); `None` for a
    /// chip that keeps none.
    firmware: Option<FirmwareFacts>,
    /// The chip's device settings and their two sectors (specification 34 TARGET-02); `None` for a
    /// chip that keeps none.
    device: Option<RefCell<SettingsStore>>,
    /// The chip's network, which the idle work keeps (specification 34 ARCH-12); `None` for a chip
    /// without one, and for one whose device settings name no network.
    network: Option<Rc<RefCell<dyn ChipNetwork>>>,
    /// The network that the device settings name and that did not start, with its reason.
    network_failure: Option<NetworkFacts>,
}

impl<C: ChipProfile, W> ChipPlatform<C, W> {
    /// A platform over `watchdog` that reports `resource_name` and the runtime `settings`, with the
    /// Live Values `live` (or [`DeviceLiveValues::empty`] for a chip without a program), the
    /// `last_reset` the boot reports, and the chip's stable `runtime_id`. It has no program's record
    /// until [`Self::set_program`].
    #[must_use]
    pub fn new(
        watchdog: W,
        resource_name: SmolStr,
        settings: StatusSettings,
        live: DeviceLiveValues,
        last_reset: LastReset,
        runtime_id: SmolStr,
    ) -> Self {
        Self {
            chip: PhantomData,
            watchdog,
            resource_name,
            settings,
            runtime: None,
            token: None,
            token_required: false,
            live,
            last_reset,
            runtime_id,
            checkpoint: None,
            program_memory: None,
            firmware: None,
            device: None,
            network: None,
            network_failure: None,
        }
    }

    /// Keeps the chip's device settings in `flash`, its two sectors, and reads the newest record
    /// (specification 34 TARGET-02): its control token is the one a request must carry, and a
    /// network that it names with a token serves the control connection, which then requires one.
    /// A chip without device settings keeps none.
    #[must_use]
    pub fn with_device_settings(mut self, flash: Box<dyn SettingsFlash>) -> Self {
        let store = SettingsStore::open(flash);
        if let Some((_, settings)) = store.settings() {
            self.token = settings.control.token.as_deref().map(SmolStr::new);
            self.token_required = self.token.is_some() && settings.network.names_one();
        }
        self.device = Some(RefCell::new(store));
        self
    }

    /// Drives `network` in the idle work between scans, and ends the loop's wait when its work is
    /// due (specification 34 ARCH-12).
    #[must_use]
    pub fn with_network(mut self, network: Rc<RefCell<dyn ChipNetwork>>) -> Self {
        self.network = Some(network);
        self
    }

    /// Reports a network that the device settings name and that did not start, with its reason in
    /// `facts.error`.
    pub fn set_network_failure(&mut self, facts: NetworkFacts) {
        self.network_failure = Some(facts);
    }

    /// The stored device settings, which the boot reads before the network and the program start.
    #[must_use]
    pub fn stored_settings(&self) -> Option<DeviceSettings> {
        let store = self.device.as_ref()?.try_borrow().ok()?;
        store.settings().map(|(_, settings)| settings)
    }

    /// Gives the platform what the boot read of the chip's firmware slots, which `capabilities`
    /// reports (specification 34 TARGET-04).
    pub fn set_firmware(&mut self, facts: FirmwareFacts) {
        self.firmware = Some(facts);
    }

    /// Gives the platform the bytes of memory the chip has for a program, which the firmware
    /// measures at boot before it loads one: the heap's free bytes then, the installed container's
    /// bytes among them, less the reserve the chip keeps for the machine (specification 34 MEM-12).
    /// The descriptor reports them, and admission refuses a program whose memory figure is more.
    pub fn set_program_memory(&mut self, bytes: usize) {
        self.program_memory = u64::try_from(bytes).ok();
    }

    /// Keeps `flash` as the chip's checkpoint flash when the record's retain mode is `file`
    /// (specification 34 §10): the installed program takes it as its checkpoint store. With `none` the
    /// flash is not kept and the platform gives no store.
    #[must_use]
    pub fn with_checkpoint(mut self, retain: RetainMode, flash: Box<dyn CheckpointFlash>) -> Self {
        self.checkpoint = (retain == RetainMode::File).then_some(flash);
        self
    }

    /// The watchdog the platform holds, for the stopped loop that serves the link when the installed
    /// program cannot start.
    #[must_use]
    pub fn into_watchdog(self) -> W {
        self.watchdog
    }

    /// The watchdog the platform holds, for a loader that sets its interval from the record of the
    /// program it loads (specification 34 TIME-04).
    pub fn watchdog_mut(&mut self) -> &mut W {
        &mut self.watchdog
    }

    /// Gives the platform the values of the record of a program that was loaded: its resource name,
    /// its settings and the Live Values (specification 34 STATE-08).
    pub fn set_program(&mut self, program: ProgramValues) {
        self.resource_name = program.resource_name;
        self.runtime = Some(program.settings);
        self.live = program.live;
    }

    /// Gives the platform the Live Values of the program that an online change gave the resource;
    /// the record, and the settings `config.set` changed, stay (specification 34 §11.3).
    pub fn set_live_values(&mut self, live: DeviceLiveValues) {
        self.live = live;
    }

    /// The reset the boot reports (specification 11, the runtime device capabilities contract). Its
    /// cause and text are the record the previous boot left. When it left none, the boot's hardware
    /// reset reason `reason` decides: the chip's watchdog reset writes no record (specification 34,
    /// the chip's supervision), and any other reason is `power_on`. `boot_count` is the chip's boot
    /// counter for this boot, reported for every reset, record or not.
    #[must_use]
    pub fn boot_last_reset(
        reason: C::Reason,
        record: Option<ResetRecord>,
        boot_count: u32,
    ) -> LastReset {
        match record {
            Some(record) => LastReset {
                cause: reset_cause(record.cause()),
                text: SmolStr::new(String::from_utf8_lossy(record.text()).as_ref()),
                boot_count,
            },
            None => LastReset {
                cause: if reason == C::watchdog_reset() {
                    ResetCauseName::Watchdog
                } else {
                    ResetCauseName::PowerOn
                },
                text: SmolStr::default(),
                boot_count,
            },
        }
    }
}

/// The runtime settings `status` reports for a chip: compiled programs, production control and no
/// simulation (specification 11, `status`; specification 44 §13.3).
#[must_use]
pub fn status_settings() -> StatusSettings {
    StatusSettings {
        execution_backend: "native",
        execution_backend_source: "default",
        simulation_enabled: false,
        simulation_time_scale: 1,
        simulation_mode: SmolStr::new_static("production"),
        simulation_warning: SmolStr::default(),
    }
}

impl<C, W> AccessPort for ChipPlatform<C, W> {
    /// The control token grants `Admin`, from any link. Without a token a local client (no
    /// transport label) acts as `Admin` and a client with a network label as `Viewer`, as on
    /// trust-runtime.
    fn request_role(
        &self,
        request: &ControlRequest,
        client: Option<&str>,
    ) -> Result<AccessRole, AuthFailure> {
        resolve_role(
            request.auth.as_deref(),
            self.token.as_deref(),
            false,
            client,
            |_| None,
        )
    }

    fn debug_enabled(&self) -> bool {
        self.runtime.is_some_and(|settings| settings.debug_enabled)
    }
}

impl<C, W> AuditSink for ChipPlatform<C, W> {
    /// A chip has no audit store, so no record id is reported.
    fn record_audit(&self, _record: AuditRecord<'_>) -> Option<SmolStr> {
        None
    }
}

/// The chip's wall clock, `None` until a time source sets it (specification 34, TARGET-06).
impl<C: ChipProfile, W> WallClock for ChipPlatform<C, W> {
    fn utc_ms(&self) -> Option<u64> {
        C::utc_ms()
    }

    fn set_utc_ms(&self, utc_ms: u64) -> bool {
        C::set_utc_ms(utc_ms)
    }
}

impl<C: ChipProfile, W> StatusPort for ChipPlatform<C, W> {
    fn resource_name(&self) -> &str {
        self.resource_name.as_str()
    }

    /// The chip's name (specification 11, `status`).
    fn platform(&self) -> &str {
        C::platform_name()
    }

    /// The chip's name and USB serial number (specification 11, `status`).
    fn runtime_id(&self) -> &str {
        self.runtime_id.as_str()
    }

    /// A chip keeps control after `shutdown`: the program takes the place of the process (specification
    /// 34, STATE-08).
    fn program_control(&self) -> ProgramControl {
        ProgramControl::KeepsControl
    }

    fn status_settings(&self) -> Option<StatusSettings> {
        Some(self.settings.clone())
    }

    /// The installed program's own drivers are reported by the cycle; the platform runs none.
    fn io_drivers(&self) -> Option<Vec<IoDriverStatus>> {
        Some(Vec::new())
    }

    fn realtime_status(&self) -> Option<serde_json::Value> {
        Some(absent_realtime_profile())
    }

    fn realtime_has_errors(&self) -> Option<bool> {
        Some(false)
    }

    fn openot_persistence(&self) -> Option<serde_json::Value> {
        Some(disabled_openot_persistence())
    }

    /// The chip's descriptor, the same admission reads (specification 34 §5.1.1, COMPAT-02 and
    /// TARGET-03).
    fn device_descriptor(&self) -> DeviceDescriptor {
        DeviceDescriptor {
            program_memory_bytes: self.program_memory,
            ..C::device_descriptor()
        }
    }

    /// The reset the boot reported (specification 11, the runtime device capabilities contract).
    fn last_reset(&self) -> Option<LastReset> {
        Some(self.last_reset.clone())
    }

    /// What the boot read of the firmware slots (specification 34 TARGET-04).
    fn firmware(&self) -> Option<FirmwareFacts> {
        self.firmware.clone()
    }

    fn network(&self) -> Option<NetworkFacts> {
        match &self.network {
            Some(network) => network.try_borrow().ok().map(|network| network.facts()),
            None => self.network_failure.clone(),
        }
    }
}

/// The wire name of the reset record's cause (specification 11, the runtime device capabilities
/// contract).
fn reset_cause(cause: ResetCause) -> ResetCauseName {
    match cause {
        ResetCause::Panic => ResetCauseName::Panic,
        ResetCause::Watchdog => ResetCauseName::Watchdog,
        ResetCause::Requested => ResetCauseName::Requested,
    }
}

/// The chip's checkpoint store, given once (specification 34 §10): its flash, and the chip's step
/// budget, at most one flash page per persister step so a step takes a few milliseconds between two
/// scans. A chip without a step budget gives no store.
impl<C: ChipProfile, W> CheckpointStorage for ChipPlatform<C, W> {
    fn take_checkpoint_store(&mut self) -> Option<CheckpointStore> {
        let program_budget = C::checkpoint_step_bytes()?;
        let flash = self.checkpoint.take()?;
        Some(CheckpointStore {
            flash,
            program_budget,
        })
    }
}

/// The chip answers no platform operation yet.
impl<C, W> ExtensionDispatch for ChipPlatform<C, W> {
    fn dispatch_extension(&self, _request: &ControlRequest) -> Option<ControlResponse> {
        None
    }

    fn audit_details(&self, _request: &ControlRequest) -> Option<serde_json::Value> {
        None
    }
}

impl<C, W: HardwareWatchdog> HardwareWatchdog for ChipPlatform<C, W> {
    fn feed_interval(&self) -> TimeSpan {
        self.watchdog.feed_interval()
    }

    fn feed(&mut self) {
        self.watchdog.feed();
    }
}

impl<C: ChipProfile, W: HardwareWatchdog + CycleWatchdog> RunnerPlatform for ChipPlatform<C, W> {
    fn live_values(&self) -> &DeviceLiveValues {
        &self.live
    }

    /// The installed application's record carries the mode (specification 34 §5.1.1, section 7);
    /// `config.set` changes it.
    fn control_mode(&self) -> ControlMode {
        self.runtime
            .map_or(ControlMode::Production, |settings| settings.control_mode)
    }

    fn runtime_settings(&self) -> Option<RuntimeSettings> {
        self.runtime
    }

    fn control_token_length(&self) -> Option<usize> {
        self.token.as_ref().map(SmolStr::len)
    }

    /// The hardware watchdog's interval follows the cycle watchdog's timeout (specification 34,
    /// TIME-04), so a timeout whose interval the chip's watchdog does not take is refused.
    fn takes_cycle_timeout_ms(&self, cycle_timeout_ms: u32) -> bool {
        self.watchdog.takes_cycle_timeout_ms(cycle_timeout_ms)
    }

    /// The settings and the token of a `config.set` that every check passed; a new cycle watchdog
    /// timeout sets the hardware watchdog's interval, which the check found the watchdog takes.
    fn commit_settings(&mut self, settings: RuntimeSettings, token: TokenChange) {
        let timeout = cycle_timeout_ms(&settings.watchdog);
        if self
            .runtime
            .is_none_or(|running| cycle_timeout_ms(&running.watchdog) != timeout)
        {
            let _ = self.watchdog.apply_cycle_timeout_ms(timeout);
        }
        self.runtime = Some(settings);
        token.apply_to(&mut self.token);
    }

    /// The Live Values of the program that ended give way to those of no program, so that their
    /// memory is free for the next program, and its record's settings go with it (specification 34
    /// STATE-08). The control token is the chip's, not the program's, and stays.
    fn unload_program(&mut self) {
        self.live = DeviceLiveValues::empty();
        self.runtime = None;
    }

    /// The chip's own work between scans, such as keeping its clock (specification 34 TARGET-06),
    /// and its network's (ARCH-12).
    fn idle(&mut self) {
        C::idle();
        if let Some(mut network) = self
            .network
            .as_ref()
            .and_then(|network| network.try_borrow_mut().ok())
        {
            network.poll();
        }
    }

    fn idle_due(&self) -> Option<MonotonicInstant> {
        self.network.as_ref()?.try_borrow_mut().ok()?.poll_due()
    }

    fn control_token_required(&self) -> bool {
        self.token_required
    }

    /// The chip resets after the reply to a `firmware.install` or a `device.reset`.
    fn reset_device(&mut self) {
        C::reset();
    }
}

/// A chip keeps its device settings in its two settings sectors (specification 34 TARGET-02).
impl<C: ChipProfile, W> DevicePort for ChipPlatform<C, W> {
    fn device_interfaces(&self) -> Option<DeviceInterfaces> {
        self.device.as_ref().and(C::interfaces())
    }

    fn device_settings(&self) -> Result<Option<(u32, DeviceSettings)>, String> {
        let Some(store) = self.device.as_ref() else {
            return Ok(None);
        };
        let store = store
            .try_borrow()
            .map_err(|_| String::from("the device settings are in use"))?;
        Ok(store.settings())
    }

    fn store_device_settings(&self, settings: &DeviceSettings) -> Result<u32, String> {
        let store = self
            .device
            .as_ref()
            .ok_or_else(|| String::from("unsupported request"))?;
        store
            .try_borrow_mut()
            .map_err(|_| String::from("the device settings are in use"))?
            .store(settings)
    }
}

/// The platform forwards the `io.*`, `set` and `var.*` operations to the Live Values of the installed
/// program, as trust-runtime's `ControlState` forwards them.
impl<C, W> IoQueuePort for ChipPlatform<C, W> {
    fn io_snapshot(&self) -> IoSnapshotRead {
        self.live.io_snapshot()
    }

    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.live.enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.live.force_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        self.live.release_io(address);
    }
}

impl<C, W> VariableQueuePort for ChipPlatform<C, W> {
    fn generation(&self) -> Generation {
        self.live.generation()
    }

    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.live.enqueue_var_write(target, value)
    }

    fn force_var(&self, target: ForcedVarTarget, value: Value) -> Result<(), RequestQueueFull> {
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

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use super::*;

    /// A chip profile of the tests: its watchdog reset is reason 7, and its name is `test-chip`.
    struct TestChip;

    impl ChipProfile for TestChip {
        type Reason = u8;

        fn watchdog_reset() -> u8 {
            7
        }

        fn platform_name() -> &'static str {
            "test-chip"
        }

        fn device_descriptor() -> DeviceDescriptor {
            DeviceDescriptor::hosted()
        }

        fn checkpoint_step_bytes() -> Option<usize> {
            None
        }
    }

    /// A watchdog that counts its feeds, so a test can see that the platform forwards them.
    #[derive(Default)]
    struct CountingWatchdog {
        feeds: u32,
    }

    impl HardwareWatchdog for CountingWatchdog {
        fn feed_interval(&self) -> TimeSpan {
            TimeSpan::new(Duration::from_millis(1_000))
        }

        fn feed(&mut self) {
            self.feeds += 1;
        }
    }

    impl super::super::watchdog::CycleWatchdog for CountingWatchdog {
        fn max_interval_ms(&self) -> u32 {
            60_000
        }

        fn set_interval_ms(&mut self, _milliseconds: u32) -> bool {
            true
        }
    }

    /// A platform of the test chip over a counting watchdog, which reports the reset `last_reset`.
    fn platform_with(last_reset: LastReset) -> ChipPlatform<TestChip, CountingWatchdog> {
        ChipPlatform::new(
            CountingWatchdog::default(),
            SmolStr::new_static("Controller"),
            status_settings(),
            DeviceLiveValues::empty(),
            last_reset,
            SmolStr::new_static("test-chip-01"),
        )
    }

    /// A reset as the tests expect it: its cause, its text and the boot counter.
    fn reset_of(cause: ResetCauseName, text: &str, boot_count: u32) -> LastReset {
        LastReset {
            cause,
            text: SmolStr::new(text),
            boot_count,
        }
    }

    /// A watchdog reset (reason 7) that left no record reads as `watchdog` with empty text, and the
    /// boot's counter is reported. Catches a watchdog reason that is taken for power-on, and a counter
    /// that is left at zero.
    #[test]
    fn a_watchdog_reset_that_left_no_record_reads_as_watchdog() {
        assert_eq!(
            ChipPlatform::<TestChip, ()>::boot_last_reset(7, None, 4),
            reset_of(ResetCauseName::Watchdog, "", 4)
        );
    }

    /// Any other reason that left no record reads as `power_on`, with this boot's counter. Catches a
    /// reason that is compared the wrong way round, which would read a power-on as a watchdog reset.
    #[test]
    fn any_other_reason_that_left_no_record_reads_as_power_on() {
        for (reason, count) in [(1_u8, 2_u32), (0, 3)] {
            assert_eq!(
                ChipPlatform::<TestChip, ()>::boot_last_reset(reason, None, count),
                reset_of(ResetCauseName::PowerOn, "", count),
                "reason {reason} with no record reads as power_on"
            );
        }
    }

    /// A record the previous boot left wins over the reason: its cause and text are reported with this
    /// boot's counter (6), not the record's own counter (5). Catches a reason that overrides the record,
    /// and a record that reports its own boot counter.
    #[test]
    fn a_record_the_boot_read_wins_over_the_reason_and_keeps_this_boots_counter() {
        let record = ResetRecord::with_text(ResetCause::Panic, "main.rs:42", 5);
        assert_eq!(
            ChipPlatform::<TestChip, ()>::boot_last_reset(7, Some(record), 6),
            reset_of(ResetCauseName::Panic, "main.rs:42", 6)
        );
    }

    /// The platform names itself with its profile's name and its runtime ID, and keeps control after
    /// `shutdown` (specification 34, STATE-08). Catches a platform name taken from the runtime ID, and
    /// a program that gives up control.
    #[test]
    fn the_platform_names_its_chip_and_keeps_control_after_shutdown() {
        let platform = platform_with(reset_of(ResetCauseName::PowerOn, "", 1));
        assert_eq!(StatusPort::platform(&platform), "test-chip");
        assert_eq!(StatusPort::runtime_id(&platform), "test-chip-01");
        assert_eq!(
            StatusPort::program_control(&platform),
            ProgramControl::KeepsControl
        );
    }

    /// The platform reports the reset its boot read, as the chip gives it (specification 11). Catches a
    /// platform that reports the reset of another boot.
    #[test]
    fn the_platform_reports_the_reset_its_boot_read() {
        let boot = reset_of(ResetCauseName::Watchdog, "", 4);
        let platform = platform_with(boot.clone());
        assert_eq!(StatusPort::last_reset(&platform), Some(boot));
    }

    /// The watchdog feeds reach the chip's watchdog, one for each feed the runner makes. Catches a
    /// platform that keeps the feed for itself, so the hardware watchdog would reset the chip.
    #[test]
    fn the_runner_feeds_reach_the_chips_watchdog() {
        let mut platform = platform_with(reset_of(ResetCauseName::PowerOn, "", 1));
        HardwareWatchdog::feed(&mut platform);
        HardwareWatchdog::feed(&mut platform);
        assert_eq!(platform.into_watchdog().feeds, 2);
    }
}
