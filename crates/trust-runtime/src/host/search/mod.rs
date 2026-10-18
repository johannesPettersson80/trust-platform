//! The one search (specification 11, "One search, device identity, and presence"): every source
//! that its origin can run, at the same time and within one deadline, with one device for each
//! thing found and every protocol found on it.
//!
//! [`SearchRegistry`](crate::search::SearchRegistry) starts searches and keeps the last eight,
//! each for five minutes after it
//! ends. `request` reads and checks the requests, `model` is what a status reports, `runner`
//! runs the sources of one search, `merge` and `presence` group the results into devices, and
//! `sources` decides which sources run and runs them. A runtime serves the search through
//! `comm.search.*`, the command line through `trust-runtime discover`, and `comm.discover` asks
//! one protocol's source at a time. `project_choices` reads and writes the choices that a project
//! remembers for the search (specification 25 §2). The model, merge and presence do no I/O.

pub mod project_choices;

mod merge;
mod model;
mod presence;
mod request;
mod runner;
pub(crate) mod sources;

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

pub use model::{
    AdsIdentity, EthercatIdentity, FoundDevice, GpioIdentity, Identity, MergedDevice,
    ModbusIdentity, MqttIdentity, OpcuaIdentity, PortUsbDevice, ProtocolFind, RuntimeIdentity,
    SearchOrigin, SearchState, SearchStatus, SerialBus, SerialIdentity, SourceId, SourceProgress,
    SourceState, UsbIdentity, UsbRole, SEARCH_SCHEMA_VERSION,
};
pub use request::{
    CancelRequest, FieldbusBus, FieldbusTarget, SearchError, SearchRequest, SearchTargets,
    StatusRequest, DEFAULT_DEADLINE_MS, MAX_DEADLINE_MS, MAX_HOSTS,
};
pub use sources::{RuntimeFacts, SearchHost};

use runner::{Search, SourcePlan};

/// How many searches a registry keeps.
pub const KEPT_SEARCHES: usize = 8;
/// How long a registry keeps a search after it ended.
pub const KEPT_AFTER_END: Duration = Duration::from_secs(5 * 60);
/// Why the Bluetooth source does not run in this version of truST: the search skips it from every
/// origin (specification 11, "Sources"). The Discover pane shows this reason (specification 25 §2).
pub const BLUETOOTH_UNAVAILABLE: &str =
    "Bluetooth search is not available in this truST version yet.";

/// The searches of one runtime or one tool: it starts them and keeps the last
/// [`KEPT_SEARCHES`], each for [`KEPT_AFTER_END`] after it ends.
pub struct SearchRegistry {
    kept: Mutex<Kept>,
}

struct Kept {
    next_id: u64,
    searches: VecDeque<Search>,
    capacity: usize,
    retention: Duration,
}

impl Default for SearchRegistry {
    fn default() -> Self {
        Self::with_limits(KEPT_SEARCHES, KEPT_AFTER_END)
    }
}

impl fmt::Debug for SearchRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchRegistry")
            .field("searches", &self.lock().searches.len())
            .finish()
    }
}

impl SearchRegistry {
    /// A registry without searches.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry that keeps `capacity` searches, each for `retention` after it ends.
    pub(crate) fn with_limits(capacity: usize, retention: Duration) -> Self {
        Self {
            kept: Mutex::new(Kept {
                next_id: 0,
                searches: VecDeque::new(),
                capacity,
                retention,
            }),
        }
    }

    /// Checks `request` and starts its search where `host` says; answers the search ID.
    ///
    /// # Errors
    /// The request breaks a rule, or every kept search is still running.
    pub fn start(&self, request: SearchRequest, host: SearchHost) -> Result<String, SearchError> {
        let origin = host.origin();
        let search = request.validate(origin)?;
        let plans = sources::plan(&search, &host);
        self.start_plans(origin, search.deadline, plans)
    }

    /// Starts a search of `plans`. A search beyond the kept number replaces the oldest search
    /// that has ended.
    pub(crate) fn start_plans(
        &self,
        origin: SearchOrigin,
        deadline: Duration,
        plans: Vec<(SourceId, SourcePlan)>,
    ) -> Result<String, SearchError> {
        let mut kept = self.lock();
        kept.forget_expired();
        if kept.searches.len() >= kept.capacity {
            let oldest_ended = kept
                .searches
                .iter()
                .position(|search| search.ended_at().is_some());
            match oldest_ended {
                Some(index) => {
                    kept.searches.remove(index);
                }
                None => return Err(SearchError::Busy(kept.searches.len())),
            }
        }
        kept.next_id += 1;
        let id = format!("s-{}", kept.next_id);
        kept.searches
            .push_back(Search::start(id.clone(), origin, deadline, plans));
        Ok(id)
    }

    /// The progress of a kept search, with every device or only those changed after `since`.
    ///
    /// # Errors
    /// No kept search has this ID.
    pub fn status(&self, search_id: &str, since: Option<u64>) -> Result<SearchStatus, SearchError> {
        let mut kept = self.lock();
        kept.forget_expired();
        kept.find(search_id).map(|search| search.status(since))
    }

    /// Cancels a kept search; answers its state afterwards: cancelled, or done when it had
    /// already ended.
    ///
    /// # Errors
    /// No kept search has this ID.
    pub fn cancel(&self, search_id: &str) -> Result<SearchState, SearchError> {
        let mut kept = self.lock();
        kept.forget_expired();
        kept.find(search_id).map(Search::cancel)
    }

    fn lock(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Kept {
    fn forget_expired(&mut self) {
        let now = Instant::now();
        let retention = self.retention;
        self.searches.retain(|search| {
            search
                .ended_at()
                .is_none_or(|ended| now.saturating_duration_since(ended) < retention)
        });
    }

    fn find(&self, search_id: &str) -> Result<&Search, SearchError> {
        self.searches
            .iter()
            .find(|search| search.id() == search_id)
            .ok_or_else(|| SearchError::NotFound(search_id.to_string()))
    }
}

#[cfg(test)]
mod tests;
