//! The state of every link the runtime runs, written into the program's `Comm.Link` blocks after
//! the inputs are read and before the program runs, each scan (specification 42): its state and
//! since when, its counters and response time, and what its blocks ask of it: to switch an
//! `io.toml` driver's link off, to test it, to start its counters again. The blocks are found and
//! their members reached through the image's symbol and type tables, and written through the
//! runtime's checks at that access point (specification 44 §13.3).

use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use trust_runtime_core::native::image::{type_kind, Header, Image, TypeRecord};

use crate::ads::diagnostics::{AdsConnectionStatus, AdsConnectionStatusState};
use crate::io::IoDriverHealth;
use crate::opcua::{OpcUaClientConnectionState, OpcUaClientConnectionStatus};
use crate::value::{Duration, EnumValue, Value};

use super::ads_subsystem::AdsSubsystem;
use super::io_subsystem::{set_link_enabled, test_link, IoDriverEntry, LinkIo};
use super::opcua_client_subsystem::OpcUaClientSubsystem;
use super::program_access::{Located, ProgramRead, ProgramWrite, Walk};

/// The block the runtime fills (§4).
pub(crate) const LINK_BLOCK: &str = "Comm.Link";
/// Most blocks filled.
const MAX_LINKS: usize = 1_000;
/// How often the program's blocks are found again (§4 item 5).
const WALK_PERIOD_MS: i64 = 1_000;
/// The length of `LastError` and `TestResult`.
const TEXT_LENGTH: usize = 120;

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
    driver_reading(
        io.link.clone(),
        &io.health,
        io.enabled,
        io.response,
        io.detail.as_deref(),
    )
}

/// The reading of the driver of `link` with `health`, switched on when `enabled`, whose last read
/// and write took `response`, and that words its protocol's state as `detail`.
fn driver_reading(
    link: SmolStr,
    health: &IoDriverHealth,
    enabled: bool,
    response: std::time::Duration,
    detail: Option<&str>,
) -> LinkReading {
    let (state, error) = match (health, enabled) {
        (_, false) => (LinkState::Disabled, None),
        (IoDriverHealth::Ok, true) => (LinkState::Connected, None),
        (IoDriverHealth::Degraded { error }, true) => {
            (LinkState::Degraded, Some(error.to_string()))
        }
        (IoDriverHealth::Faulted { error }, true) => (LinkState::Failed, Some(error.to_string())),
    };
    LinkReading {
        name: link,
        state,
        error,
        response_ms: i64::try_from(response.as_millis()).unwrap_or(i64::MAX),
        detail: detail.map(ToString::to_string),
    }
}

/// Each linked driver's reading in this scan, without the facts a link's state does not need.
fn driver_readings(drivers: &[IoDriverEntry]) -> impl Iterator<Item = LinkReading> + '_ {
    drivers.iter().filter_map(|entry| {
        let link = entry.link.clone()?;
        Some(driver_reading(
            link,
            &entry.driver.health(),
            entry.enabled,
            entry.read_time + entry.write_time,
            entry.driver.detail().as_deref(),
        ))
    })
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
struct LinkRequest {
    name: String,
    enable: bool,
    test: bool,
    reset: bool,
}

/// Where the members of one block that the runtime reads and writes lie in the program (§4), each
/// by its declared name; a member the block's type lacks is none, and the block shows the rest.
#[derive(Debug, Clone, Copy)]
struct Members {
    name: Option<Located>,
    enable: Option<Located>,
    test: Option<Located>,
    reset: Option<Located>,
    state: Option<Located>,
    connected: Option<Located>,
    disabled: Option<Located>,
    known: Option<Located>,
    state_time: Option<Located>,
    last_good_age: Option<Located>,
    last_error: Option<Located>,
    exchanges: Option<Located>,
    failures: Option<Located>,
    response_time: Option<Located>,
    test_result: Option<Located>,
    detail: Option<Located>,
}

