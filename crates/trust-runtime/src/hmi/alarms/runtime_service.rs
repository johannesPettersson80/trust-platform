//! The alarm service (specification 40, §5, item 6): at each call of `Hmi.Display` the alarms read
//! from the program go onto the alarm board, so the records and the OpenOT history do not depend
//! on an open display; the board answers the web server's requests for the list and the records.

use std::time::{SystemTime, UNIX_EPOCH};

use super::board::{AlarmBoard, AlarmEvent, AlarmSummary, EventKind};

/// What the web server gets from the runtime (§5.1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AlarmSnapshot {
    /// Grows whenever the list or the records change.
    pub generation: u64,
    /// The number of the latest record.
    pub last_seq: u64,
    /// The list, when it changed since the generation asked about.
    pub summary: Option<AlarmSummary>,
    /// The records after the number asked about, oldest first.
    pub events: Vec<AlarmEvent>,
}

/// An operator's applied action, as the web server reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct OperatorAction {
    /// The alarm's path.
    pub path: String,
    /// The transition the action causes.
    pub kind: EventKind,
    /// Who sent it.
    pub user: String,
    /// A comment's text or a shelving's duration, for the record.
    pub detail: Option<String>,
    /// A shelving's seconds, for the OpenOT record.
    pub seconds: Option<u32>,
}

/// The alarm board.
#[derive(Debug, Default)]
pub struct RuntimeAlarms {
    board: AlarmBoard,
}

impl RuntimeAlarms {
    /// Puts the alarms' states, as read from the program, on the board, and returns the records
    /// that added: the call of `Hmi.Display` reads them (specification 39, §18 Phase 5).
    pub fn apply(&mut self, statuses: Vec<super::status::AlarmStatus>) -> Vec<AlarmEvent> {
        let before = self.board.last_seq();
        self.board.update(statuses, wall_ms());
        self.board.events_after(before)
    }

    /// The list when it changed since `generation`, and the records after `after`.
    #[must_use]
    pub fn snapshot(&self, generation: Option<u64>, after: u64) -> AlarmSnapshot {
        let current = self.board.generation();
        AlarmSnapshot {
            generation: current,
            last_seq: self.board.last_seq(),
            summary: (generation != Some(current)).then(|| self.board.summary()),
            events: self.board.events_after(after),
        }
    }

    /// The alarm's path as the program declares it, matched without regard to case.
    #[must_use]
    pub fn path(&self, path: &str) -> Option<String> {
        self.board.path(path)
    }

    /// The latest state of one alarm, by its path as the program declares it.
    #[must_use]
    pub fn status(&self, path: &str) -> Option<&super::status::AlarmStatus> {
        self.board.status(path)
    }

    /// An operator's applied action; returns the records it adds at once (a comment), for the
    /// OpenOT ring.
    pub fn operator_action(&mut self, action: OperatorAction) -> Vec<AlarmEvent> {
        let before = self.board.last_seq();
        self.board.operator_action(
            &action.path,
            action.kind,
            &action.user,
            action.detail,
            action.seconds,
            wall_ms(),
        );
        self.board.events_after(before)
    }

    /// A command's final outcome among the records.
    pub fn command(
        &mut self,
        path: &str,
        message: &str,
        user: Option<&str>,
        outcome: &str,
        detail: Option<String>,
    ) {
        self.board
            .command(path, message, user, outcome, detail, wall_ms());
    }
}

/// Milliseconds since the Unix epoch.
fn wall_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}
