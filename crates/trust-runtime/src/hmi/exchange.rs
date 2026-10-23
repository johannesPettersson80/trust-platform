//! The HMI exchange (specification 39, §18 Phase 5): where the HMI meets the running program. The
//! program calls `Hmi.Display` in a task of its choice; after that task the runtime serves the
//! exchange: it applies the commands the screens queued, copies the values they show and reads the
//! alarms. The web server reads the copy and queues commands without waiting on the PLC thread, so
//! the task's interval and priority are the HMI's and nothing of the HMI runs between scans.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use rustc_hash::FxHashMap;

mod pulse_ends;
use pulse_ends::PulseEnds;
pub(crate) use pulse_ends::{PulseEnd, PulseEndReport};
mod settings_jobs;
use settings_jobs::PendingSettings;
pub(crate) use settings_jobs::{SettingChange, SettingsJob, SettingsOutcome};

use crate::debug::RequestQueueFull;
use crate::hmi::alarms::{read_alarms, AlarmEvent, AlarmSnapshot, OperatorAction, RuntimeAlarms};
use crate::hmi::commands::write::{write_point, WriteAction, Written};
use crate::hmi::display_values::{self, HmiRead};
use crate::hmi::member_paths::MemberPath;
use crate::runtime::hmi_stale::{silent_age, SilentInput};
use crate::runtime::Runtime;

/// Most commands, settings jobs and one-off reads waiting for the next call each. A request
/// past the bound is refused at once with `request queue full` and changes nothing, as for the
/// runtime's other request queues (specification 11, request queue bounds).
const MAX_PENDING: usize = 1_024;

/// An open screen, as the exchange knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScreenId(u64);

/// The paths the open screens show, each once, with where it is; kept until a screen changes, so
/// a call does not merge them again.
#[derive(Debug, Default, Clone)]
struct Union {
    paths: Vec<MemberPath>,
    index: FxHashMap<MemberPath, usize>,
}

impl Union {
    fn of<'a>(paths: impl Iterator<Item = &'a MemberPath>) -> Self {
        let mut union = Self::default();
        union.extend(paths);
        union
    }

    fn extend<'a>(&mut self, paths: impl Iterator<Item = &'a MemberPath>) {
        for path in paths {
            if !self.index.contains_key(path) {
                self.index.insert(path.clone(), self.paths.len());
                self.paths.push(path.clone());
            }
        }
    }
}

/// The values copied at the last call, for the paths the screens showed then.
#[derive(Debug)]
struct HmiCopy {
    union: Arc<Union>,
    read: HmiRead,
}

impl HmiCopy {
    /// The copy's values for these paths; `None` when it lacks one, a path no call copied yet.
    fn answer(&self, paths: &[MemberPath]) -> Option<HmiRead> {
        let mut read = HmiRead {
            cycle: self.read.cycle,
            values: Vec::with_capacity(paths.len()),
            present: Vec::with_capacity(paths.len()),
            instance_types: Vec::with_capacity(paths.len()),
        };
        for path in paths {
            let index = *self.union.index.get(path)?;
            read.values.push(self.read.values[index].clone());
            read.present.push(self.read.present[index]);
            read.instance_types
                .push(self.read.instance_types[index].clone());
        }
        Some(read)
    }
}

/// A command waiting for the next call.
struct PendingWrite {
    id: u64,
    path: MemberPath,
    action: WriteAction,
    pulse: Option<Duration>,
    /// Told what happens to the pulse's end once the pulse has started.
    report: Option<PulseEndReport>,
    respond_to: Sender<Result<Written, String>>,
}

#[derive(Default)]
struct ExchangeState {
    screens: FxHashMap<u64, Vec<MemberPath>>,
    /// The screens' paths merged; `None` once a screen opened or closed.
    union: Option<Arc<Union>>,
    next_screen: u64,
    copy: Option<Arc<HmiCopy>>,
    writes: Vec<PendingWrite>,
    /// Settings jobs waiting for the next call (§21.4 item 4).
    settings: Vec<PendingSettings>,
    /// The ends of the pulses that have started (§21.5 item 1).
    pulse_ends: PulseEnds,
    alarms: RuntimeAlarms,
    /// Records the operators' actions added, for the OpenOT ring at the next call.
    alarm_records: Vec<AlarmEvent>,
    calls: u64,
    next_id: u64,
    /// One-off reads waiting for the next call, and the answers it gave them.
    waiting: Vec<(u64, Vec<MemberPath>)>,
    answered: FxHashMap<u64, HmiRead>,
    last_call: Option<Instant>,
    /// The time between the last two calls.
    period: Option<Duration>,
    /// The values the active bundle's trends show, read at each call into the runtime's own
    /// history (specification 39, §5.9 item 1), as written and as paths.
    trended: Arc<Vec<String>>,
    trended_paths: Arc<Vec<MemberPath>>,
    trends: super::trend_history::TrendHistory,
}

