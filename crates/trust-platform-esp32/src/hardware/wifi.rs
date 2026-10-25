//! The C6's Wi-Fi station and its network (specification 34 ARCH-12; checklist L3).
//!
//! The radio's closed driver needs a preemptive scheduler: `esp-rtos` makes the boot's context its
//! main task and runs the driver's tasks above it. The resource loop stays that main task and stays
//! synchronous: the platform's idle work polls the network once per pass ([`C6Net`] is the chip's
//! [`ChipNetwork`]), which joins the network and joins it again after a loss, moves the frames
//! through the shared stack and serves the control connection, and the loop's control link reads that
//! connection through [`NetLink`]. The driver wakes the loop's wait when a frame arrives, a frame is
//! sent or the link changes, through the waker below.
//!
//! Only the main task reaches the network, through the one `Rc<RefCell<C6Net>>` that the platform
//! and the link share; the driver's tasks reach the loop only through the wake counter. Nothing here
//! runs inside a critical section: the driver frees its receive buffers under a mutex of its own,
//! which a critical section would deadlock.

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::task::Wake;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use embassy_net_driver::{Driver, LinkState};
use esp_hal::peripherals::{FROM_CPU_INTR0, WIFI};
use esp_hal::rng::Rng;
use esp_hal::timer::systimer::Alarm;
use esp_radio::wifi::sta::{ConnectedInfo, StationConfig};
use esp_radio::wifi::{
    AuthenticationMethodConfig, Config as WifiConfig, ConnectionError, ControllerConfig,
    Interface as Station, Password, PowerSaveMode, Ssid, WifiController, WifiError, WifiRxToken,
    WifiTxToken,
};
use trust_platform_memory::memory::MeasuredHeap;
use trust_platform_net::smoltcp::phy::{self, DeviceCapabilities, Medium};
use trust_platform_net::smoltcp::time::Instant;
use trust_platform_net::{retry, Network, NetworkSettings};
use trust_runtime_core::ports::{ControlLink, LinkFrame, MonotonicInstant, PortError, WakeSignal};
use trust_runtime_services::chip::network::{ChipNetwork, DriverMemory, NetworkFacts};
use trust_runtime_services::device_settings::{DeviceSettings, WifiSettings};
use trust_runtime_services::link::{ReplyRefused, DEVICE_FRAME_LEN};

use super::clock::now_us;
use super::wait::LinkWake;

/// How long one attempt to join the network may take before it is given up and retried.
const CONNECT_TIMEOUT_US: u64 = 20_000_000;
/// The radio rules allowed in every country, ESP-IDF's world-safe code, without a country in the
/// device settings.
const WORLD_SAFE: [u8; 2] = *b"01";
/// `PortError` code of a reply the network link refused because another reply is in flight.
const LINK_BUSY: u16 = 1;
/// `PortError` code of a reply the network link refused because it is longer than the frame.
const LINK_TOO_LONG: u16 = 2;
/// The `PortError` device number of the network link, after the USB link's 0.
const LINK_DEVICE: u16 = 1;

/// What the radio takes from the board: the scheduler's timer and software interrupt, and the
/// Wi-Fi radio.
pub struct RadioParts {
    /// SYSTIMER alarm 2: the loop's clock and wait keep alarms 0 and 1.
    pub scheduler_alarm: Alarm<'static>,
    /// The software interrupt that switches the scheduler's tasks.
    pub scheduler_interrupt: FROM_CPU_INTR0<'static>,
    /// The Wi-Fi radio.
    pub wifi: WIFI<'static>,
}

/// Why the radio did not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioFailure {
    /// The settings were refused: the network's name or its password.
    Settings(WifiError),
    /// The radio refused to start.
    Radio(WifiError),
}

/// Starts the radio's scheduler. Call once, in thread mode, before [`bring_up`]: the boot's context
/// becomes esp-rtos's main task on the main stack, and the scheduler takes SYSTIMER alarm 2.
pub fn start_scheduler(alarm: Alarm<'static>, interrupt: FROM_CPU_INTR0<'static>) {
    esp_rtos::start(alarm, interrupt);
}

