//! The state of every link the runtime runs, written into the program's `Comm.Link` blocks at the
//! start of each scan (specification 42): its state and since when, its counters and response
//! time, and what its blocks ask of it: to switch an `io.toml` driver's link off, to test it, to
//! start its counters again.

use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;

use crate::ads::diagnostics::{AdsConnectionStatus, AdsConnectionStatusState};
use crate::hmi::instance_walk::instances_of;
use crate::hmi::member_paths::find_ignoring_case;
use crate::io::IoDriverHealth;
use crate::memory::{InstanceId, VariableStorage};
use crate::opcua::{OpcUaClientConnectionState, OpcUaClientConnectionStatus};
use crate::value::{Duration, EnumValue, Value};

use super::ads_subsystem::AdsSubsystem;
use super::io_subsystem::{IoSubsystem, LinkIo};
use super::opcua_client_subsystem::OpcUaClientSubsystem;
use super::RuntimeMetadata;
use crate::debug::ScanWrites;

/// The block the runtime fills (§4).
pub(crate) const LINK_BLOCK: &str = "Comm.Link";
/// Most blocks filled.
const MAX_LINKS: usize = 1_000;
/// How often the program's blocks are found again (§4 item 5).
const WALK_PERIOD_MS: i64 = 1_000;
/// The length of `LastError` and `TestResult`.
const TEXT_LENGTH: usize = 120;
/// Deepest base chain followed.
const MAX_DEPTH: usize = 32;

/// A link's state (§3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkState {
    Unknown,
    Connecting,
    Connected,
    Degraded,
    Failed,
    Disabled,
}

impl LinkState {
    fn name_and_value(self) -> (&'static str, i64) {
        match self {
            Self::Unknown => ("Unknown", 0),
            Self::Connecting => ("Connecting", 1),
            Self::Connected => ("Connected", 2),
            Self::Degraded => ("Degraded", 3),
            Self::Failed => ("Failed", 4),
            Self::Disabled => ("Disabled", 5),
        }
    }

    /// Data is exchanged.
    pub(super) fn exchanges(self) -> bool {
        matches!(self, Self::Connected | Self::Degraded)
    }
}

/// One link's state as its source reports it in this scan.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LinkReading {
    pub(crate) name: SmolStr,
    pub(crate) state: LinkState,
    pub(crate) error: Option<String>,
    /// How long the exchange took, for an `io.toml` driver; 0 for a client connection.
    pub(crate) response_ms: i64,
    /// The protocol's own state in words (§4 item 9).
    pub(crate) detail: Option<String>,
}

/// An I/O driver's health as its link's state (§3 item 1); switched off, it is `Disabled`.
pub(crate) fn io_reading(io: &LinkIo<'_>) -> LinkReading {
    let (state, error) = match (&io.health, io.enabled) {
        (_, false) => (LinkState::Disabled, None),
        (IoDriverHealth::Ok, true) => (LinkState::Connected, None),
        (IoDriverHealth::Degraded { error }, true) => {
            (LinkState::Degraded, Some(error.to_string()))
        }
        (IoDriverHealth::Faulted { error }, true) => (LinkState::Failed, Some(error.to_string())),
    };
    LinkReading {
        name: io.link.clone(),
        state,
        error,
        response_ms: i64::try_from(io.response.as_millis()).unwrap_or(i64::MAX),
        detail: io.detail.as_ref().map(ToString::to_string),
    }
}

/// An ADS client connection's report as its link's state (§3 item 2).
pub(crate) fn ads_reading(connection: &AdsConnectionStatus) -> LinkReading {
    let state = match connection.state {
        AdsConnectionStatusState::Connected if connection.degraded_points > 0 => {
            LinkState::Degraded
        }
        AdsConnectionStatusState::Connected => LinkState::Connected,
        AdsConnectionStatusState::Stale => LinkState::Degraded,
        AdsConnectionStatusState::NotReady => LinkState::Connecting,
        AdsConnectionStatusState::Reconnecting | AdsConnectionStatusState::Faulted => {
            LinkState::Failed
        }
        AdsConnectionStatusState::Disabled => LinkState::Disabled,
        AdsConnectionStatusState::Unknown => LinkState::Unknown,
    };
    LinkReading {
        name: SmolStr::new(format!("ads:{}", connection.name)),
        state,
        error: (!state.exchanges() || state == LinkState::Degraded)
            .then(|| connection.summary.clone())
            .filter(|summary| !summary.is_empty()),
        response_ms: 0,
        detail: Some(connection.summary.clone()).filter(|summary| !summary.is_empty()),
    }
}

