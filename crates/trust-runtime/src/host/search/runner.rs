//! One search while it runs (specification 11, "Search"): every source on its own thread at the
//! same time, each result kept as it arrives, the deadline and cancel, and the progress that
//! `comm.search.status` reports. A search ends when every source has finished, when it is
//! cancelled, or when its deadline passes; a result that arrives after that is dropped.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use super::merge::Devices;
use super::model::{
    ProtocolFind, SearchOrigin, SearchState, SearchStatus, SourceId, SourceProgress, SourceState,
    SEARCH_SCHEMA_VERSION,
};

/// The work of one source: it reports to its sink until it is done or the search ends.
pub(crate) type SourceRun = Box<dyn FnOnce(&SourceSink) + Send>;

/// What one source does in a search.
pub(crate) enum SourcePlan {
    /// It runs.
    Run(SourceRun),
    /// It does not run, for this reason.
    Skip(String),
}

/// How often a waiting source looks whether the search has ended.
const WAIT_STEP: Duration = Duration::from_millis(50);

/// A search that is running or has ended.
pub(crate) struct Search {
    shared: Arc<Shared>,
}

struct Shared {
    id: String,
    origin: SearchOrigin,
    deadline: Duration,
    started: Instant,
    progress: Mutex<Progress>,
}

struct Progress {
    sources: Vec<Slot>,
    devices: Devices,
    cancelled_at: Option<Instant>,
}

struct Slot {
    id: SourceId,
    skipped: Option<String>,
    finished_at: Option<Instant>,
    count: u32,
    warnings: Vec<String>,
}

/// When a search ends, and whether a cancel ends it.
#[derive(Clone, Copy)]
struct End {
    at: Instant,
    cancelled: bool,
}

impl Search {
    /// Starts every source of `plans` on its own thread; a skipped source finishes at once.
    pub(crate) fn start(
        id: String,
        origin: SearchOrigin,
        deadline: Duration,
        plans: Vec<(SourceId, SourcePlan)>,
    ) -> Self {
        let started = Instant::now();
        let mut slots = Vec::with_capacity(plans.len());
        let mut runs = Vec::new();
        for (index, (source, plan)) in plans.into_iter().enumerate() {
            let mut slot = Slot {
                id: source,
                skipped: None,
                finished_at: None,
                count: 0,
                warnings: Vec::new(),
            };
            match plan {
                SourcePlan::Skip(reason) => {
                    slot.skipped = Some(reason);
                    slot.finished_at = Some(started);
                }
                SourcePlan::Run(run) => runs.push((index, source, run)),
            }
            slots.push(slot);
        }
        let shared = Arc::new(Shared {
            id,
            origin,
            deadline,
            started,
            progress: Mutex::new(Progress {
                sources: slots,
                devices: Devices::default(),
                cancelled_at: None,
            }),
        });
        for (index, source, run) in runs {
            let sink = SourceSink {
                shared: Arc::clone(&shared),
                index,
            };
            let spawned = std::thread::Builder::new()
                .name(format!("search-{}", source.as_str()))
                .spawn(move || {
                    run(&sink);
                    sink.finish();
                });
            if let Err(error) = spawned {
                let mut progress = shared.lock();
                let slot = &mut progress.sources[index];
                slot.skipped = Some(format!("The source could not start: {error}."));
                slot.finished_at = Some(started);
            }
        }
        Self { shared }
    }

    /// The search ID.
    pub(crate) fn id(&self) -> &str {
        &self.shared.id
    }

    /// Ends a running search; answers the search's state afterwards.
    pub(crate) fn cancel(&self) -> SearchState {
        let now = Instant::now();
        let mut progress = self.shared.lock();
        if now < self.shared.end(&progress).at {
            progress.cancelled_at = Some(now);
        }
        state_at(self.shared.end(&progress), now)
    }

