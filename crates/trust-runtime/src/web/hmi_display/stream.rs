//! The live-value stream of the runtime display (specification 39, §11.1): the expected update
//! period, then once per period every path's value with the runtime's cycle count. The simulation
//! the header shows is sent when it changes (specification 33, §6.1).

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use std::collections::BTreeMap;

use serde_json::{json, Value};
use tungstenite::protocol::{Role, WebSocket};
use tungstenite::Message;

use super::super::web_helpers::header_value;
use crate::control::ControlState;
use crate::hmi::access::sessions::{SessionInfo, SessionStore};
use crate::hmi::bundle::store::BundleStore;
use crate::hmi::commands::registry::CommandRegistry;
use crate::hmi::display_values::WireValue;
use crate::hmi::exchange::{HmiExchange, ScreenId};
use crate::hmi::member_paths::MemberPath;
use crate::scheduler::{ResourceControl, StdClock};

/// The shortest update period, whatever the scan period (§11.1).
pub(super) const MIN_PERIOD: Duration = Duration::from_millis(250);
/// The most streams open at once.
const MAX_STREAMS: usize = 32;

static OPEN_STREAMS: AtomicUsize = AtomicUsize::new(0);

/// A place among the open streams, given back when the stream ends.
pub(super) struct StreamSlot;

impl StreamSlot {
    /// A slot, or `None` when the most streams are already open.
    pub(super) fn take() -> Option<Self> {
        OPEN_STREAMS
            .try_update(Ordering::AcqRel, Ordering::Acquire, |open| {
                (open < MAX_STREAMS).then_some(open + 1)
            })
            .ok()
            .map(|_| Self)
    }
}