impl Members {
    /// The members of the block at `at`, its inherited ones included, compared without case.
    fn of<P: ProgramRead + ?Sized>(program: &P, at: &Located) -> Self {
        let member = |name: &str| program.member(at, name);
        Self {
            name: member("Name"),
            enable: member("Enable"),
            test: member("Test"),
            reset: member("Reset"),
            state: member("State"),
            connected: member("Connected"),
            disabled: member("Disabled"),
            known: member("Known"),
            state_time: member("StateTime"),
            last_good_age: member("LastGoodAge"),
            last_error: member("LastError"),
            exchanges: member("Exchanges"),
            failures: member("Failures"),
            response_time: member("ResponseTime"),
            test_result: member("TestResult"),
            detail: member("Detail"),
        }
    }
}

/// A `Comm.Link` instance of the program, as the last walk found it (§4 item 5): where it and its
/// members lie, valid in the program it was found in, and its `Test` and `Reset` in the last scan,
/// for their rising edges.
#[derive(Debug, Clone)]
struct Block {
    at: Located,
    members: Members,
    /// The enumeration its `State` is of, which a written state names; none when its `State` is no
    /// enumeration.
    state_type: Option<SmolStr>,
    test: bool,
    reset: bool,
}

impl Block {
    fn new<P: ProgramRead + ?Sized>(program: &P, at: Located, test: bool, reset: bool) -> Self {
        let members = Members::of(program, &at);
        let state_type = members
            .state
            .and_then(|state| program.read_at(&state).ok())
            .and_then(|state| match state {
                Value::Enum(state) => Some(state.type_name().clone()),
                _ => None,
            });
        Self {
            at,
            members,
            state_type,
            test,
            reset,
        }
    }
}

/// The links' states and the program's `Comm.Link` blocks.
#[derive(Debug, Default)]
pub(super) struct LinkSubsystem {
    disabled: Vec<SmolStr>,
    tracked: FxHashMap<SmolStr, Tracked>,
    /// When the runtime first looked for each name it does not run (§4 item 3).
    unknown_since: FxHashMap<String, i64>,
    /// The program the blocks were found in, by its image's header, and whether its types hold the
    /// block at all.
    program: Option<(Header, bool)>,
    blocks: Vec<Block>,
    walked_ms: Option<i64>,
    /// The outcome of each link's last test, by name (§4 item 7).
    tests: FxHashMap<String, String>,
}

/// The scan's sources of the links' states (§3): the `io.toml` drivers, which the blocks switch off
/// and test (§4 items 6 and 7), and the ADS and OPC UA client connections.
pub(super) struct LinkSources<'a> {
    /// The drivers of `io.toml`, in their order.
    pub(super) drivers: &'a mut [IoDriverEntry],
    pub(super) ads: &'a AdsSubsystem,
    pub(super) opcua: &'a OpcUaClientSubsystem,
    /// The size of the input image the drivers read, for a link test's spare image.
    pub(super) input_len: usize,
}

impl LinkSubsystem {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Notes the link of an `io.toml` driver configured with `enabled = false`, which reads
    /// `Disabled` (§3 item 1).
    pub(super) fn add_disabled(&mut self, link: SmolStr) {
        if !self.disabled.contains(&link) {
            self.disabled.push(link);
        }
    }

