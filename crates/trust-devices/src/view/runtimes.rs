//! The runtime cards and their inspectors (specification 25 §5.1, §5.2, §5.5 and §5.12): every
//! runtime is drawn and handled the same way, whether it is the simulator, a runtime this
//! computer manages, a runtime on another computer or the truST firmware of a chip.
//!
//! - The name: the simulator is always `Simulator`, whatever its resource is called.
//! - The state: the word of the runtime's lifecycle with the tone of its rolled-up health. A
//!   runtime is never shown as stopped without the evidence: one that does not answer is
//!   `Unknown`, one that did not answer in time and a managed one whose status cannot be read are
//!   `Status unavailable`.
//! - The inspector: the actions first (`lifecycle`), then the state and detail, run target, mode,
//!   cycle time, address, truST version, up time, faults, access, connections and container.

use trust_runtime_client::TargetProbe;

use super::address;
use super::chips;
use super::health::{runtime_rollup, Health};
use super::lifecycle::{self, Contact, Owner, Subject};
use super::list;
use super::status::{ProgramState, RuntimeStatus};
use super::types::{
    EmptyHint, HoverCard, Inspector, ItemId, Look, Pill, Row, Runtime, RuntimeIcon, Section, Tone,
};
use crate::gather::Reach;
use crate::inputs::Facts;
use crate::request::RunMode;
use crate::texts::graph as texts;
use crate::topology::{self, ManagedState, RuntimeOrigin, SimulatorState};

/// Where a runtime is drawn.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Place<'a> {
    /// The heading of its computer.
    pub(crate) computer: &'a str,
    /// The container it runs in.
    pub(crate) container: Option<&'a str>,
    /// Whether that container is drawn as a frame; otherwise it is a chip on the card.
    pub(crate) container_framed: bool,
}

/// The card of `runtime`, drawn at `place`.
pub(crate) fn runtime(runtime: &topology::Runtime, place: Place<'_>, facts: &Facts) -> Runtime {
    let id = ItemId(runtime.id.clone());
    let owner = owner(runtime, facts);
    let name = display_name(runtime, owner);
    let (contact, endpoint) = contact(runtime, owner, facts);
    let answered = matches!(contact, Contact::Answered(_));
    let project = is_project(runtime, facts, endpoint.as_deref());
    let subject = subject_of(
        runtime,
        &id,
        &name,
        owner,
        contact,
        endpoint.as_deref(),
        facts,
    );
    let health = rollup(runtime);
    let state = state(&subject, health);
    let status = match contact {
        Contact::Answered(status) => Some(status),
        _ => None,
    };
    let icon = icon(owner, status);
    let address = endpoint.as_deref().and_then(address::host_and_port);
    let detail = clean_detail(&runtime.detail);
    Runtime {
        list: list::runtime_cells(&list::RuntimeCells {
            name: &name,
            state: &state.word,
            address: address.as_deref(),
            kind: kind_word(icon),
            computer: place.computer,
            detail: &detail,
            version: status.and_then(|status| status.version.as_deref()),
            platform: status.and_then(|status| status.platform.as_deref()),
        }),
        hover: HoverCard {
            title: name.clone(),
            rows: hover_rows(runtime, &state, address.as_deref(), place, &detail),
        },
        inspector: Inspector {
            breadcrumb: texts::breadcrumb(texts::KIND_RUNTIME),
            kind: texts::KIND_RUNTIME.to_owned(),
            title: name.clone(),
            health: Some(state.tone),
            primary: lifecycle::primary_actions(&subject),
            secondary: lifecycle::secondary_actions(&subject),
            sections: vec![Section {
                heading: None,
                rows: inspector_rows(runtime, &state, status, endpoint.as_deref(), place),
            }],
            edit: None,
        },
        context_menu: lifecycle::context_menu(&subject),
        slots: lifecycle::runtime_slots(&id),
        empty_hint: empty_hint(runtime, owner, health),
        container_chip: place
            .container
            .filter(|_| !place.container_framed)
            .map(str::to_owned),
        cycle: status
            .and_then(|status| status.cycle_last_ms)
            .filter(|_| state.word == texts::RUNNING)
            .map(|last| texts::milliseconds(&format_ms(last))),
        state: Pill {
            word: state.word,
            tone: state.tone,
            live: state.live,
        },
        id,
        name,
        icon,
        address,
        control_endpoint: endpoint,
        answers: answered,
        project,
        run_target: runtime.run_target,
        look: state.look,
        connections: Vec::new(),
        dimmed: false,
        hidden: false,
    }
}