impl Drop for StreamSlot {
    fn drop(&mut self) {
        OPEN_STREAMS.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(super) struct Stream {
    pub(super) resource: ResourceControl<StdClock>,
    pub(super) period: Duration,
    pub(super) paths: Vec<String>,
    pub(super) slot: StreamSlot,
    pub(super) sessions: Arc<SessionStore>,
    /// Later command outcomes for the session (§12.8).
    pub(super) commands: Arc<CommandRegistry>,
    /// The bundles, for the version a changed bundle has (§16.4).
    pub(super) store: Arc<BundleStore>,
    /// Counts activations.
    pub(super) generation: Arc<AtomicU64>,
    /// The alarm list and records (specification 40, §5.1).
    pub(super) alarms: Arc<super::super::hmi_alarms::AlarmService>,
    /// The runtime's control state: the simulation settings and speeds the header shows.
    pub(super) control: Arc<ControlState>,
    /// The session cookie the page connected with, if any.
    pub(super) token: Option<String>,
}

impl Stream {
    pub(super) fn spawn(self, socket: Box<dyn tiny_http::ReadWrite + Send>) {
        thread::spawn(move || {
            if let Err(error) = self.run(socket) {
                tracing::debug!("hmi value stream closed: {error}");
            }
        });
    }

    fn run(self, socket: Box<dyn tiny_http::ReadWrite + Send>) -> Result<(), tungstenite::Error> {
        let Self {
            resource,
            period,
            paths,
            slot: _slot,
            sessions,
            commands,
            store,
            generation,
            alarms,
            control,
            token,
        } = self;
        let mut socket = WebSocket::from_raw_socket(socket, Role::Server, None);
        let parsed: Vec<Option<MemberPath>> = paths
            .iter()
            .map(|path| MemberPath::parse(path).ok())
            .collect();
        let readable: Vec<MemberPath> = parsed.iter().flatten().cloned().collect();
        // The screen's values come from the program's Hmi.Display calls (§18 Phase 5).
        let exchange = resource.hmi_exchange();
        let _screen = ScreenGuard::open(&exchange, readable.clone());
        let mut period = period;
        send(&mut socket, &json!({ "period_ms": period.as_millis() }))?;
        // The session the page connected with (§12.4): reported now, and again when it ends.
        let session_now = || token.as_deref().and_then(|token| sessions.session(token));
        let mut session = session_now();
        send(&mut socket, &session_message(session.as_ref()))?;
        let session_id = session.as_ref().map(|info| info.id.clone());
        let mut seen = commands.last_seq();
        let mut activations = generation.load(Ordering::Acquire);
        // The list when the stream opens and whenever it changes; records from now on.
        let mut alarm_generation = None;
        let mut alarm_seq: Option<u64> = None;
        let mut sent_calls = 0;
        // What the page has: each path's value as last sent, and the missing paths, so an update
        // carries only what changed (§18 phase 5 item 4).
        let mut shown: BTreeMap<&str, String> = BTreeMap::new();
        let mut shown_missing: Vec<&str> = Vec::new();
        let mut shown_simulation: Option<Value> = None;
        loop {
            let started = Instant::now();
            // A deployment or rollback: the page tells its user to reload (§16.2.6).
            let now = generation.load(Ordering::Acquire);
            if now != activations {
                activations = now;
                let version =
                    store.read(|root| super::files::HmiFiles::new(root.to_path_buf()).version());
                send(&mut socket, &json!({ "bundle": version }))?;
            }
            if session.is_some() {
                session = session_now();
                if session.is_none() {
                    send(&mut socket, &session_message(None))?;
                    // The alarms' actions follow the session: send the list again (§5.7 item 4).
                    alarm_generation = None;
                }
            }
            // The simulation the header shows (specification 33, §6.1), sent when it changes.
            if let Some(now) = simulation_status(&control) {
                if shown_simulation.as_ref() != Some(&now) {
                    send(&mut socket, &json!({ "simulation": &now }))?;
                    shown_simulation = Some(now);
                }
            }

            // Values arrive as often as the program calls Hmi.Display; the page is told.
            let wanted = exchange
                .period()
                .map_or(period, |measured| measured.max(MIN_PERIOD));
            if wanted.abs_diff(period) * 4 > period {
                period = wanted;
                send(&mut socket, &json!({ "period_ms": period.as_millis() }))?;
            }
            // The values right after the program's next call; without one within a period nothing
            // is sent, so the display turns stale.
            let calls = exchange.wait_for_call(sent_calls, period);
            let fresh = calls != sent_calls;
            sent_calls = calls;
            if let Some(read) = exchange.read(&readable).filter(|_| fresh) {
                let mut answers = read.values.into_iter().zip(read.present);
                let mut changed = BTreeMap::new();
                let mut missing = Vec::new();
                for (path, parsed) in paths.iter().zip(&parsed) {
                    let (value, present) = match parsed {
                        Some(_) => answers.next().unwrap_or((None, false)),
                        None => (None, false),
                    };
                    if !present {
                        missing.push(path.as_str());
                    }
                    let text = serde_json::to_string(&value).unwrap_or_default();
                    if shown.get(path.as_str()) != Some(&text) {
                        shown.insert(path.as_str(), text);
                        changed.insert(path.as_str(), value);
                    }
                }
                // Nothing changed: the cycle count alone, so the page still sees the program run.
                let missing_changed = missing != shown_missing;
                let update = update_text(read.cycle, &changed, missing_changed.then_some(&missing));
                if missing_changed {
                    shown_missing = missing;
                }
                send_text(&mut socket, update)?;
            }
            if let Some(snapshot) = alarms.snapshot(alarm_generation, alarm_seq.unwrap_or(u64::MAX))
            {
                if let Some(summary) = snapshot.summary {
                    let role = session.as_ref().map(|session| session.role);
                    send(&mut socket, &json!({ "alarms": summary.for_role(role) }))?;
                }
                if alarm_seq.is_some() && !snapshot.events.is_empty() {
                    send(&mut socket, &json!({ "events": snapshot.events }))?;
                }
                alarm_generation = Some(snapshot.generation);
                alarm_seq = Some(snapshot.last_seq);
            }
            // The session's commands' later outcomes, after the values they were read with, so a
            // confirmed value is on screen when its outcome arrives; even after the session ended.
            if let Some(id) = &session_id {
                for event in commands.events_after(id, seen) {
                    seen = event.seq;
                    send(&mut socket, &json!({ "command": event.answer }))?;
                }
            }
            // At most four updates a second (§11.1).
            thread::sleep(MIN_PERIOD.saturating_sub(started.elapsed()));
        }
    }
}

/// `{"session": {"user", "role"}}`, or `{"session": null}` when there is none.
fn session_message(session: Option<&SessionInfo>) -> Value {
    match session {
        Some(info) => {
            json!({ "session": { "user": info.user.as_str(), "role": info.role.as_str() } })
        }
        None => json!({ "session": null }),
    }
}

/// The simulation fields `control.status` reports (specification 33, §6.1): whether simulation is
/// on, the configured speed, and the achieved speed (`null` until it is measured). They are read
/// from the state `control.status` reads, so the header and the status agree. `None` while that
/// state cannot be read.
fn simulation_status(control: &ControlState) -> Option<Value> {
    let (enabled, time_scale) = control
        .settings
        .lock()
        .map(|settings| (settings.simulation.enabled, settings.simulation.time_scale))
        .ok()?;
    let achieved = control
        .metrics
        .lock()
        .ok()
        .and_then(|metrics| metrics.simulation_achieved_time_scale());
    Some(json!({
        "enabled": enabled,
        "time_scale": time_scale,
        "achieved_time_scale": achieved,
    }))
}

/// The `Sec-WebSocket-Accept` answer to a websocket upgrade request.
pub(super) fn accept_key(request: &tiny_http::Request) -> Result<String, &'static str> {
    let upgrade = header_value(request, "Upgrade").ok_or("missing Upgrade header")?;
    if !upgrade.eq_ignore_ascii_case("websocket") {
        return Err("not a websocket upgrade");
    }
    let connection = header_value(request, "Connection").ok_or("missing Connection header")?;
    if !connection.to_ascii_lowercase().contains("upgrade") {
        return Err("not a Connection upgrade");
    }
    let key = header_value(request, "Sec-WebSocket-Key").ok_or("missing Sec-WebSocket-Key")?;
    Ok(tungstenite::handshake::derive_accept_key(key.as_bytes()))
}

/// An open screen in the exchange, closed when the stream ends.
struct ScreenGuard<'a> {
    exchange: &'a HmiExchange,
    screen: ScreenId,
}

