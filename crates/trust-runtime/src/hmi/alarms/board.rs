//! The alarm board (specification 40, §5.2–5.3): the latest state of every alarm, the transitions
//! between reads as events with their time, and the alarm list in display order.

use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;

use super::offers::{
    list_of, offered_actions, sound, AlarmList, AlarmSound, COMMENT_MAX_CHARS, SHELVE_FOR_SECONDS,
};
use super::status::{AlarmStatus, Priority};
use crate::security::AccessRole;

/// What happened to an alarm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventKind {
    /// It became active.
    Raised,
    /// Its condition cleared.
    Cleared,
    /// It was acknowledged.
    Acknowledged,
    /// It was shelved.
    Shelved,
    /// It returned from shelving.
    Unshelved,
    /// The program suppressed it.
    Suppressed,
    /// The program stopped suppressing it.
    Unsuppressed,
    /// It was taken out of service.
    OutOfService,
    /// It was put back in service.
    InService,
    /// A latched alarm was reset.
    Reset,
    /// An operator commented on it.
    Comment,
    /// A command's final outcome (specification 39, §12.5).
    Command,
}

/// One record.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlarmEvent {
    /// A number that grows with each event.
    pub seq: u64,
    /// When, in ms since the Unix epoch.
    pub time_ms: u64,
    /// The alarm's instance path, or a command's target point.
    pub path: String,
    /// What happened.
    pub kind: EventKind,
    /// The alarm's message, or what a command asked.
    pub message: String,
    /// The alarm's priority; a command has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    /// The user, for an operator's action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// More detail: a comment's text, a shelving's duration, why a command failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// A command's final outcome.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    /// For a raise and its clearing: the alarm's raise count, new for each activation (the
    /// OpenOT correlation, specification 40, §5.2).
    #[serde(skip)]
    pub correlation: Option<u32>,
    /// For a shelving: its seconds (OpenOT `ShelveSecs`).
    #[serde(skip)]
    pub seconds: Option<u32>,
}

/// An alarm in the list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlarmRow {
    /// Its state.
    #[serde(flatten)]
    pub status: AlarmStatus,
    /// When it last became active, in ms since the Unix epoch.
    pub since_ms: u64,
    /// A limit alarm's value against its limit when it was raised.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raised: Option<super::status::Raised>,
    /// How it sounds on the display (specification 39, §5.9 item 2).
    pub sound: AlarmSound,
    /// The actions its state allows the session the list is sent to (§5.7 item 4), filled by
    /// [`AlarmSummary::for_role`].
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<&'static str>,
}

/// The alarm list as the display shows it (specification 39, §13.6).
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct AlarmSummary {
    /// Annunciating alarms: unacknowledged first, then by priority, then newest first.
    pub active: Vec<AlarmRow>,
    /// Shelved alarms.
    pub shelved: Vec<AlarmRow>,
    /// Alarms suppressed by design.
    pub suppressed: Vec<AlarmRow>,
    /// Alarms out of service.
    pub out_of_service: Vec<AlarmRow>,
    /// How many wait for acknowledgement.
    pub unacknowledged: usize,
    /// How many annunciate.
    pub annunciating: usize,
    /// The newest annunciating alarm, for the top bar.
    pub newest: Option<AlarmRow>,
    /// The shelving durations an operator chooses from, in seconds.
    pub shelve_for: &'static [u32],
    /// The longest comment, in characters.
    pub comment_max_chars: usize,
}

impl AlarmSummary {
    /// The list as a session with `role` receives it: each alarm with the actions its state allows
    /// that role (specification 39, §5.7 item 4).
    #[must_use]
    pub fn for_role(mut self, role: Option<AccessRole>) -> Self {
        let rows = self
            .active
            .iter_mut()
            .chain(&mut self.shelved)
            .chain(&mut self.suppressed)
            .chain(&mut self.out_of_service)
            .chain(self.newest.as_mut());
        for row in rows {
            row.actions = offered_actions(&row.status, role);
        }
        self
    }
}

/// The most events kept.
const MAX_EVENTS: usize = 1_000;
/// How long an operator's action waits for the read that shows it.
const PENDING_FOR_MS: u64 = 10_000;
/// The most operator actions waiting at once.
const MAX_PENDING: usize = 64;

/// An operator's action, waiting for the read that shows its transition.
#[derive(Debug, Clone)]
struct Pending {
    path: String,
    kind: EventKind,
    user: String,
    detail: Option<String>,
    seconds: Option<u32>,
    time_ms: u64,
}

#[derive(Debug, Clone)]
struct Entry {
    status: AlarmStatus,
    since_ms: u64,
    /// The reading of the read that raised it (§5.9 item 2).
    raised: Option<super::status::Raised>,
}

/// The latest state of every alarm and the latest events.
#[derive(Debug, Default)]
pub struct AlarmBoard {
    alarms: BTreeMap<String, Entry>,
    events: VecDeque<AlarmEvent>,
    seq: u64,
    /// Operator actions waiting for the read that shows them, which then names the user.
    pending: Vec<Pending>,
    read_once: bool,
    generation: u64,
}