/// The exchange between the screens and the running program, shared by the runtime and the web
/// server.
#[derive(Default)]
pub struct HmiExchange {
    pub(crate) methods: super::method_exchange::MethodExchange,
    /// Authenticated owner sessions and host-installed remote command transport.
    pub remote_operations: super::remote_operations::RemoteOperations,
    /// Network-owned samples; copied only when the PLC calls Hmi.Display.
    pub remote_values: super::remote_values::RemoteValues,
    state: Mutex<ExchangeState>,
    /// Wakes one-off reads once a call has answered them.
    served: Condvar,
}

impl std::fmt::Debug for HmiExchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state();
        f.debug_struct("HmiExchange")
            .field("screens", &state.screens.len())
            .field("calls", &state.calls)
            .finish()
    }
}

impl HmiExchange {
    /// Requested remote paths, for the non-realtime mesh worker; no network work runs here.
    pub fn remote_paths(&self) -> Vec<MemberPath> {
        let state = self.state();
        let mut paths = Vec::new();
        for path in state
            .screens
            .values()
            .flatten()
            .chain(state.trended_paths.iter())
            .chain(state.waiting.iter().flat_map(|(_, paths)| paths))
        {
            if super::remote_values::remote_path(path).is_some()
                && !paths.contains(path)
                && paths.len() < 500
            {
                paths.push(path.clone());
            }
        }
        paths
    }

    /// An exchange with no screen open and no call yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The values the active bundle's trends show, to keep in the runtime's own history from the
    /// next call on (specification 39, §5.9 item 1); a path that does not parse is left out.
    pub fn keep_trends(&self, paths: &[String]) {
        let parsed: Vec<(String, MemberPath)> = paths
            .iter()
            .filter_map(|path| {
                MemberPath::parse(path)
                    .ok()
                    .map(|parsed| (path.clone(), parsed))
            })
            .collect();
        let names: Vec<String> = parsed.iter().map(|(name, _)| name.clone()).collect();
        let mut state = self.state();
        state.trends.keep(&names);
        state.trended_paths = Arc::new(parsed.into_iter().map(|(_, path)| path).collect());
        state.trended = Arc::new(names);
    }

    /// A trended value's samples from `since_ms` on, oldest first; none for a value no trend
    /// shows.
    pub fn trend_samples(&self, path: &str, since_ms: u64) -> Option<Vec<(u64, f64)>> {
        let state = self.state();
        state
            .trends
            .keeps(path)
            .then(|| state.trends.samples(path, since_ms))
    }

    fn state(&self) -> MutexGuard<'_, ExchangeState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A screen opens, showing these paths.
    pub fn open_screen(&self, paths: Vec<MemberPath>) -> ScreenId {
        let mut state = self.state();
        state.next_screen += 1;
        let id = state.next_screen;
        state.screens.insert(id, paths);
        state.union = None;
        ScreenId(id)
    }

    /// A screen closed.
    pub fn close_screen(&self, screen: ScreenId) {
        let mut state = self.state();
        state.screens.remove(&screen.0);
        state.union = None;
    }

    /// The last call's values for these paths; nothing until a call has copied them all, so a
    /// screen that has just opened never shows empty values.
    #[must_use]
    pub fn read(&self, paths: &[MemberPath]) -> Option<HmiRead> {
        let copy = self.state().copy.clone()?;
        copy.answer(paths)
    }

    /// Queues a command; its outcome arrives at the next call.
    pub fn queue_write(
        &self,
        path: MemberPath,
        action: WriteAction,
    ) -> Receiver<Result<Written, String>> {
        self.queue_write_with_id(path, action, None, None).1
    }

    /// Queues a command, with the number that withdraws it.
    fn queue_write_with_id(
        &self,
        path: MemberPath,
        action: WriteAction,
        pulse: Option<Duration>,
        report: Option<PulseEndReport>,
    ) -> (u64, Receiver<Result<Written, String>>) {
        let (respond_to, outcome) = mpsc::channel();
        let mut state = self.state();
        state.next_id += 1;
        let id = state.next_id;
        if state.writes.len() >= MAX_PENDING {
            let _ = respond_to.send(Err(RequestQueueFull.to_string()));
            return (id, outcome);
        }
        state.writes.push(PendingWrite {
            id,
            path,
            action,
            pulse,
            report,
            respond_to,
        });
        (id, outcome)
    }