/// An OPC UA client connection's report as its link's state (§3 item 3).
pub(crate) fn opcua_reading(connection: &OpcUaClientConnectionStatus) -> LinkReading {
    let state = match connection.state {
        OpcUaClientConnectionState::Connected if connection.degraded_points > 0 => {
            LinkState::Degraded
        }
        OpcUaClientConnectionState::Connected => LinkState::Connected,
        OpcUaClientConnectionState::Configured | OpcUaClientConnectionState::Connecting => {
            LinkState::Connecting
        }
        OpcUaClientConnectionState::Stale => LinkState::Degraded,
        OpcUaClientConnectionState::Reconnecting | OpcUaClientConnectionState::Faulted => {
            LinkState::Failed
        }
        OpcUaClientConnectionState::Disabled => LinkState::Disabled,
    };
    LinkReading {
        name: SmolStr::new(format!("opcua_client:{}", connection.name)),
        state,
        error: (!state.exchanges() || state == LinkState::Degraded)
            .then(|| connection.detail.clone())
            .filter(|detail| !detail.is_empty()),
        response_ms: 0,
        detail: Some(connection.detail.clone()).filter(|detail| !detail.is_empty()),
    }
}

/// What the runtime keeps of a link between scans.
#[derive(Debug)]
struct Tracked {
    state: LinkState,
    /// When the state last changed.
    since_ms: i64,
    /// When data was last exchanged, or else when the link was first seen.
    last_good_ms: i64,
    last_error: String,
    /// The scans with data and the scans `Failed`, since the start or a reset (§4 item 8).
    exchanges: u64,
    failures: u64,
    response_ms: i64,
    detail: String,
}

/// What one block asks of its link in this scan: its name, whether the link may run, and the
/// rising edges of its `Test` and `Reset`.
#[derive(Debug)]
pub(super) struct LinkRequest {
    pub(super) name: String,
    pub(super) enable: bool,
    pub(super) test: bool,
    pub(super) reset: bool,
}

/// The links' states and the program's `Comm.Link` blocks.
#[derive(Debug, Default)]
pub(super) struct LinkSubsystem {
    disabled: Vec<SmolStr>,
    tracked: FxHashMap<SmolStr, Tracked>,
    /// When the runtime first looked for each name it does not run (§4 item 3).
    unknown_since: FxHashMap<String, i64>,
    blocks: Vec<InstanceId>,
    walked_ms: Option<i64>,
    /// Each block's `Test` and `Reset` in the last scan, for their rising edges.
    inputs: FxHashMap<InstanceId, (bool, bool)>,
    /// The outcome of each link's last test, by name (§4 item 7).
    tests: FxHashMap<String, String>,
}

impl LinkSubsystem {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn add_disabled(&mut self, link: SmolStr) {
        if !self.disabled.contains(&link) {
            self.disabled.push(link);
        }
    }

    /// What each block asks of its link, the program having called it in the last scan.
    fn requests(&mut self, storage: &VariableStorage) -> Vec<LinkRequest> {
        let mut requests = Vec::with_capacity(self.blocks.len());
        for &part in &self.blocks {
            let Some(instance) = storage.get_instance(part) else {
                continue;
            };
            let flag =
                |name: &str, otherwise: bool| match find_ignoring_case(&instance.variables, name) {
                    Some(Value::Bool(value)) => *value,
                    _ => otherwise,
                };
            let name = match find_ignoring_case(&instance.variables, "Name") {
                Some(Value::String(name)) => name.trim().to_string(),
                _ => String::new(),
            };
            let (test, reset) = (flag("Test", false), flag("Reset", false));
            let (was_test, was_reset) = self
                .inputs
                .insert(part, (test, reset))
                .unwrap_or((false, false));
            requests.push(LinkRequest {
                name,
                enable: flag("Enable", true),
                test: test && !was_test,
                reset: reset && !was_reset,
            });
        }
        requests
    }

    /// Notes this scan's readings and the tests' outcomes, starts the asked counters again, then
    /// writes every `Comm.Link` block of the program.
    fn apply(
        &mut self,
        writes: &mut dyn ScanWrites,
        readings: Vec<LinkReading>,
        tests: Vec<(String, String)>,
        resets: &FxHashSet<String>,
        now_ms: i64,
    ) {
        let disabled = self.disabled.iter().map(|name| LinkReading {
            name: name.clone(),
            state: LinkState::Disabled,
            error: None,
            response_ms: 0,
            detail: None,
        });
        let all: Vec<LinkReading> = readings.into_iter().chain(disabled).collect();
        for reading in all {
            self.note(reading, now_ms);
        }
        for (name, tracked) in &mut self.tracked {
            if resets.contains(name.as_str()) {
                tracked.exchanges = 0;
                tracked.failures = 0;
            }
        }
        self.tests.extend(tests);
        for id in self.blocks.clone() {
            self.fill(writes, id, now_ms);
        }
    }