/// What the actions and the state of `runtime` are decided from.
fn subject_of<'a>(
    runtime: &'a topology::Runtime,
    id: &'a ItemId,
    name: &'a str,
    owner: Owner<'a>,
    contact: Contact<'a>,
    endpoint: Option<&'a str>,
    facts: &Facts,
) -> Subject<'a> {
    Subject {
        id,
        name,
        owner,
        contact,
        endpoint,
        run_target_endpoint: trimmed(runtime.control_endpoint.as_deref()),
        attached: runtime.attached,
        simulator: facts.simulator,
        simulator_failed: facts.simulator_failed,
        simulator_answered: runtime.origin == RuntimeOrigin::Simulator,
    }
}

/// The runtime's health, rolled up from its own and its connections' health words.
fn rollup(runtime: &topology::Runtime) -> Health {
    runtime_rollup(
        &runtime.health,
        runtime
            .endpoints
            .iter()
            .map(|endpoint| endpoint.health.as_str()),
    )
}

/// Whether the runtime's program runs, as its card's state shows it: the summary strip counts
/// it (specification 25 §5.11).
pub(crate) fn runs(runtime: &topology::Runtime, facts: &Facts) -> bool {
    let id = ItemId(runtime.id.clone());
    let owner = owner(runtime, facts);
    let name = display_name(runtime, owner);
    let (contact, endpoint) = contact(runtime, owner, facts);
    let subject = subject_of(
        runtime,
        &id,
        &name,
        owner,
        contact,
        endpoint.as_deref(),
        facts,
    );
    state(&subject, rollup(runtime)).running
}

/// Whether the runtime answered `status` this round, so that its connections can be tested
/// (specification 25 §5.13).
pub(crate) fn answers(runtime: &topology::Runtime, facts: &Facts) -> bool {
    matches!(
        contact(runtime, owner(runtime, facts), facts).0,
        Contact::Answered(_)
    )
}

/// The kind icon: the simulator, the truST firmware of a chip when the runtime reports a chip as
/// its platform, else a runtime on a computer.
fn icon(owner: Owner<'_>, status: Option<&RuntimeStatus>) -> RuntimeIcon {
    let chip = status
        .and_then(|status| status.platform.as_deref())
        .and_then(chips::model);
    match (owner, chip) {
        (Owner::Simulator, _) => RuntimeIcon::Simulator,
        (_, Some(_)) => RuntimeIcon::Chip,
        _ => RuntimeIcon::Computer,
    }
}

fn kind_word(icon: RuntimeIcon) -> &'static str {
    match icon {
        RuntimeIcon::Simulator => texts::RUNTIME_KIND_SIMULATOR,
        RuntimeIcon::Computer => texts::RUNTIME_KIND_COMPUTER,
        RuntimeIcon::Chip => texts::RUNTIME_KIND_CHIP,
    }
}

/// `No devices yet` on a runtime without connections, said only where it is certain: on the
/// simulator, and on a runtime that answers and runs. A stopped runtime may have connections
/// that show once it runs.
fn empty_hint(runtime: &topology::Runtime, owner: Owner<'_>, health: Health) -> Option<EmptyHint> {
    (runtime.endpoints.is_empty() && (owner == Owner::Simulator || health.running())).then(|| {
        EmptyHint {
            title: texts::NO_DEVICES_YET.to_owned(),
            text: texts::NO_DEVICES_HINT.to_owned(),
        }
    })
}

