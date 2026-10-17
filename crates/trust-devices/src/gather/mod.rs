//! Gathering the panel's inputs (design §1.5): the chosen project's files, and the answers of every
//! target, each asked on a thread of its own. A gather ends at its deadline. A target that has not
//! answered by then is `Status unavailable` for this round, never `Stopped`, and its late answer
//! is kept for the next round. The topology merge reads what a gather returns.

mod live;
mod project;

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use trust_runtime::config::fleet_manifest::FleetRuntimeLifecycleResponse;
use trust_runtime_client::{ControlError, ProbeTimeouts, TargetProbe};

use crate::request::{GatherMode, ViewRequest};
use crate::targets::{live_primary, panel_targets, Target, TargetKey, TargetOrigin};

pub use project::ProjectInputs;

/// A managed runtime's lifecycle state, which the topology and the view show.
pub use crate::topology::ManagedState;

/// How long a gather and its requests wait (design §1.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// When the whole gather ends, counted from its start.
    pub deadline: Duration,
    /// The probe's waits for the connection and for `status`.
    pub probe: ProbeTimeouts,
    /// The wait for each further request.
    pub request: Duration,
    /// The age after which an `auto` gather asks a target again.
    pub period: Duration,
}

impl Default for Budget {
    /// 2,500 ms for the gather; 400 ms to connect and 750 ms for `status`; 2,000 ms for each
    /// further request; and the panel's 1,500 ms period.
    fn default() -> Self {
        Self {
            deadline: Duration::from_millis(2_500),
            probe: ProbeTimeouts::default(),
            request: Duration::from_millis(2_000),
            period: Duration::from_millis(1_500),
        }
    }
}

/// A request's answer, or why there is none.
pub type Reply = Result<Value, ControlError>;

/// What a gather returns.
#[derive(Debug, Clone)]
pub struct Gathered {
    /// The chosen project's inputs that need no runtime.
    pub project: ProjectInputs,
    /// What each target answered, in the order of `panel_targets`.
    pub targets: Vec<TargetReport>,
}

impl Gathered {
    /// The report of the target that serves the panel's live data, by the rule of
    /// `targets::live_primary`.
    #[must_use]
    pub fn live_primary(&self) -> Option<&TargetReport> {
        let report = |origin: TargetOrigin| {
            self.targets
                .iter()
                .find(|report| report.target.origin == origin)
        };
        let probe = |origin: TargetOrigin| report(origin).and_then(TargetReport::probe);
        live_primary(probe(TargetOrigin::Project), probe(TargetOrigin::Simulator)).and_then(report)
    }
}

/// What one target answered.
#[derive(Debug, Clone)]
pub struct TargetReport {
    /// The target.
    pub target: Target,
    /// Whether and how it answered.
    pub reach: Reach,
    /// What its runtime answered after the status.
    pub live: LiveData,
}

impl TargetReport {
    /// The probe's finding, for a probed target that answered in time.
    #[must_use]
    pub fn probe(&self) -> Option<&TargetProbe> {
        if let Reach::Probe(probe) = &self.reach {
            Some(probe)
        } else {
            None
        }
    }

    /// A managed runtime's state: `Unavailable` when it did not answer in time
    /// (specification 25 §6.5). `None` for a target that is not managed.
    #[must_use]
    pub fn managed_state(&self) -> Option<ManagedState> {
        self.target.managed.as_ref()?;
        Some(match &self.reach {
            Reach::Managed(status) => status.state,
            Reach::Probe(_) | Reach::Unanswered => ManagedState::Unavailable,
        })
    }
}

/// Whether and how a target answered.
#[derive(Debug, Clone)]
pub enum Reach {
    /// The finding of the `status` probe.
    Probe(TargetProbe),
    /// A managed runtime's status, read from its fleet root.
    Managed(Box<ManagedStatus>),
    /// No answer before the gather's deadline: `Status unavailable` for this round, never
    /// `Stopped` (specification 25 §5.1 and §6.5).
    Unanswered,
}

/// A managed runtime's status (specification 25 §5.10 and §6.5).
#[derive(Debug, Clone)]
pub struct ManagedStatus {
    /// The state.
    pub state: ManagedState,
    /// What the status read reported.
    pub report: Option<FleetRuntimeLifecycleResponse>,
    /// Why the status could not be read.
    pub error: Option<String>,
}

/// What a reachable runtime answered after its status. A request that was not sent is `None`.
#[derive(Debug, Clone, Default)]
pub struct LiveData {
    /// `fleet.topology`.
    pub topology: Option<Reply>,
    /// `connectors.status`.
    pub connectors: Option<Reply>,
    /// `comm.capabilities`.
    pub capabilities: Option<Reply>,
    /// `comm.schema`, read from the project's runtime and the simulator.
    pub schema: Option<Reply>,
    /// `events.tail`, read only for the selected runtime.
    pub events: Option<Reply>,
}

/// What a panel's session keeps between gathers.
#[derive(Debug, Default)]
pub struct Cache {
    project: project::ProjectCache,
    live: Arc<LiveSlots>,
}

#[derive(Debug, Default)]
struct LiveSlots {
    slots: Mutex<HashMap<TargetKey, Slot>>,
    answered: Condvar,
}