    fn note(&mut self, reading: LinkReading, now_ms: i64) {
        let tracked = self.tracked.entry(reading.name).or_insert(Tracked {
            state: reading.state,
            since_ms: now_ms,
            last_good_ms: now_ms,
            last_error: String::new(),
            exchanges: 0,
            failures: 0,
            response_ms: 0,
            detail: String::new(),
        });
        if tracked.state != reading.state {
            tracked.state = reading.state;
            tracked.since_ms = now_ms;
        }
        if reading.state.exchanges() {
            tracked.last_good_ms = now_ms;
            tracked.exchanges = tracked.exchanges.saturating_add(1);
        }
        if reading.state == LinkState::Failed {
            tracked.failures = tracked.failures.saturating_add(1);
        }
        tracked.response_ms = reading.response_ms;
        tracked.detail = reading.detail.unwrap_or_default();
        if let Some(error) = reading.error.filter(|error| !error.is_empty()) {
            tracked.last_error = error;
        }
    }

    /// Finds the blocks again once a second, and at once when a known block is gone or no longer
    /// a link (an online change).
    fn find_blocks(
        &mut self,
        storage: &VariableStorage,
        holdings: &crate::hmi::instance_holdings::Holdings<'_>,
        is_program: &dyn Fn(&str) -> bool,
        now_ms: i64,
    ) {
        let gone = self
            .blocks
            .iter()
            .any(|id| link_part(storage, *id).is_none());
        let due = self
            .walked_ms
            .is_none_or(|walked| now_ms < walked || now_ms - walked >= WALK_PERIOD_MS);
        if gone || due {
            self.blocks = instances_of(storage, holdings, LINK_BLOCK, MAX_LINKS, is_program)
                .into_iter()
                .filter_map(|found| link_part(storage, found.id))
                .collect();
            self.walked_ms = Some(now_ms);
            let blocks: FxHashSet<InstanceId> = self.blocks.iter().copied().collect();
            self.inputs.retain(|id, _| blocks.contains(id));
        }
    }

    /// Writes one block's outputs from the link its `Name` names (§4).
    fn fill(&mut self, writes: &mut dyn ScanWrites, part: InstanceId, now_ms: i64) {
        let name = match writes
            .storage()
            .get_instance(part)
            .and_then(|instance| find_ignoring_case(&instance.variables, "Name"))
        {
            Some(Value::String(name)) => name.trim().to_string(),
            _ => String::new(),
        };
        let test = self.tests.get(name.as_str()).cloned().unwrap_or_default();
        let (state, known, state_ms, good_ms, error, counts) = match self.tracked.get(name.as_str())
        {
            Some(link) => (
                link.state,
                true,
                now_ms.saturating_sub(link.since_ms),
                if link.state.exchanges() {
                    0
                } else {
                    now_ms.saturating_sub(link.last_good_ms)
                },
                link.last_error.clone(),
                (link.exchanges, link.failures, link.response_ms),
            ),
            None if name.is_empty() => (LinkState::Unknown, false, 0, 0, String::new(), (0, 0, 0)),
            None => {
                let since = *self.unknown_since.entry(name.clone()).or_insert(now_ms);
                let age = now_ms.saturating_sub(since);
                (
                    LinkState::Unknown,
                    false,
                    age,
                    age,
                    format!("no link {name} in this runtime"),
                    (0, 0, 0),
                )
            }
        };
        let (word, value) = state.name_and_value();
        if let Some(Value::Enum(current)) = writes
            .storage()
            .get_instance(part)
            .and_then(|instance| find_ignoring_case(&instance.variables, "State"))
        {
            let next = EnumValue::from_canonical_parts(
                current.type_name().clone(),
                SmolStr::new(word),
                value,
            );
            set_member(writes, part, "State", Value::Enum(Box::new(next)));
        }
        set_member(writes, part, "Connected", Value::Bool(state.exchanges()));
        set_member(
            writes,
            part,
            "Disabled",
            Value::Bool(state == LinkState::Disabled),
        );
        set_member(writes, part, "Known", Value::Bool(known));
        set_member(
            writes,
            part,
            "StateTime",
            Value::Time(Duration::from_millis(state_ms)),
        );
        set_member(
            writes,
            part,
            "LastGoodAge",
            Value::Time(Duration::from_millis(good_ms)),
        );
        set_member(
            writes,
            part,
            "LastError",
            Value::String(SmolStr::new(clip(&error))),
        );
        let (exchanges, failures, response_ms) = counts;
        set_member(
            writes,
            part,
            "Exchanges",
            Value::UDInt(u32::try_from(exchanges).unwrap_or(u32::MAX)),
        );
        set_member(
            writes,
            part,
            "Failures",
            Value::UDInt(u32::try_from(failures).unwrap_or(u32::MAX)),
        );
        set_member(
            writes,
            part,
            "ResponseTime",
            Value::Time(Duration::from_millis(response_ms)),
        );
        set_member(
            writes,
            part,
            "TestResult",
            Value::String(SmolStr::new(clip(&test))),
        );
        let detail = self
            .tracked
            .get(name.as_str())
            .map(|link| link.detail.clone())
            .unwrap_or_default();
        set_member(
            writes,
            part,
            "Detail",
            Value::String(SmolStr::new(clip(&detail))),
        );
    }
}