/// Who owns the runtime's process. The project's configured runtime is the simulator's while the
/// project runs in the simulator.
pub(crate) fn owner<'a>(runtime: &'a topology::Runtime, facts: &Facts) -> Owner<'a> {
    if let Some(managed) = &runtime.managed {
        return Owner::Managed(managed);
    }
    match runtime.origin {
        RuntimeOrigin::Simulator | RuntimeOrigin::SimulatorFallback => Owner::Simulator,
        RuntimeOrigin::Configured if facts.mode == RunMode::Simulate => Owner::Simulator,
        _ => Owner::Session,
    }
}

/// The name a runtime is shown with: `Simulator` for the simulator, whatever its Structured Text
/// resource is called (specification 25 §5.6), and `Runtime` for one without a name.
pub(crate) fn display_name(runtime: &topology::Runtime, owner: Owner<'_>) -> String {
    if owner == Owner::Simulator {
        return texts::SIMULATOR.to_owned();
    }
    let name = runtime.name.trim();
    if name.is_empty() {
        texts::KIND_RUNTIME.to_owned()
    } else {
        name.to_owned()
    }
}

fn trimmed(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

/// Whether the runtime is the open project's own runtime: the project's configured runtime, the
/// simulator that runs the project, or the runtime that answered at the project's control
/// endpoint (specification 25 §5.14). An action writes such a runtime in process into the
/// project's files while it does not answer.
fn is_project(runtime: &topology::Runtime, facts: &Facts, endpoint: Option<&str>) -> bool {
    match runtime.origin {
        RuntimeOrigin::Configured | RuntimeOrigin::Simulator | RuntimeOrigin::SimulatorFallback => {
            true
        }
        _ => facts
            .project_target
            .as_ref()
            .is_some_and(|target| trimmed(Some(target.endpoint.as_str())) == endpoint),
    }
}

/// How the panel reached the runtime, and the endpoint that its session and program actions use.
/// The project's configured runtime in online mode is the runtime at the project's control
/// endpoint, so that target's answer is its own.
fn contact<'f>(
    runtime: &topology::Runtime,
    owner: Owner<'_>,
    facts: &'f Facts,
) -> (Contact<'f>, Option<String>) {
    let own = trimmed(runtime.control_endpoint.as_deref());
    let answered = |endpoint: Option<&str>| {
        endpoint
            .and_then(|endpoint| facts.statuses.get(endpoint))
            .map_or(Contact::NotAsked, Contact::Answered)
    };
    match runtime.origin {
        RuntimeOrigin::AuthFailed(failure) => {
            return (Contact::AuthFailed(failure), own.map(str::to_owned));
        }
        RuntimeOrigin::Unreachable => return (Contact::Unreachable, own.map(str::to_owned)),
        RuntimeOrigin::Configured if owner == Owner::Session => {
            if let Some(target) = &facts.project_target {
                let endpoint = trimmed(Some(target.endpoint.as_str())).or(own);
                let contact = match &target.reach {
                    Reach::Probe(TargetProbe::Unreachable) => Contact::Unreachable,
                    Reach::Probe(TargetProbe::AuthFailed(failure)) => Contact::AuthFailed(*failure),
                    Reach::Probe(TargetProbe::MissingEndpoint) => Contact::NoEndpoint,
                    Reach::Unanswered => Contact::Unanswered,
                    Reach::Probe(TargetProbe::Reachable(_)) | Reach::Managed(_) => {
                        answered(endpoint)
                    }
                };
                return (contact, endpoint.map(str::to_owned));
            }
        }
        _ => {}
    }
    (answered(own), own.map(str::to_owned))
}

/// A runtime's state as its card and inspector show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct State {
    /// The word.
    pub(crate) word: String,
    /// Its tone.
    pub(crate) tone: Tone,
    /// Whether its dot pulses: only while the runtime runs.
    pub(crate) live: bool,
    /// Dashed while the state is not proven.
    pub(crate) look: Look,
    /// Whether the program runs, also while a connection reports an error or is degraded.
    pub(crate) running: bool,
}

impl State {
    fn fixed(word: &str, tone: Tone) -> Self {
        Self {
            word: word.to_owned(),
            tone,
            live: false,
            look: Look::Normal,
            running: false,
        }
    }

    /// A state that nothing proves yet: dashed and never green.
    fn pending(word: &str) -> Self {
        Self {
            look: Look::Pending,
            ..Self::fixed(word, Tone::Neutral)
        }
    }