/// Starts the radio as a station of the network that `settings` name, builds the network over it
/// and makes the first attempt to join; `arena` is the radio's memory, whose use the network
/// reports. Call after [`start_scheduler`], with interrupts enabled, before the program starts: the
/// first attempt derives the key from the passphrase.
///
/// # Errors
///
/// The [`RadioFailure`] of settings the driver refuses or of a radio that does not start.
pub fn bring_up(
    radio: WIFI<'static>,
    settings: &DeviceSettings,
    wifi: &WifiSettings,
    arena: &'static MeasuredHeap,
) -> Result<Rc<RefCell<C6Net>>, RadioFailure> {
    let station_config = StationConfig::default()
        .with_ssid(Ssid::try_from(wifi.ssid.as_str()).map_err(RadioFailure::Settings)?)
        .with_authentication(authentication(&wifi.password).map_err(RadioFailure::Settings)?);
    let country = wifi
        .country
        .as_deref()
        .and_then(|code| <[u8; 2]>::try_from(code.as_bytes()).ok())
        .unwrap_or(WORLD_SAFE);
    let config = ControllerConfig::default()
        .with_country_info(country)
        .with_static_rx_buf_num(6)
        .with_dynamic_rx_buf_num(12)
        .with_dynamic_tx_buf_num(12)
        .with_rx_ba_win(4)
        .with_sta_disconnected_pm(false)
        .with_initial_config(WifiConfig::Station(station_config));
    let mut controller = WifiController::new(radio, config).map_err(RadioFailure::Radio)?;
    // No modem sleep, which would delay every frame by a beacon interval.
    controller
        .set_power_saving(PowerSaveMode::None)
        .map_err(RadioFailure::Radio)?;
    let mut station = Station::station();
    let rng = Rng::new();
    let seed = (u64::from(rng.random()) << 32) | u64::from(rng.random());
    let waker = Waker::from(Arc::new(LoopWaker));
    let mut cx = Context::from_waker(&waker);
    let network_settings =
        NetworkSettings::new(settings, &wifi.address_settings(), station.mac_address());
    let network = Network::new(
        network_settings,
        &mut WifiDevice {
            station: &mut station,
            cx: &mut cx,
        },
        seed,
        now_us(),
    );
    let mut net = C6Net {
        controller: Rc::new(RefCell::new(controller)),
        station,
        network,
        phase: Phase::Idle {
            retry_at_us: 0,
            failures: 0,
        },
        failure: None,
        arena,
        waker,
    };
    net.poll();
    Ok(Rc::new(RefCell::new(net)))
}

/// The network's authentication: none for an empty password, WPA2-Personal otherwise, from a
/// passphrase or a 64-digit pre-shared key.
fn authentication(password: &str) -> Result<AuthenticationMethodConfig, WifiError> {
    if password.is_empty() {
        return Ok(AuthenticationMethodConfig::Open);
    }
    Ok(AuthenticationMethodConfig::Wpa2Personal(
        Password::try_from(password)?,
    ))
}

/// An attempt to join the network: esp-radio's `connect_async`, polled by the loop. It holds the
/// controller while it runs, so dropping an attempt that timed out frees the controller again.
type Attempt = Pin<Box<dyn Future<Output = Result<ConnectedInfo, ConnectionError>>>>;

/// Where the station is in joining its network.
enum Phase {
    /// Not joined; the next attempt starts at `retry_at_us`.
    Idle { retry_at_us: u64, failures: u32 },
    /// An attempt runs; it is given up at `deadline_us`.
    Connecting {
        attempt: Attempt,
        deadline_us: u64,
        failures: u32,
    },
    /// Joined.
    Joined,
}

/// The C6's Wi-Fi station and its network, which the platform's idle work and the network link
/// share.
pub struct C6Net {
    controller: Rc<RefCell<WifiController<'static>>>,
    station: Station,
    network: Network,
    phase: Phase,
    /// Why the last attempt to join the network failed, until the station joins it.
    failure: Option<String>,
    /// The radio's arena, whose use `capabilities` reports.
    arena: &'static MeasuredHeap,
    /// The waker the driver's tasks wake the loop with.
    waker: Waker,
}

