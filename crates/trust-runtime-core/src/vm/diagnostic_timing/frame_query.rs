//! Bounded phase-query counts for post-run diagnosis.

use crate::bytecode::{InitializationPhase, InitializationTrigger};
use core::cell::{Cell, RefCell};
use core::marker::PhantomData;
use std::rc::Rc;

/// Maximum distinct owner/phase/trigger rows retained.
pub const FRAME_QUERY_RECORD_CAPACITY: usize = 128;

/// Aggregates for one owner, phase, and trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameQueryRecord {
    /// POU that selected the slice.
    pub owner_pou_id: u32,
    /// Queried initialization phase.
    pub phase: InitializationPhase,
    /// Lifecycle trigger used for the query.
    pub trigger: InitializationTrigger,
    /// Number of queries.
    pub query_count: u64,
    /// Actions in selected slices.
    pub selected_actions: u64,
    /// Actions skipped because caller data supplied the value.
    pub supplied_skips: u64,
    /// Actions dispatched to the declaration runner.
    pub action_attempts: u64,
    /// Queries completed successfully.
    pub completed_queries: u64,
    /// Queries ended before successful completion.
    pub errors: u64,
}

impl FrameQueryRecord {
    const EMPTY: Self = Self {
        owner_pou_id: 0,
        phase: InitializationPhase::Resource,
        trigger: InitializationTrigger::Ordinary,
        query_count: 0,
        selected_actions: 0,
        supplied_skips: 0,
        action_attempts: 0,
        completed_queries: 0,
        errors: 0,
    };

    fn matches(self, key: Key) -> bool {
        self.query_count != 0
            && self.owner_pou_id == key.owner
            && self.phase == key.phase
            && self.trigger == key.trigger
    }
}

/// Fixed-size query totals for one sampled recording window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameQuerySnapshot {
    /// Populated rows occupy `records[..len]`.
    pub records: [FrameQueryRecord; FRAME_QUERY_RECORD_CAPACITY],
    /// Number of populated rows.
    pub len: usize,
    /// Events lost to table/counter overflow.
    pub overflow_count: u64,
    /// Events lost to recorder borrowing or TLS failure.
    pub recorder_busy: u64,
    /// Queries open when recording finished.
    pub active_queries: u64,
    /// Open queries invalidated by finish/reset.
    pub unbalanced_queries: u64,
}

impl FrameQuerySnapshot {
    const fn empty() -> Self {
        Self {
            records: [FrameQueryRecord::EMPTY; FRAME_QUERY_RECORD_CAPACITY],
            len: 0,
            overflow_count: 0,
            recorder_busy: 0,
            active_queries: 0,
            unbalanced_queries: 0,
        }
    }

    /// Whether the entire recording window was accounted for.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.overflow_count == 0
            && self.recorder_busy == 0
            && self.active_queries == 0
            && self.unbalanced_queries == 0
    }
}

/// Clear prior rows and enable frame-query recording on this thread.
pub fn start_frame_query_log() {
    let mut reset = false;
    let result = LOG.try_with(|log| {
        if let Ok(mut log) = log.try_borrow_mut() {
            log.reset();
            reset = true;
        }
    });
    if result.is_err() || !reset {
        note_busy();
        let _ = ENABLED.try_with(|enabled| enabled.set(false));
        return;
    }
    let _ = FAILURES.try_with(|failures| failures.set(0));
    let _ = ENABLED.try_with(|enabled| enabled.set(true));
}

/// Stop recording and capture the fixed-size aggregate.
#[must_use]
pub fn finish_frame_query_log() -> FrameQuerySnapshot {
    let _ = ENABLED.try_with(|enabled| enabled.set(false));
    let mut snapshot = FrameQuerySnapshot::empty();
    if LOG
        .try_with(|log| match log.try_borrow_mut() {
            Ok(mut log) => snapshot = log.finish(),
            Err(_) => snapshot.recorder_busy = 1,
        })
        .is_err()
    {
        snapshot.recorder_busy = snapshot.recorder_busy.saturating_add(1);
    }
    if let Ok(failures) = FAILURES.try_with(Cell::get) {
        snapshot.recorder_busy = snapshot.recorder_busy.saturating_add(failures);
    }
    snapshot
}