    /// A running runtime, unless a connection reports an error or is degraded.
    fn running(health: Health) -> Self {
        let state = match health {
            Health::Error => Self::fixed(texts::ERROR, Tone::Error),
            Health::Degraded => Self::fixed(texts::DEGRADED, Tone::Warning),
            _ => Self {
                live: true,
                ..Self::fixed(texts::RUNNING, Tone::Ok)
            },
        };
        Self {
            running: true,
            ..state
        }
    }

    /// The state that the rolled-up health alone shows.
    fn of_health(health: Health) -> Self {
        match health {
            Health::Error | Health::Degraded | Health::Connected | Health::Simulate => {
                Self::running(health)
            }
            Health::Stopped => Self::fixed(texts::STOPPED, Tone::Neutral),
            Health::Pending => Self::pending(texts::PENDING),
            Health::Unknown => Self::pending(texts::UNKNOWN),
        }
    }
}

/// The state of a runtime (specification 25 §5.1, §5.5, §5.6 and §6.5): the managed runtime's
/// lifecycle, the simulator's, the refused or missing answer, the program state that `status`
/// names, and else the rolled-up health.
pub(crate) fn state(subject: &Subject<'_>, health: Health) -> State {
    match subject.owner {
        Owner::Managed(managed) => {
            return match managed.state {
                ManagedState::Running => State::running(health),
                ManagedState::Stopped => State::fixed(texts::STOPPED, Tone::Neutral),
                ManagedState::Starting => State::pending(texts::STARTING),
                ManagedState::Stopping => State::pending(texts::STOPPING),
                ManagedState::Unavailable => State::fixed(texts::STATUS_UNAVAILABLE, Tone::Error),
            };
        }
        Owner::Simulator if !subject.simulator_answered => {
            if subject.simulator_failed {
                return State::fixed(texts::ERROR, Tone::Error);
            }
            return match subject.simulator {
                SimulatorState::Running => State::running(health),
                SimulatorState::Starting => State::pending(texts::STARTING),
                SimulatorState::NotStarted => State::fixed(texts::STOPPED, Tone::Neutral),
            };
        }
        Owner::Simulator | Owner::Session => {}
    }
    match subject.contact {
        Contact::AuthFailed(_) => State::fixed(texts::AUTHENTICATION_FAILED, Tone::Error),
        Contact::Unreachable => State::pending(texts::UNKNOWN),
        Contact::Unanswered => State::pending(texts::STATUS_UNAVAILABLE),
        Contact::Answered(status) => match status.state {
            Some(ProgramState::Running) => State::running(health),
            Some(ProgramState::Stopped) => State::fixed(texts::STOPPED, Tone::Neutral),
            Some(ProgramState::Faulted) => State::fixed(texts::FAULTED, Tone::Error),
            Some(ProgramState::Paused) => State::fixed(texts::PAUSED, Tone::Warning),
            Some(ProgramState::Ready) => State::fixed(texts::READY, Tone::Neutral),
            Some(ProgramState::Boot) => State::pending(texts::STARTING),
            None => State::of_health(health),
        },
        Contact::NotAsked | Contact::NoEndpoint => State::of_health(health),
    }
}

/// A runtime's detail without the prefix that says where its configuration comes from:
/// `Configured in …;` or `Loaded from project files;`.
pub(crate) fn clean_detail(detail: &str) -> String {
    const LOADED: &str = "loaded from project files;";
    let detail = detail.trim();
    let lower = detail.to_ascii_lowercase();
    let rest = if lower.starts_with("configured in ") {
        detail.split_once(';').map_or(detail, |(_, rest)| rest)
    } else if lower.starts_with(LOADED) {
        detail.get(LOADED.len()..).unwrap_or(detail)
    } else {
        detail
    };
    rest.trim().to_owned()
}

/// The `State` row: the state word and, when it says more, the detail (specification 25 §5.2).
pub(crate) fn state_row(word: &str, health: &str, detail: &str) -> String {
    if detail.is_empty()
        || detail.eq_ignore_ascii_case(word)
        || detail.eq_ignore_ascii_case(health.trim())
    {
        word.to_owned()
    } else {
        texts::joined(&[word, detail])
    }
}