impl C6Net {
    /// Joins the network, and joins it again after a loss: at once at boot, then after the waits of
    /// [`retry::wait_us`].
    fn drive_station(&mut self, now: u64, cx: &mut Context<'_>, up: bool) {
        self.phase = match core::mem::replace(&mut self.phase, Phase::Joined) {
            Phase::Joined if up => Phase::Joined,
            Phase::Joined => {
                self.failure = Some("the station lost its Wi-Fi network".to_string());
                Phase::Idle {
                    retry_at_us: now.saturating_add(retry::wait_us(1)),
                    failures: 1,
                }
            }
            Phase::Idle {
                retry_at_us,
                failures,
            } if now >= retry_at_us => {
                let controller = Rc::clone(&self.controller);
                let attempt: Attempt = Box::pin(async move {
                    let mut controller = controller.borrow_mut();
                    controller.connect_async().await
                });
                poll_attempt(
                    attempt,
                    now.saturating_add(CONNECT_TIMEOUT_US),
                    failures,
                    now,
                    cx,
                    &mut self.failure,
                )
            }
            Phase::Connecting {
                attempt,
                deadline_us,
                failures,
            } => poll_attempt(attempt, deadline_us, failures, now, cx, &mut self.failure),
            idle => idle,
        };
        if matches!(self.phase, Phase::Joined) {
            self.failure = None;
        }
    }
}

/// Polls an attempt: joined, failed (the next attempt after the wait its failures give), given up at
/// its deadline, or still running. A failure's reason goes to `failure`.
fn poll_attempt(
    mut attempt: Attempt,
    deadline_us: u64,
    failures: u32,
    now: u64,
    cx: &mut Context<'_>,
    failure: &mut Option<String>,
) -> Phase {
    let mut failed = |failures: u32, reason: String| {
        *failure = Some(reason);
        let failures = failures.saturating_add(1);
        Phase::Idle {
            retry_at_us: now.saturating_add(retry::wait_us(failures)),
            failures,
        }
    };
    let elapsed_ms = now.saturating_sub(deadline_us.saturating_sub(CONNECT_TIMEOUT_US)) / 1_000;
    match attempt.as_mut().poll(cx) {
        Poll::Ready(Ok(_)) => Phase::Joined,
        Poll::Ready(Err(ConnectionError::Failed(info))) => failed(
            failures,
            format!(
                "joining the Wi-Fi network failed after {elapsed_ms} ms, attempt {}: {:?} from \
                 {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} (signal {} dBm)",
                failures.saturating_add(1),
                info.reason,
                info.bssid[0],
                info.bssid[1],
                info.bssid[2],
                info.bssid[3],
                info.bssid[4],
                info.bssid[5],
                info.rssi
            ),
        ),
        Poll::Ready(Err(error)) => failed(
            failures,
            format!("the radio refused to join the Wi-Fi network: {error:?}"),
        ),
        Poll::Pending if now >= deadline_us => failed(
            failures,
            "the Wi-Fi network did not answer within 20 seconds".to_string(),
        ),
        Poll::Pending => Phase::Connecting {
            attempt,
            deadline_us,
            failures,
        },
    }
}

impl ChipNetwork for C6Net {
    fn poll(&mut self) {
        let now = now_us();
        let waker = self.waker.clone();
        let mut cx = Context::from_waker(&waker);
        let up = <Station as Driver>::link_state(&mut self.station, &mut cx) == LinkState::Up;
        self.network.set_link(up);
        self.drive_station(now, &mut cx, up);
        let mut device = WifiDevice {
            station: &mut self.station,
            cx: &mut cx,
        };
        self.network.poll(now, &mut device);
    }

    fn facts(&self) -> NetworkFacts {
        NetworkFacts {
            interface: "wifi",
            link_up: self.network.link_up(),
            address: self.network.address().map(|address| address.to_string()),
            control_port: self.network.control_port(),
            error: self.failure.clone(),
            driver_memory: Some({
                let (_, peak, _, failures) = self.arena.emergency_metrics();
                DriverMemory {
                    bytes: self.arena.capacity() as u64,
                    peak_bytes: peak as u64,
                    failures: failures as u64,
                }
            }),
        }
    }