    /// The search's progress, with every device or only those changed after `since`.
    pub(crate) fn status(&self, since: Option<u64>) -> SearchStatus {
        let now = Instant::now();
        let progress = self.shared.lock();
        let end = self.shared.end(&progress);
        let (devices, merged) = progress.devices.snapshot(since);
        SearchStatus {
            schema_version: SEARCH_SCHEMA_VERSION,
            search_id: self.shared.id.clone(),
            origin: self.shared.origin,
            state: state_at(end, now),
            deadline_ms: millis(self.shared.deadline),
            elapsed_ms: millis(
                end.at
                    .min(now)
                    .saturating_duration_since(self.shared.started),
            ),
            cursor: progress.devices.cursor(),
            sources: progress
                .sources
                .iter()
                .map(|slot| slot_progress(slot, end, now))
                .collect(),
            devices,
            merged,
            warnings: Vec::new(),
        }
    }

    /// When the search ended; `None` while it runs.
    pub(crate) fn ended_at(&self) -> Option<Instant> {
        let now = Instant::now();
        let progress = self.shared.lock();
        let end = self.shared.end(&progress);
        (now >= end.at).then_some(end.at)
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Progress> {
        self.progress.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The first of the deadline, the cancel, and the time the last source finished.
    fn end(&self, progress: &Progress) -> End {
        let mut end = End {
            at: self.started + self.deadline,
            cancelled: false,
        };
        if let Some(cancelled_at) = progress.cancelled_at
            && cancelled_at < end.at
        {
            end = End {
                at: cancelled_at,
                cancelled: true,
            };
        }
        let all_finished = progress
            .sources
            .iter()
            .try_fold(self.started, |latest, slot| {
                slot.finished_at.map(|finished| latest.max(finished))
            });
        if let Some(finished_at) = all_finished
            && finished_at <= end.at
        {
            end = End {
                at: finished_at,
                cancelled: false,
            };
        }
        end
    }
}

/// Where one source reports: its results, its warnings, and whether the search goes on.
pub(crate) struct SourceSink {
    shared: Arc<Shared>,
    index: usize,
}

impl SourceSink {
    /// Keeps one result, unless the search has ended.
    pub(crate) fn found(&self, find: ProtocolFind) {
        let mut progress = self.shared.lock();
        if Instant::now() >= self.shared.end(&progress).at {
            return;
        }
        if progress.devices.add(find) {
            progress.sources[self.index].count += 1;
        }
    }

    /// Keeps one warning, once, unless the search has ended.
    pub(crate) fn warn(&self, text: impl Into<String>) {
        let text = text.into();
        let mut progress = self.shared.lock();
        if Instant::now() >= self.shared.end(&progress).at {
            return;
        }
        let warnings = &mut progress.sources[self.index].warnings;
        if !warnings.contains(&text) {
            warnings.push(text);
        }
    }

    /// Whether the search has ended: its deadline passed or it was cancelled.
    pub(crate) fn ended(&self) -> bool {
        self.remaining().is_zero()
    }

    /// The time left until the search ends at its deadline; zero once it has ended.
    pub(crate) fn remaining(&self) -> Duration {
        let progress = self.shared.lock();
        self.shared
            .end(&progress)
            .at
            .saturating_duration_since(Instant::now())
    }

    /// Waits for up to `pause`, and less when the search ends meanwhile.
    pub(crate) fn wait(&self, pause: Duration) {
        let until = Instant::now() + pause;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() || self.ended() {
                return;
            }
            std::thread::sleep(left.min(WAIT_STEP));
        }
    }

    fn finish(&self) {
        let mut progress = self.shared.lock();
        progress.sources[self.index]
            .finished_at
            .get_or_insert_with(Instant::now);
    }
}

fn state_at(end: End, now: Instant) -> SearchState {
    if now < end.at {
        SearchState::Running
    } else if end.cancelled {
        SearchState::Cancelled
    } else {
        SearchState::Done
    }
}

fn slot_progress(slot: &Slot, end: End, now: Instant) -> SourceProgress {
    let state = match (&slot.skipped, slot.finished_at) {
        (Some(_), _) => SourceState::Skipped,
        (None, Some(finished_at)) if finished_at <= end.at => SourceState::Found,
        _ if now < end.at => SourceState::Searching,
        _ if end.cancelled => SourceState::Cancelled,
        _ => SourceState::TimedOut,
    };
    SourceProgress {
        id: slot.id,
        state,
        count: slot.skipped.is_none().then_some(slot.count),
        reason: slot.skipped.clone(),
        warnings: slot.warnings.clone(),
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