/// Milliseconds with three significant digits at most, without trailing zeros: `4.2`, `1.02`,
/// `10`.
pub(crate) fn format_ms(ms: f64) -> String {
    let text = if ms >= 100.0 {
        format!("{ms:.0}")
    } else if ms >= 10.0 {
        format!("{ms:.1}")
    } else {
        format!("{ms:.2}")
    };
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

fn row(label: &str, value: impl Into<String>) -> Row {
    Row {
        label: label.to_owned(),
        value: value.into(),
        tone: None,
        wide: false,
        copy: None,
    }
}

fn address_row(endpoint: Option<&str>) -> Option<Row> {
    let endpoint = endpoint?;
    if address::is_local_socket(endpoint) {
        return Some(row(texts::ROW_ADDRESS, texts::LOCAL_CONTROL_SOCKET));
    }
    let address = address::host_and_port(endpoint)?;
    Some(Row {
        copy: Some(address.clone()),
        ..row(texts::ROW_ADDRESS, address)
    })
}

fn inspector_rows(
    runtime: &topology::Runtime,
    state: &State,
    status: Option<&RuntimeStatus>,
    endpoint: Option<&str>,
    place: Place<'_>,
) -> Vec<Row> {
    let mut rows = vec![
        Row {
            tone: Some(state.tone),
            ..row(
                texts::ROW_STATE,
                state_row(&state.word, &runtime.health, &clean_detail(&runtime.detail)),
            )
        },
        row(
            texts::ROW_RUN_TARGET,
            if runtime.run_target {
                texts::SELECTED
            } else {
                texts::NOT_SELECTED
            },
        ),
    ];
    let mode = texts::mode_word(&runtime.mode);
    if !mode.is_empty() {
        rows.push(row(texts::ROW_MODE, mode));
    }
    if let Some(last) = status.and_then(|status| status.cycle_last_ms) {
        let last = format_ms(last);
        let value = status.and_then(|status| status.cycle_max_ms).map_or_else(
            || texts::milliseconds(&last),
            |max| texts::cycle_with_maximum(&last, &format_ms(max)),
        );
        rows.push(row(texts::ROW_CYCLE, value));
    }
    rows.extend(address_row(endpoint));
    if let Some(status) = status {
        if let Some(version) = &status.version {
            rows.push(row(texts::ROW_VERSION, version.clone()));
        }
        if let Some(uptime) = status.uptime_ms {
            rows.push(row(texts::ROW_UP_FOR, texts::duration(uptime / 1_000)));
        }
        if let Some(faults) = status.faults {
            rows.push(row(texts::ROW_FAULTS, texts::faults(faults)));
        }
        if let Some(role) = status.role {
            rows.push(row(
                texts::ROW_ACCESS,
                texts::access(role.as_str(), status.may_write, status.may_force),
            ));
        }
    }
    rows.push(row(
        texts::ROW_CONNECTIONS,
        runtime.endpoints.len().to_string(),
    ));
    if let Some(container) = place.container {
        rows.push(row(texts::ROW_CONTAINER, container));
    }
    rows
}

fn hover_rows(
    runtime: &topology::Runtime,
    state: &State,
    address: Option<&str>,
    place: Place<'_>,
    detail: &str,
) -> Vec<Row> {
    let mut rows = vec![row(texts::ROW_STATE, state.word.clone())];
    let mode = texts::mode_word(&runtime.mode);
    if !mode.is_empty() {
        rows.push(row(texts::ROW_MODE, mode));
    }
    if let Some(address) = address {
        rows.push(row(texts::ROW_ADDRESS, address));
    }
    rows.push(row(
        texts::ROW_CONNECTIONS,
        runtime.endpoints.len().to_string(),
    ));
    if let Some(container) = place.container {
        rows.push(row(texts::ROW_CONTAINER, container));
    }
    if !detail.is_empty() {
        rows.push(row(texts::ROW_DETAIL, detail));
    }
    rows
}