    /// Queues a command and waits up to `timeout` for the call that applies it. A command the
    /// program has not taken by then is withdrawn, so it is never applied later; `None` then.
    pub fn write(
        &self,
        path: MemberPath,
        action: WriteAction,
        timeout: Duration,
    ) -> Option<Result<Written, String>> {
        self.write_with_pulse(path, action, timeout, None)
    }

    /// A pulse is ended by the owning Hmi.Display task even when the network disappears.
    pub fn write_with_pulse(
        &self,
        path: MemberPath,
        action: WriteAction,
        timeout: Duration,
        pulse: Option<Duration>,
    ) -> Option<Result<Written, String>> {
        self.write_reporting(path, action, timeout, pulse, None)
    }

    /// Like [`Self::write_with_pulse`] for a pulse whose end is told to `report` (§21.5 item 1):
    /// the call that applies the pulse schedules its end, and the first call after `pulse` writes
    /// it, tried again at every later call until it is written. Nothing of the end depends on
    /// the caller, its request or its session.
    pub(crate) fn write_pulse(
        &self,
        path: MemberPath,
        action: WriteAction,
        timeout: Duration,
        pulse: Duration,
        report: PulseEndReport,
    ) -> Option<Result<Written, String>> {
        self.write_reporting(path, action, timeout, Some(pulse), Some(report))
    }

    fn write_reporting(
        &self,
        path: MemberPath,
        action: WriteAction,
        timeout: Duration,
        pulse: Option<Duration>,
        report: Option<PulseEndReport>,
    ) -> Option<Result<Written, String>> {
        let (id, outcome) = self.queue_write_with_id(path, action, pulse, report);
        match outcome.recv_timeout(timeout) {
            Ok(result) => Some(result),
            Err(_) => {
                let mut state = self.state();
                let before = state.writes.len();
                state.writes.retain(|write| write.id != id);
                let withdrawn = state.writes.len() != before;
                drop(state);
                // Taken in the meantime: the call is applying it, so wait for its outcome.
                if withdrawn {
                    None
                } else {
                    outcome.recv_timeout(timeout).ok()
                }
            }
        }
    }

    /// Runs a settings job at the next call and waits up to `timeout` for it (§21.4 item 4): its
    /// changes are written together or not at all, then its members are read. A job the program
    /// has not taken by then is withdrawn, so it never runs later; `None` then.
    pub(crate) fn settings(&self, job: SettingsJob, timeout: Duration) -> Option<SettingsOutcome> {
        let (respond_to, outcome) = mpsc::channel();
        let id = {
            let mut state = self.state();
            if state.settings.len() >= MAX_PENDING {
                return Some(SettingsOutcome::Refused(RequestQueueFull.to_string()));
            }
            state.next_id += 1;
            let id = state.next_id;
            state.settings.push(PendingSettings {
                id,
                job,
                respond_to,
            });
            id
        };
        match outcome.recv_timeout(timeout) {
            Ok(result) => Some(result),
            Err(_) => {
                let mut state = self.state();
                let before = state.settings.len();
                state.settings.retain(|pending| pending.id != id);
                let withdrawn = state.settings.len() != before;
                drop(state);
                // Taken in the meantime: the call is running it, so wait for its outcome.
                if withdrawn {
                    None
                } else {
                    outcome.recv_timeout(timeout).ok()
                }
            }
        }
    }

    /// These paths' values at the next call, waiting up to `timeout` for it; `None` when no call
    /// came. For one-off reads: a faceplate's instance type, a command's readback, Live mode.
    pub fn read_next(&self, paths: &[MemberPath], timeout: Duration) -> Option<HmiRead> {
        let deadline = Instant::now() + timeout;
        let mut state = self.state();
        if state.waiting.len() >= MAX_PENDING {
            return None;
        }
        state.next_id += 1;
        let id = state.next_id;
        state.waiting.push((id, paths.to_vec()));
        loop {
            if let Some(read) = state.answered.remove(&id) {
                return Some(read);
            }
            let now = Instant::now();
            if now >= deadline {
                state.waiting.retain(|(waiting, _)| *waiting != id);
                return None;
            }
            state = self
                .served
                .wait_timeout(state, deadline - now)
                .map(|(guard, _)| guard)
                .unwrap_or_else(|poisoned| poisoned.into_inner().0);
        }
    }