    fn poll_due(&mut self) -> Option<MonotonicInstant> {
        let station_due = match &self.phase {
            Phase::Idle { retry_at_us, .. } => Some(*retry_at_us),
            Phase::Connecting { deadline_us, .. } => Some(*deadline_us),
            Phase::Joined => None,
        };
        let network_due = self.network.poll_due(now_us());
        [station_due, network_due]
            .into_iter()
            .flatten()
            .min()
            .map(MonotonicInstant::from_micros)
    }
}

/// The control connection over the C6's network as the loop's control link reads it: none while
/// the device settings name no network.
pub struct NetLink(Option<Rc<RefCell<C6Net>>>);

impl NetLink {
    /// The link of `net`, or a link that never has a request without one.
    #[must_use]
    pub fn new(net: Option<Rc<RefCell<C6Net>>>) -> Self {
        Self(net)
    }
}

impl ControlLink for NetLink {
    fn max_frame_len(&self) -> usize {
        DEVICE_FRAME_LEN
    }

    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        let mut net = self.0.as_ref()?.try_borrow_mut().ok()?;
        // What arrived since the last pass's idle work, so a request is served in this pass.
        net.poll();
        net.network.poll_frame(buffer, now_us())
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        let refused = |code| PortError {
            code,
            device: LINK_DEVICE,
        };
        let mut net = self
            .0
            .as_ref()
            .and_then(|net| net.try_borrow_mut().ok())
            .ok_or_else(|| refused(LINK_BUSY))?;
        net.network
            .send_frame(frame, now_us())
            .map_err(|refusal| match refusal {
                ReplyRefused::Busy => refused(LINK_BUSY),
                ReplyRefused::TooLong => refused(LINK_TOO_LONG),
            })?;
        // The reply's first bytes leave in this pass.
        net.poll();
        Ok(())
    }

    fn sending(&self) -> bool {
        self.0
            .as_ref()
            .and_then(|net| net.try_borrow().ok())
            .is_some_and(|net| net.network.sending())
    }
}

/// The loop's waker: waking it advances the wake counter that the loop's wait reads, so a frame that
/// arrives, a frame sent or a change of the link ends the wait at once.
struct LoopWaker;

impl Wake for LoopWaker {
    fn wake(self: Arc<Self>) {
        LinkWake.wake();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        LinkWake.wake();
    }
}

/// The station as the stack's device: esp-radio's driver with the loop's waker as its context, as
/// embassy-net's own adapter drives it.
struct WifiDevice<'d, 'c> {
    station: &'d mut Station,
    cx: &'d mut Context<'c>,
}

impl phy::Device for WifiDevice<'_, '_> {
    type RxToken<'a>
        = RxToken
    where
        Self: 'a;
    type TxToken<'a>
        = TxToken
    where
        Self: 'a;

    fn receive(&mut self, _: Instant) -> Option<(RxToken, TxToken)> {
        <Station as Driver>::receive(self.station, self.cx)
            .map(|(rx, tx)| (RxToken(rx), TxToken(tx)))
    }

    fn transmit(&mut self, _: Instant) -> Option<TxToken> {
        <Station as Driver>::transmit(self.station, self.cx).map(TxToken)
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let driver = <Station as Driver>::capabilities(self.station);
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ethernet;
        capabilities.max_transmission_unit = driver.max_transmission_unit;
        capabilities.max_burst_size = driver.max_burst_size;
        capabilities
    }
}

/// A received frame; the driver frees its buffer when the stack has read it.
struct RxToken(WifiRxToken);

impl phy::RxToken for RxToken {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, read: F) -> R {
        self.0.consume_token(|frame| read(frame))
    }
}

/// Room for a frame to send; the driver copies the frame the stack writes.
struct TxToken(WifiTxToken);

impl phy::TxToken for TxToken {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, write: F) -> R {
        self.0.consume_token(len, write)
    }
}