impl AlarmBoard {
    /// Takes a new read of every alarm at `now_ms` and records the transitions since the last
    /// one. The first read is the baseline and records nothing.
    pub fn update(&mut self, statuses: Vec<AlarmStatus>, now_ms: u64) {
        self.pending
            .retain(|pending| now_ms.saturating_sub(pending.time_ms) <= PENDING_FOR_MS);
        let mut changed = statuses.len() != self.alarms.len();
        let mut seen = Vec::with_capacity(statuses.len());
        for status in statuses {
            seen.push(status.path.clone());
            let previous = self.alarms.get(&status.path).cloned();
            let (since_ms, raised) = match &previous {
                Some(previous) if self.read_once => {
                    if self.transitions(&previous.status, &status, now_ms) {
                        (now_ms, status.reading.0.clone())
                    } else {
                        (previous.since_ms, previous.raised.clone())
                    }
                }
                // Seen first: an active alarm's reading now is the nearest to its raise.
                _ => (now_ms, status.reading.0.clone().filter(|_| status.active)),
            };
            changed |= previous
                .as_ref()
                .is_none_or(|previous| previous.status != status);
            self.alarms.insert(
                status.path.clone(),
                Entry {
                    status,
                    since_ms,
                    raised,
                },
            );
        }
        self.alarms.retain(|path, _| seen.contains(path));
        self.read_once = true;
        if changed {
            self.generation += 1;
        }
    }

    /// Records what changed for one alarm; whether it became active.
    fn transitions(&mut self, before: &AlarmStatus, after: &AlarmStatus, now_ms: u64) -> bool {
        let raises = if after.raise_count >= before.raise_count {
            after.raise_count - before.raise_count
        } else {
            // The program restarted cold and counts from zero again.
            after.raise_count
        };
        for _ in 0..raises.min(16) {
            self.record(after, EventKind::Raised, now_ms);
        }
        let hidden = after.shelved || after.out_of_service || after.suppressed;
        let pairs = [
            (!before.shelved && after.shelved, EventKind::Shelved),
            (before.shelved && !after.shelved, EventKind::Unshelved),
            (
                !before.suppressed && after.suppressed,
                EventKind::Suppressed,
            ),
            (
                before.suppressed && !after.suppressed,
                EventKind::Unsuppressed,
            ),
            (
                !before.out_of_service && after.out_of_service,
                EventKind::OutOfService,
            ),
            (
                before.out_of_service && !after.out_of_service,
                EventKind::InService,
            ),
        ];
        for (happened, kind) in pairs {
            if happened {
                self.record(after, kind, now_ms);
            }
        }
        if (before.active || raises > 0) && !after.active {
            let kind = if self.is_pending(&after.path, EventKind::Reset) {
                EventKind::Reset
            } else {
                EventKind::Cleared
            };
            self.record(after, kind, now_ms);
        }
        // Hiding an alarm also ends its wait for acknowledgement; that is no acknowledgement.
        let acknowledged =
            (before.unacknowledged || raises > 0) && !after.unacknowledged && !hidden;
        if acknowledged && (before.unacknowledged || after.state == "ActiveAcknowledged") {
            self.record(after, EventKind::Acknowledged, now_ms);
        }
        raises > 0
    }

    fn is_pending(&self, path: &str, kind: EventKind) -> bool {
        self.pending
            .iter()
            .any(|pending| pending.path == path && pending.kind == kind)
    }

    /// A transition the read shows, with the user of the operator's action that caused it.
    fn record(&mut self, status: &AlarmStatus, kind: EventKind, now_ms: u64) {
        let (user, detail, seconds) = match self
            .pending
            .iter()
            .position(|pending| pending.path == status.path && pending.kind == kind)
        {
            Some(index) => {
                let pending = self.pending.remove(index);
                (Some(pending.user), pending.detail, pending.seconds)
            }
            None => (None, None, None),
        };
        let correlation = matches!(
            kind,
            EventKind::Raised | EventKind::Cleared | EventKind::Reset
        )
        .then_some(status.raise_count);
        self.push(AlarmEvent {
            seq: 0,
            time_ms: now_ms,
            path: status.path.clone(),
            kind,
            message: status.message.clone(),
            priority: Some(status.priority),
            user,
            detail,
            outcome: None,
            correlation,
            seconds,
        });
    }

    /// Keeps a record with the next number, the oldest going when there are too many.
    fn push(&mut self, mut event: AlarmEvent) {
        self.seq += 1;
        event.seq = self.seq;
        self.events.push_back(event);
        while self.events.len() > MAX_EVENTS {
            self.events.pop_front();
        }
        self.generation += 1;
    }