    /// Waits up to `timeout` for a call after the `after`th, and returns how many calls there have
    /// been: a stream sends the values as soon as a call has copied them.
    pub fn wait_for_call(&self, after: u64, timeout: Duration) -> u64 {
        let deadline = Instant::now() + timeout;
        let mut state = self.state();
        while state.calls <= after {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            state = self
                .served
                .wait_timeout(state, deadline - now)
                .map(|(guard, _)| guard)
                .unwrap_or_else(|poisoned| poisoned.into_inner().0);
        }
        state.calls
    }

    /// How long between the program's last two calls; nothing before the second call.
    #[must_use]
    pub fn period(&self) -> Option<Duration> {
        self.state().period
    }

    /// How often the runtime has served the exchange.
    #[must_use]
    pub fn calls(&self) -> u64 {
        self.state().calls
    }

    /// The alarm list when it changed since `generation`, and the records after `after`.
    #[must_use]
    pub fn alarm_snapshot(&self, generation: Option<u64>, after: u64) -> AlarmSnapshot {
        self.state().alarms.snapshot(generation, after)
    }

    /// The alarm's path as the program declares it.
    #[must_use]
    pub fn alarm_path(&self, path: &str) -> Option<String> {
        self.state().alarms.path(path)
    }

    /// The alarm's state as the program's last `Hmi.Display` call read it.
    #[must_use]
    pub fn alarm_status(&self, path: &str) -> Option<crate::hmi::alarms::AlarmStatus> {
        self.state().alarms.status(path).cloned()
    }

    /// An operator's applied action among the records.
    pub fn alarm_operator(&self, action: OperatorAction) {
        let mut state = self.state();
        let records = state.alarms.operator_action(action);
        state.alarm_records.extend(records);
    }

    /// A command's final outcome among the records.
    pub fn alarm_command(
        &self,
        path: &str,
        message: &str,
        user: Option<&str>,
        outcome: &str,
        detail: Option<String>,
    ) {
        self.state()
            .alarms
            .command(path, message, user, outcome, detail);
    }

    /// Serves the screens from the running program, after a task in which it called
    /// `Hmi.Display`: applies the queued commands, copies the values the open screens show, reads
    /// the alarms, and hands the records to the OpenOT history. The program is read outside the
    /// lock, which the web server only takes to copy or queue.
    pub(crate) fn serve(&self, runtime: &mut Runtime, silent: &[SilentInput]) {
        let (writes, settings, union, waiting, due) = {
            let mut state = self.state();
            let due = state.pulse_ends.take_due(Instant::now());
            let writes = std::mem::take(&mut state.writes);
            let settings = std::mem::take(&mut state.settings);
            let waiting = std::mem::take(&mut state.waiting);
            let screens = match state.union.clone() {
                Some(union) => union,
                None => {
                    let union = Arc::new(Union::of(state.screens.values().flatten()));
                    state.union = Some(Arc::clone(&union));
                    union
                }
            };
            // A one-off read's paths join this call's copy only.
            let union = if waiting.is_empty() {
                screens
            } else {
                let mut union = Union::clone(&screens);
                union.extend(waiting.iter().flat_map(|(_, asked)| asked));
                Arc::new(union)
            };
            (writes, settings, union, waiting, due)
        };
        // A pulse's end is written at the first call after its duration and, until it is written,
        // tried again at every call (§12.6, §21.5 item 1).
        let unwritten: Vec<_> = due
            .into_iter()
            .filter_map(|end| {
                end.end(|path| {
                    write_point(runtime, path, &WriteAction::Set(serde_json::json!(false)))
                })
            })
            .collect();
        self.state().pulse_ends.retry(unwritten);
        let mut replaced = Vec::new();
        for write in writes {
            let outcome = write_point(runtime, &write.path, &write.action);
            if outcome.is_ok() {
                let mut state = self.state();
                // A later command to the point replaces an end still waiting there; a pulse's
                // own end is scheduled as it starts.
                replaced.extend(state.pulse_ends.replace(&write.path));
                if let Some(duration) = write.pulse {
                    state.pulse_ends.schedule(
                        write.path.clone(),
                        Instant::now(),
                        duration,
                        write.report,
                    );
                }
            }
            let _ = write.respond_to.send(outcome);
        }
        for end in replaced {
            end.replaced();
        }
        // Settings take effect together at this call, before the screens' copy (§21.4 item 4).
        for pending in settings {
            let outcome = settings_jobs::run(runtime, &pending.job);
            let _ = pending.respond_to.send(outcome);
        }
        let mut read = display_values::read(runtime, &union.paths);
        self.remote_values.overlay(&union.paths, &mut read);
        let (trended, trended_paths) = {
            let state = self.state();
            (Arc::clone(&state.trended), Arc::clone(&state.trended_paths))
        };
        let trend_read = (!trended_paths.is_empty()).then(|| {
            let mut read = display_values::read(runtime, &trended_paths);
            self.remote_values.overlay(&trended_paths, &mut read);
            read
        });
        // A value at the inputs of a link without data is stale with the link's age.
        for (path, value) in union.paths.iter().zip(read.values.iter_mut()) {
            if let Some(wire) = value
                && super::remote_values::remote_path(path).is_none()
            {
                wire.stale_ms = silent_age(path, silent);
            }
        }
        let statuses = read_alarms(runtime);
        let records = {
            let mut state = self.state();
            let copy = HmiCopy { union, read };
            for (id, asked) in waiting {
                if let Some(answer) = copy.answer(&asked) {
                    state.answered.insert(id, answer);
                }
            }
            state.copy = Some(Arc::new(copy));
            if let Some(trend_read) = trend_read {
                let now_ms = super::trend_history::wall_ms();
                for (name, value) in trended.iter().zip(trend_read.values.iter()) {
                    if let Some(number) = value.as_ref().and_then(|wire| number_of(&wire.value)) {
                        state.trends.record(name, now_ms, number);
                    }
                }
            }
            state.calls += 1;
            let now = Instant::now();
            state.period = state
                .last_call
                .map(|last| now.saturating_duration_since(last));
            state.last_call = Some(now);
            let mut records = std::mem::take(&mut state.alarm_records);
            records.extend(state.alarms.apply(statuses));
            records
        };
        self.served.notify_all();
        if !records.is_empty() {
            runtime.publish_alarm_records(&records);
        }
    }
}