#[derive(Debug, Default)]
struct Slot {
    last: Option<Answer>,
    /// The number of the question that is still out.
    asking: Option<u64>,
    questions: u64,
}

#[derive(Debug, Clone)]
struct Answer {
    reach: Reach,
    live: LiveData,
    at: Instant,
    with_events: bool,
}

/// Gathers the inputs of one view. The chosen project's files are read again only when one
/// changed. Every target is asked on a thread of its own, and the gather waits until all have
/// answered or `budget.deadline` has passed. `events_for` names the selected runtime's target,
/// whose recent events are read too.
pub fn gather(
    request: &ViewRequest,
    events_for: Option<&TargetKey>,
    cache: &mut Cache,
    budget: &Budget,
) -> Gathered {
    let started = Instant::now();
    let project = cache.project.read(
        &request.workspace_folders,
        request.project.as_deref(),
        request.gather,
    );
    let managed = project.managed.as_deref().unwrap_or_default();
    let mut targets = panel_targets(&request.host, project.chosen.as_deref(), managed);
    for target in &mut targets {
        target.events = events_for == Some(&target.key());
    }
    let targets = ask(&cache.live, targets, request.gather, budget, started);
    Gathered { project, targets }
}

fn ask(
    slots: &Arc<LiveSlots>,
    targets: Vec<Target>,
    mode: GatherMode,
    budget: &Budget,
    started: Instant,
) -> Vec<TargetReport> {
    let keys = targets.iter().map(Target::key).collect::<Vec<_>>();
    let mut waits = Vec::with_capacity(targets.len());
    {
        let mut table = lock(&slots.slots);
        table.retain(|key, slot| keys.contains(key) || slot.asking.is_some());
        for (target, key) in targets.iter().zip(&keys) {
            let slot = table.entry(key.clone()).or_default();
            let wait = needs_asking(slot.last.as_ref(), target, mode, budget.period, started);
            // A question that is still out is answered once; it is never asked twice.
            if wait && slot.asking.is_none() {
                slot.questions += 1;
                slot.asking = Some(slot.questions);
                if spawn(slots, key.clone(), target.clone(), *budget, slot.questions).is_err() {
                    slot.asking = None;
                }
            }
            waits.push(wait);
        }
    }
    let deadline = started.checked_add(budget.deadline).unwrap_or(started);
    let table = wait(slots, &keys, &waits, deadline);
    targets
        .into_iter()
        .zip(keys)
        .zip(waits)
        .map(|((target, key), waited)| {
            // An answer from before this round is used only when this round did not ask again.
            let (reach, live) = table
                .get(&key)
                .and_then(|slot| slot.last.as_ref())
                .filter(|answer| !waited || answer.at >= started)
                .map_or_else(
                    || (Reach::Unanswered, LiveData::default()),
                    |answer| (answer.reach.clone(), answer.live.clone()),
                );
            TargetReport {
                target,
                reach,
                live,
            }
        })
        .collect()
}

/// Whether this round asks `target` again: always for `now`, never for `none`, and for `auto`
/// when its last answer is older than `period` or lacks the events it needs now.
fn needs_asking(
    last: Option<&Answer>,
    target: &Target,
    mode: GatherMode,
    period: Duration,
    now: Instant,
) -> bool {
    match mode {
        GatherMode::Now => true,
        GatherMode::Cached => false,
        GatherMode::Auto => last.is_none_or(|answer| {
            now.saturating_duration_since(answer.at) >= period
                || (target.events && !answer.with_events)
        }),
    }
}

fn spawn(
    slots: &Arc<LiveSlots>,
    key: TargetKey,
    target: Target,
    budget: Budget,
    question: u64,
) -> std::io::Result<()> {
    let slots = Arc::clone(slots);
    thread::Builder::new()
        .name("devices-gather".to_string())
        .spawn(move || {
            let (reach, live) = live::ask_target(&target, &budget);
            let mut table = lock(&slots.slots);
            if let Some(slot) = table
                .get_mut(&key)
                .filter(|slot| slot.asking == Some(question))
            {
                slot.last = Some(Answer {
                    reach,
                    live,
                    at: Instant::now(),
                    with_events: target.events,
                });
                slot.asking = None;
            }
            drop(table);
            slots.answered.notify_all();
        })
        .map(|_| ())
}

/// Waits until no target that this round waits for has a question out, or until `deadline`.
fn wait<'a>(
    slots: &'a LiveSlots,
    keys: &[TargetKey],
    waits: &[bool],
    deadline: Instant,
) -> MutexGuard<'a, HashMap<TargetKey, Slot>> {
    let mut table = lock(&slots.slots);
    loop {
        let pending = keys.iter().zip(waits).any(|(key, waited)| {
            *waited && table.get(key).is_some_and(|slot| slot.asking.is_some())
        });
        let left = deadline.saturating_duration_since(Instant::now());
        if !pending || left.is_zero() {
            return table;
        }
        table = slots
            .answered
            .wait_timeout(table, left)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
impl Cache {
    /// Whether a question to a target is still out.
    pub(crate) fn asking(&self) -> bool {
        lock(&self.live.slots)
            .values()
            .any(|slot| slot.asking.is_some())
    }
}

#[cfg(test)]
mod managed_tests;
#[cfg(test)]
mod tests;