/// A text cut to the length of the block's strings.
fn clip(text: &str) -> String {
    text.chars().take(TEXT_LENGTH).collect()
}

/// The `Comm.Link` part of an instance whose type is, or extends, the block.
fn link_part(storage: &VariableStorage, id: InstanceId) -> Option<InstanceId> {
    let mut current = Some(id);
    for _ in 0..MAX_DEPTH {
        let id = current?;
        let instance = storage.get_instance(id)?;
        if instance.type_name.eq_ignore_ascii_case(LINK_BLOCK) {
            return Some(id);
        }
        current = instance.parent;
    }
    None
}

/// Writes a member the block declares, under its declared name, through the scan's checked
/// writes. A member the runtime refuses keeps its value; the block shows the rest.
fn set_member(writes: &mut dyn ScanWrites, part: InstanceId, name: &str, value: Value) {
    let storage = writes.storage();
    let Some(reference) = storage
        .get_instance(part)
        .and_then(|instance| {
            instance
                .variables
                .keys()
                .find(|key| key.eq_ignore_ascii_case(name))
                .cloned()
        })
        .and_then(|key| storage.ref_for_instance(part, &key))
    else {
        return;
    };
    writes.write(&reference, value);
}

/// The scan's services the link states come from.
pub(super) struct LinkSources<'a> {
    pub(super) io: &'a mut IoSubsystem,
    pub(super) ads: &'a AdsSubsystem,
    pub(super) opcua: &'a OpcUaClientSubsystem,
    pub(super) metadata: &'a RuntimeMetadata,
    /// The size of the input image, for a link test's spare image.
    pub(super) input_len: usize,
}

impl LinkSubsystem {
    /// Fills the program's `Comm.Link` blocks with the state of their links, after the inputs
    /// are read and before the program runs (§4 item 1); first it does what the blocks asked in
    /// the last scan: switching `io.toml` drivers' links off or on, testing them, starting their
    /// counters again. A program without the block is skipped.
    pub(super) fn apply_link_states(
        &mut self,
        sources: LinkSources<'_>,
        writes: &mut dyn ScanWrites,
        now_ms: i64,
    ) {
        let LinkSources {
            io,
            ads,
            opcua,
            metadata,
            input_len,
        } = sources;
        if find_ignoring_case(metadata.function_blocks(), LINK_BLOCK).is_none() {
            return;
        }
        {
            let programs = metadata.program_templates();
            let is_program = |name: &str| find_ignoring_case(programs, name).is_some();
            let holdings = crate::hmi::instance_holdings::Holdings::new(
                metadata.function_blocks(),
                metadata.classes(),
                metadata.program_templates(),
                metadata.registry(),
            );
            self.find_blocks(writes.storage(), &holdings, &is_program, now_ms);
        }
        let requests = self.requests(writes.storage());
        // A link runs while none of its blocks switches it off (§4 item 6).
        let off: FxHashSet<&str> = requests
            .iter()
            .filter(|request| !request.enable)
            .map(|request| request.name.as_str())
            .collect();
        let links: Vec<SmolStr> = io.link_ios().map(|io| io.link.clone()).collect();
        for link in &links {
            io.set_link_enabled(link, !off.contains(link.as_str()));
        }
        let mut tests = Vec::new();
        for request in requests.iter().filter(|request| request.test) {
            let outcome = match io.test_link(&request.name, input_len) {
                Some(Ok(time)) => format!("answered in {} ms", time.as_millis()),
                Some(Err(error)) => error,
                None if request.name.starts_with("ads:")
                    || request.name.starts_with("opcua_client:") =>
                {
                    "a client connection is not tested this way".to_string()
                }
                None => format!("no link {} in this runtime", request.name),
            };
            tests.push((request.name.clone(), outcome));
        }
        let resets: FxHashSet<String> = requests
            .iter()
            .filter(|request| request.reset)
            .map(|request| request.name.clone())
            .collect();
        let mut readings: Vec<LinkReading> = io.link_ios().map(|io| io_reading(&io)).collect();
        if ads.connection_count() > 0 {
            readings.extend(ads.status_report().connections.iter().map(ads_reading));
        }
        if opcua.connection_count() > 0 {
            readings.extend(opcua.status_report().connections.iter().map(opcua_reading));
        }
        self.apply(writes, readings, tests, &resets, now_ms);
    }
}

#[cfg(test)]
#[path = "link_subsystem_tests.rs"]
mod tests;