    /// Fills the program's `Comm.Link` blocks with the state of their links, after the inputs
    /// are read and before the program runs (§4 item 1); first it does what the blocks asked in
    /// the last scan: switching `io.toml` drivers' links off or on, from the next read on, testing
    /// them, starting their counters again. A program without the block is skipped.
    pub(super) fn apply_link_states(
        &mut self,
        program: &mut dyn ProgramWrite,
        links: LinkSources<'_>,
        now_ms: i64,
    ) {
        let LinkSources {
            drivers,
            ads,
            opcua,
            input_len,
        } = links;
        if !self.find_blocks(&*program, now_ms) {
            return;
        }
        let requests = self.requests(&*program);
        // A link runs while none of its blocks switches it off (§4 item 6).
        let off: FxHashSet<&str> = requests
            .iter()
            .filter(|request| !request.enable)
            .map(|request| request.name.as_str())
            .collect();
        let links: Vec<SmolStr> = drivers
            .iter()
            .filter_map(|entry| entry.link.clone())
            .collect();
        for link in &links {
            set_link_enabled(drivers, link, !off.contains(link.as_str()));
        }
        let mut tests = Vec::new();
        for request in requests.iter().filter(|request| request.test) {
            let outcome = match test_link(drivers, &request.name, input_len) {
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
        let mut readings: Vec<LinkReading> = driver_readings(drivers).collect();
        if ads.connection_count() > 0 {
            readings.extend(ads.status_report().connections.iter().map(ads_reading));
        }
        if opcua.connection_count() > 0 {
            readings.extend(opcua.status_report().connections.iter().map(opcua_reading));
        }
        self.apply(program, readings, tests, &resets, now_ms);
    }

    /// Finds the program's blocks again once a second, and at once in a program other than the
    /// one they were found in (an online change), whose places are that program's alone; a block
    /// found again at its place keeps its last `Test` and `Reset`. A program whose types do not
    /// hold the block is not searched (§4 item 5): false then.
    fn find_blocks<P: ProgramRead + ?Sized>(&mut self, program: &P, now_ms: i64) -> bool {
        let image = program.program_image();
        let header = *image.header();
        let changed = self.program.is_none_or(|(found_in, _)| found_in != header);
        if changed {
            self.program = Some((header, declares_block(&image)));
        }
        if !self.program.is_some_and(|(_, declares)| declares) {
            self.blocks.clear();
            return false;
        }
        let due = changed
            || self
                .walked_ms
                .is_none_or(|walked| now_ms < walked || now_ms - walked >= WALK_PERIOD_MS);
        if !due {
            return true;
        }
        self.walked_ms = Some(now_ms);
        let found = blocks_of(program);
        let same = found.len() == self.blocks.len()
            && found
                .iter()
                .zip(&self.blocks)
                .all(|(at, block)| *at == block.at);
        if changed || !same {
            let previous = std::mem::take(&mut self.blocks);
            self.blocks = found
                .into_iter()
                .map(|at| {
                    let (test, reset) = previous
                        .iter()
                        .find(|block| block.at == at)
                        .map_or((false, false), |block| (block.test, block.reset));
                    Block::new(program, at, test, reset)
                })
                .collect();
        }
        true
    }

    /// What each block asks of its link, the program having called it in the last scan.
    fn requests<P: ProgramRead + ?Sized>(&mut self, program: &P) -> Vec<LinkRequest> {
        let mut requests = Vec::with_capacity(self.blocks.len());
        for block in &mut self.blocks {
            let flag = |at: Option<Located>, otherwise: bool| match read(program, at) {
                Some(Value::Bool(value)) => value,
                _ => otherwise,
            };
            let (test, reset) = (
                flag(block.members.test, false),
                flag(block.members.reset, false),
            );
            requests.push(LinkRequest {
                name: name_of(program, block),
                enable: flag(block.members.enable, true),
                test: test && !block.test,
                reset: reset && !block.reset,
            });
            block.test = test;
            block.reset = reset;
        }
        requests
    }

    /// Notes this scan's readings and the tests' outcomes, starts the asked counters again, then
    /// writes every `Comm.Link` block of the program.
    fn apply(
        &mut self,
        program: &mut dyn ProgramWrite,
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
        for index in 0..self.blocks.len() {
            self.fill(program, index, now_ms);
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

    /// Writes the outputs of block `index` from the link its `Name` names (§4).
    fn fill(&mut self, program: &mut dyn ProgramWrite, index: usize, now_ms: i64) {
        let Some(block) = self.blocks.get(index) else {
            return;
        };
        let name = name_of(&*program, block);
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
        let detail = self
            .tracked
            .get(name.as_str())
            .map(|link| link.detail.clone())
            .unwrap_or_default();
        let members = block.members;
        if let Some(type_name) = &block.state_type {
            let (word, value) = state.name_and_value();
            let next =
                EnumValue::from_canonical_parts(type_name.clone(), SmolStr::new(word), value);
            write(program, members.state, Value::Enum(Box::new(next)));
        }
        write(program, members.connected, Value::Bool(state.exchanges()));
        write(
            program,
            members.disabled,
            Value::Bool(state == LinkState::Disabled),
        );
        write(program, members.known, Value::Bool(known));
        write(
            program,
            members.state_time,
            Value::Time(Duration::from_millis(state_ms)),
        );
        write(
            program,
            members.last_good_age,
            Value::Time(Duration::from_millis(good_ms)),
        );
        write(
            program,
            members.last_error,
            Value::String(SmolStr::new(clip(&error))),
        );
        let (exchanges, failures, response_ms) = counts;
        write(
            program,
            members.exchanges,
            Value::UDInt(u32::try_from(exchanges).unwrap_or(u32::MAX)),
        );
        write(
            program,
            members.failures,
            Value::UDInt(u32::try_from(failures).unwrap_or(u32::MAX)),
        );
        write(
            program,
            members.response_time,
            Value::Time(Duration::from_millis(response_ms)),
        );
        write(
            program,
            members.test_result,
            Value::String(SmolStr::new(clip(&test))),
        );
        write(
            program,
            members.detail,
            Value::String(SmolStr::new(clip(&detail))),
        );
    }
}

/// Whether the program's types hold the block (§4 item 5: a program without the Comm library is
/// not searched).
fn declares_block(image: &Image<'_>) -> bool {
    image.table::<TypeRecord>().is_ok_and(|types| {
        types.iter().any(|record| {
            record.kind == type_kind::FUNCTION_BLOCK
                && image
                    .string(record.name)
                    .is_some_and(|name| name.eq_ignore_ascii_case(LINK_BLOCK))
        })
    })
}

/// Every instance of the block or of a block derived from it, at any depth of the program, where
/// it is declared, in declaration order, at most [`MAX_LINKS`] (§4 item 1); a block's own members
/// are not searched.
fn blocks_of<P: ProgramRead + ?Sized>(program: &P) -> Vec<Located> {
    let mut found = Vec::new();
    program.instances(&mut |_, at| {
        if !program.is_a(&at, LINK_BLOCK) {
            return Walk::Descend;
        }
        found.push(at);
        if found.len() >= MAX_LINKS {
            Walk::Stop
        } else {
            Walk::Skip
        }
    });
    found
}

/// The value at the member `at`, when the block has it and it reads.
fn read<P: ProgramRead + ?Sized>(program: &P, at: Option<Located>) -> Option<Value> {
    program.read_at(&at?).ok()
}

/// The link the block's `Name` names, without the spaces around it.
fn name_of<P: ProgramRead + ?Sized>(program: &P, block: &Block) -> String {
    match read(program, block.members.name) {
        Some(Value::String(name)) => name.trim().to_string(),
        _ => String::new(),
    }
}

/// Writes a member the block declares through the runtime's checks. A member the runtime refuses
/// keeps its value; the block shows the rest.
fn write(program: &mut dyn ProgramWrite, at: Option<Located>, value: Value) {
    if let Some(at) = at {
        let _ = program.write_at(&at, &value);
    }
}

/// A text cut to the length of the block's strings.
fn clip(text: &str) -> String {
    text.chars().take(TEXT_LENGTH).collect()
}

#[cfg(test)]
#[path = "link_subsystem_tests.rs"]
mod tests;