/// One in-progress phase query. It stays on its recording thread and an
/// uncompleted guard records an error when dropped.
pub struct FrameQuery {
    key: Key,
    generation: u64,
    active: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl FrameQuery {
    /// Count a caller-supplied skip.
    pub fn supplied_skip(&mut self) {
        self.event(Event::SuppliedSkip);
    }

    /// Count an attempted action dispatch.
    pub fn action_attempt(&mut self) {
        self.event(Event::ActionAttempt);
    }

    /// Mark the action slice successfully processed.
    pub fn complete(mut self) {
        self.event(Event::Complete);
    }

    fn event(&mut self, event: Event) {
        if !self.active {
            return;
        }
        let result = LOG.try_with(|log| match log.try_borrow_mut() {
            Ok(mut log) => log.event(self.key, self.generation, event),
            Err(_) => {
                note_busy();
                false
            }
        });
        if !matches!(result, Ok(true)) {
            if result.is_err() {
                note_busy();
            }
            self.active = false;
        } else if matches!(event, Event::Complete | Event::Error) {
            self.active = false;
        }
    }
}

impl Drop for FrameQuery {
    fn drop(&mut self) {
        self.event(Event::Error);
    }
}

/// Begin counting one selected action slice. Disabled calls return an inert guard.
pub fn begin_frame_query(
    owner_pou_id: u32,
    phase: InitializationPhase,
    trigger: InitializationTrigger,
    selected_actions: usize,
) -> FrameQuery {
    let key = Key {
        owner: owner_pou_id,
        phase,
        trigger,
    };
    let mut query = FrameQuery {
        key,
        generation: 0,
        active: false,
        _not_send_or_sync: PhantomData,
    };
    match ENABLED.try_with(Cell::get) {
        Ok(true) => {}
        Ok(false) => return query,
        Err(_) => {
            note_busy();
            return query;
        }
    }
    let result = LOG.try_with(|log| match log.try_borrow_mut() {
        Ok(mut log) => {
            if let Some(generation) = log.begin(key, selected_actions) {
                query.generation = generation;
                query.active = true;
            }
        }
        Err(_) => note_busy(),
    });
    if result.is_err() {
        note_busy();
    }
    query
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    owner: u32,
    phase: InitializationPhase,
    trigger: InitializationTrigger,
}

#[derive(Clone, Copy)]
enum Event {
    SuppliedSkip,
    ActionAttempt,
    Complete,
    Error,
}

struct Recorder {
    records: [FrameQueryRecord; FRAME_QUERY_RECORD_CAPACITY],
    len: usize,
    generation: u64,
    active: u64,
    unbalanced: u64,
    overflow: u64,
}

impl Recorder {
    const fn new() -> Self {
        Self {
            records: [FrameQueryRecord::EMPTY; FRAME_QUERY_RECORD_CAPACITY],
            len: 0,
            generation: 0,
            active: 0,
            unbalanced: 0,
            overflow: 0,
        }
    }

    fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.records.fill(FrameQueryRecord::EMPTY);
        self.len = 0;
        self.active = 0;
        self.unbalanced = 0;
        self.overflow = 0;
    }

    fn begin(&mut self, key: Key, selected: usize) -> Option<u64> {
        let Ok(selected) = u64::try_from(selected) else {
            self.overflow = self.overflow.saturating_add(1);
            return None;
        };
        let index = self.index(key)?;
        let record = &mut self.records[index];
        let (Some(queries), Some(actions), Some(active)) = (
            record.query_count.checked_add(1),
            record.selected_actions.checked_add(selected),
            self.active.checked_add(1),
        ) else {
            self.overflow = self.overflow.saturating_add(1);
            return None;
        };
        record.query_count = queries;
        record.selected_actions = actions;
        self.active = active;
        Some(self.generation)
    }