    /// A command's final outcome (specification 40, §5.1): `path` is the alarm for an alarm
    /// action and the target point otherwise, `message` what it asked, and `detail` why it was
    /// refused or timed out.
    pub fn command(
        &mut self,
        path: &str,
        message: &str,
        user: Option<&str>,
        outcome: &str,
        detail: Option<String>,
        now_ms: u64,
    ) {
        self.push(AlarmEvent {
            seq: 0,
            time_ms: now_ms,
            path: path.to_string(),
            kind: EventKind::Command,
            message: message.to_string(),
            priority: None,
            user: user.map(str::to_string),
            detail,
            outcome: Some(outcome.to_string()),
            correlation: None,
            seconds: None,
        });
    }

    /// An operator's applied action. Its record is made by the read that shows the transition,
    /// which then names the user; an action that changes nothing makes no record. A comment shows
    /// in no read and is recorded at once.
    pub fn operator_action(
        &mut self,
        path: &str,
        kind: EventKind,
        user: &str,
        detail: Option<String>,
        seconds: Option<u32>,
        now_ms: u64,
    ) {
        if kind == EventKind::Comment {
            let (message, priority) = self
                .alarms
                .get(path)
                .map(|entry| (entry.status.message.clone(), Some(entry.status.priority)))
                .unwrap_or_default();
            self.push(AlarmEvent {
                seq: 0,
                time_ms: now_ms,
                path: path.to_string(),
                kind,
                message,
                priority,
                user: Some(user.to_string()),
                detail,
                outcome: None,
                correlation: None,
                seconds: None,
            });
            return;
        }
        if self.pending.len() >= MAX_PENDING {
            self.pending.remove(0);
        }
        self.pending.push(Pending {
            path: path.to_string(),
            kind,
            user: user.to_string(),
            detail,
            seconds,
            time_ms: now_ms,
        });
    }

    /// Withdraws the pending action of `user` of `kind` on `path`, whose write the runtime refused
    /// or did not apply in time, so that no later transition the program makes is recorded as the
    /// user's.
    pub fn withdraw_operator_action(&mut self, path: &str, kind: EventKind, user: &str) {
        if let Some(index) = self.pending.iter().rposition(|pending| {
            pending.path == path && pending.kind == kind && pending.user == user
        }) {
            self.pending.remove(index);
        }
    }

    /// The number of the latest record, 0 before the first.
    pub fn last_seq(&self) -> u64 {
        self.seq
    }

    /// The latest events, oldest first.
    pub fn events(&self) -> &VecDeque<AlarmEvent> {
        &self.events
    }

    /// The events after `seq`, oldest first.
    pub fn events_after(&self, seq: u64) -> Vec<AlarmEvent> {
        self.events
            .iter()
            .filter(|event| event.seq > seq)
            .cloned()
            .collect()
    }

    /// Grows whenever the list or the events change.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether the runtime has this alarm.
    pub fn knows(&self, path: &str) -> bool {
        self.alarms.contains_key(path)
    }

    /// The alarm's path as the program declares it, matched without regard to case as IEC
    /// 61131-3 compares identifiers.
    pub fn path(&self, path: &str) -> Option<String> {
        if self.alarms.contains_key(path) {
            return Some(path.to_string());
        }
        self.alarms
            .keys()
            .find(|known| known.eq_ignore_ascii_case(path))
            .cloned()
    }

    /// The latest state of one alarm.
    pub fn status(&self, path: &str) -> Option<&AlarmStatus> {
        self.alarms.get(path).map(|entry| &entry.status)
    }

    /// The alarm list: annunciating alarms unacknowledged first, then by priority, then newest
    /// first; out-of-service, shelved and suppressed alarms apart (specification 39, §13.6).
    pub fn summary(&self) -> AlarmSummary {
        let mut summary = AlarmSummary {
            shelve_for: &SHELVE_FOR_SECONDS,
            comment_max_chars: COMMENT_MAX_CHARS,
            ..AlarmSummary::default()
        };
        for entry in self.alarms.values() {
            let row = AlarmRow {
                status: entry.status.clone(),
                since_ms: entry.since_ms,
                raised: entry.raised.clone(),
                sound: sound(&entry.status),
                actions: Vec::new(),
            };
            match list_of(&entry.status) {
                Some(AlarmList::OutOfService) => summary.out_of_service.push(row),
                Some(AlarmList::Shelved) => summary.shelved.push(row),
                Some(AlarmList::Suppressed) => summary.suppressed.push(row),
                Some(AlarmList::Active) => summary.active.push(row),
                None => {}
            }
        }
        summary.active.sort_by(|a, b| {
            (b.status.unacknowledged, b.status.priority, b.since_ms)
                .cmp(&(a.status.unacknowledged, a.status.priority, a.since_ms))
                .then_with(|| a.status.path.cmp(&b.status.path))
        });
        summary.unacknowledged = summary
            .active
            .iter()
            .filter(|row| row.status.unacknowledged)
            .count();
        summary.annunciating = summary.active.len();
        summary.newest = summary
            .active
            .iter()
            .max_by(|a, b| (a.since_ms, a.status.priority).cmp(&(b.since_ms, b.status.priority)))
            .cloned();
        summary
    }
}