/// A value a trend can draw: a number, or a boolean as 0 or 1.
fn number_of(shown: &display_values::Shown) -> Option<f64> {
    match shown {
        display_values::Shown::Single(serde_json::Value::Number(number)) => number.as_f64(),
        display_values::Shown::Single(serde_json::Value::Bool(on)) => {
            Some(if *on { 1.0 } else { 0.0 })
        }
        _ => None,
    }
}

#[cfg(test)]
mod pulse_end_tests;

#[cfg(test)]
mod queue_bound_tests {
    use super::*;

    /// Commands past the bound are refused at once with `request queue full`, as the runtime's
    /// other request queues are; the queued ones keep waiting. Catches an unbounded queue that a
    /// flood of operator requests could grow without limit.
    #[test]
    fn a_command_past_the_queue_bound_is_refused_at_once() {
        let exchange = HmiExchange::new();
        let path = MemberPath::parse("Main.Start").unwrap();
        let queued: Vec<_> = (0..MAX_PENDING)
            .map(|_| exchange.queue_write(path.clone(), WriteAction::Toggle))
            .collect();
        let refused = exchange.queue_write(path, WriteAction::Toggle);
        assert_eq!(
            refused.try_recv().ok(),
            Some(Err("request queue full".to_string()))
        );
        assert!(queued.iter().all(|outcome| outcome.try_recv().is_err()));
    }
}

#[cfg(test)]
mod remote_pulse_tests {
    use super::*;
    #[test]
    fn pulse_end_is_owned_by_the_exchange_after_the_requester_disconnects() {
        let mut plant = crate::harness::TestHarness::from_source(
            "PROGRAM Main VAR Pulse : BOOL; END_VAR END_PROGRAM",
        )
        .unwrap();
        plant.cycle();
        let exchange = HmiExchange::new();
        let path = MemberPath::parse("Main.Pulse").unwrap();
        let (_, reply) = exchange.queue_write_with_id(
            path.clone(),
            WriteAction::Set(serde_json::json!(true)),
            Some(Duration::ZERO),
            None,
        );
        drop(reply);
        exchange.serve(plant.runtime_mut(), &[]);
        assert_eq!(
            crate::hmi::member_paths::resolve_member(&path, plant.runtime().storage()),
            Some(&crate::value::Value::Bool(true))
        );
        exchange.serve(plant.runtime_mut(), &[]);
        assert_eq!(
            crate::hmi::member_paths::resolve_member(&path, plant.runtime().storage()),
            Some(&crate::value::Value::Bool(false))
        );
    }
}