    fn event(&mut self, key: Key, generation: u64, event: Event) -> bool {
        if generation != self.generation {
            return false;
        }
        let Some(index) = self.records[..self.len]
            .iter()
            .position(|row| row.matches(key))
        else {
            self.overflow = self.overflow.saturating_add(1);
            return false;
        };
        let record = &mut self.records[index];
        let counter = match event {
            Event::SuppliedSkip => &mut record.supplied_skips,
            Event::ActionAttempt => &mut record.action_attempts,
            Event::Complete => &mut record.completed_queries,
            Event::Error => &mut record.errors,
        };
        let Some(value) = counter.checked_add(1) else {
            self.overflow = self.overflow.saturating_add(1);
            return false;
        };
        let active = if matches!(event, Event::Complete | Event::Error) {
            let Some(active) = self.active.checked_sub(1) else {
                self.overflow = self.overflow.saturating_add(1);
                return false;
            };
            Some(active)
        } else {
            None
        };
        *counter = value;
        if let Some(active) = active {
            self.active = active;
        }
        true
    }

    fn index(&mut self, key: Key) -> Option<usize> {
        if let Some(index) = self.records[..self.len]
            .iter()
            .position(|row| row.matches(key))
        {
            return Some(index);
        }
        if self.len == FRAME_QUERY_RECORD_CAPACITY {
            self.overflow = self.overflow.saturating_add(1);
            return None;
        }
        let index = self.len;
        self.records[index] = FrameQueryRecord {
            owner_pou_id: key.owner,
            phase: key.phase,
            trigger: key.trigger,
            ..FrameQueryRecord::EMPTY
        };
        self.len += 1;
        Some(index)
    }

    fn finish(&mut self) -> FrameQuerySnapshot {
        let active = self.active;
        self.unbalanced = self.unbalanced.saturating_add(active);
        self.generation = self.generation.wrapping_add(1);
        self.active = 0;
        FrameQuerySnapshot {
            records: self.records,
            len: self.len,
            overflow_count: self.overflow,
            recorder_busy: 0,
            active_queries: active,
            unbalanced_queries: self.unbalanced,
        }
    }
}

fn note_busy() {
    let _ = FAILURES.try_with(|failures| failures.set(failures.get().saturating_add(1)));
}

thread_local! {
    static LOG: RefCell<Recorder> = const { RefCell::new(Recorder::new()) };
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static FAILURES: Cell<u64> = const { Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query() -> FrameQuery {
        begin_frame_query(
            7,
            InitializationPhase::Return,
            InitializationTrigger::Ordinary,
            3,
        )
    }

    #[test]
    fn aggregation_disable_and_reset() {
        query().complete();
        assert_eq!(finish_frame_query_log().len, 0);
        start_frame_query_log();
        let mut query = query();
        query.supplied_skip();
        query.action_attempt();
        query.complete();
        let snapshot = finish_frame_query_log();
        assert!(snapshot.is_valid());
        let row = snapshot.records[0];
        assert_eq!((row.query_count, row.selected_actions), (1, 3));
        assert_eq!((row.supplied_skips, row.action_attempts), (1, 1));
        assert_eq!((row.completed_queries, row.errors), (1, 0));
        start_frame_query_log();
        assert_eq!(finish_frame_query_log().len, 0);
    }

    #[test]
    fn overflow_and_open_guard_invalidate_snapshot() {
        start_frame_query_log();
        let open = query();
        let snapshot = finish_frame_query_log();
        assert_eq!(
            (snapshot.active_queries, snapshot.unbalanced_queries),
            (1, 1)
        );
        assert!(!snapshot.is_valid());
        assert!(!finish_frame_query_log().is_valid());
        start_frame_query_log();
        let current = query();
        drop(open);
        current.complete();
        let snapshot = finish_frame_query_log();
        assert!(snapshot.is_valid());
        assert_eq!(snapshot.records[0].query_count, 1);
        assert_eq!(snapshot.records[0].completed_queries, 1);
        assert_eq!(snapshot.records[0].errors, 0);

        start_frame_query_log();
        for owner in 0..=FRAME_QUERY_RECORD_CAPACITY as u32 {
            begin_frame_query(
                owner,
                InitializationPhase::Frame,
                InitializationTrigger::Ordinary,
                0,
            )
            .complete();
        }
        let snapshot = finish_frame_query_log();
        assert_eq!(snapshot.len, FRAME_QUERY_RECORD_CAPACITY);
        assert_eq!(snapshot.overflow_count, 1);
        assert!(!snapshot.is_valid());
    }
}