impl<'a> ScreenGuard<'a> {
    fn open(exchange: &'a HmiExchange, paths: Vec<MemberPath>) -> Self {
        let screen = exchange.open_screen(paths);
        Self { exchange, screen }
    }
}

impl Drop for ScreenGuard<'_> {
    fn drop(&mut self) {
        self.exchange.close_screen(self.screen);
    }
}

/// One update: the cycle count and each path's value, `null` where it is unknown, and the paths
/// the running program does not have (§16.3). Members keep their declared order, so the message
/// is written from the ordered values, not a JSON map.
/// An update: the cycle count, the values that changed, and the missing paths when they changed.
fn update_text(
    cycle: u64,
    values: &BTreeMap<&str, Option<WireValue>>,
    missing: Option<&Vec<&str>>,
) -> String {
    #[derive(serde::Serialize)]
    struct Update<'a> {
        cycle: u64,
        values: &'a BTreeMap<&'a str, Option<WireValue>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        missing: Option<&'a Vec<&'a str>>,
    }
    serde_json::to_string(&Update {
        cycle,
        values,
        missing,
    })
    .unwrap_or_default()
}

fn send(
    socket: &mut WebSocket<Box<dyn tiny_http::ReadWrite + Send>>,
    message: &Value,
) -> Result<(), tungstenite::Error> {
    send_text(socket, message.to_string())
}

fn send_text(
    socket: &mut WebSocket<Box<dyn tiny_http::ReadWrite + Send>>,
    text: String,
) -> Result<(), tungstenite::Error> {
    socket.send(Message::text(text))
}
