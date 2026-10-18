//! A panel's session (design §1.5): what it gathered, the newest request it has seen, and when it
//! was last used. A session that stays idle for longer than ten minutes is closed. It also keeps
//! what the last view showed of each runtime, which the actions of the panel resolve their target
//! from (design §4.3 B7).
//!
//! - `runtimes`: what the last view showed of each runtime, and the catalog it serves.

mod runtimes;

pub use runtimes::{KnownConnection, KnownRuntime};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::browse::BrowseCache;
use crate::gather::{Cache, Gathered};
use crate::view::types::{DevicesView, OperationDone};

/// How long a session may stay idle before it is closed.
pub const IDLE_EXPIRY: Duration = Duration::from_mins(10);

/// One panel's session.
#[derive(Debug)]
pub struct Session {
    cache: Cache,
    newest_seq: Option<u64>,
    last_used: Instant,
    operations: Operations,
    /// What the last view showed of each runtime, in the order the view showed them.
    runtimes: Vec<KnownRuntime>,
    /// The open project's folder, which the offline browse reads (design §4.3 B8).
    project: Option<PathBuf>,
    /// The browse answers, filled by the background runs (design §4.3 B8).
    browse: BrowseCache,
}

/// The slow actions a session started, and the results that a later request delivers.
#[derive(Debug, Default)]
struct Operations {
    /// How many operations the session has started.
    started: u64,
    /// The finished results not yet delivered, in the order they finished.
    done: Vec<OperationDone>,
}

impl Session {
    fn new(now: Instant) -> Self {
        Self {
            cache: Cache::default(),
            newest_seq: None,
            last_used: now,
            operations: Operations::default(),
            runtimes: Vec::new(),
            project: None,
            browse: BrowseCache::default(),
        }
    }

    /// Keeps what `view` showed of each runtime, so that a later action resolves the runtime it
    /// writes to from what the panel last showed, never from a fact the extension sent (design
    /// §4.3 B7; specification 25 §5.14). `gathered` is the gather that the view was projected
    /// from, which also tells each runtime's served catalog; the view does not carry it.
    pub fn remember(&mut self, gathered: &Gathered, view: &DevicesView) {
        self.project.clone_from(&gathered.project.chosen);
        self.runtimes = view
            .computers
            .iter()
            .flat_map(|computer| &computer.runtimes)
            .map(|runtime| KnownRuntime::of(gathered, runtime))
            .collect();
    }

    /// The open project's folder, as the last view gathered it (design §4.3 B8): the offline
    /// browse reads a stopped project's files through it.
    #[must_use]
    pub(crate) fn project_folder(&self) -> Option<&Path> {
        self.project.as_deref()
    }

    /// The browse answers of this session, filled by the background runs (design §4.3 B8).
    #[must_use]
    pub(crate) fn browse(&self) -> &BrowseCache {
        &self.browse
    }

    /// Whether a browse run of this session is still out, so a test can wait for the next poll.
    #[cfg(test)]
    pub(crate) fn browse_asking(&self) -> bool {
        self.browse.asking()
    }

    /// The runtime that the last view showed under the item ID `id`; `None` when the session has
    /// shown no view yet or the view showed no such runtime.
    #[must_use]
    pub fn runtime(&self, id: &str) -> Option<&KnownRuntime> {
        self.runtimes
            .iter()
            .find(|runtime| runtime.id.as_str() == id)
    }

    /// Every runtime that the last view showed, in the order it showed them.
    #[must_use]
    pub fn runtimes(&self) -> &[KnownRuntime] {
        &self.runtimes
    }

    /// Records a request with the number `seq`. Returns whether it is newer than every request
    /// seen before; an older one is answered from the cache, without a gather.
    pub fn accept(&mut self, seq: u64) -> bool {
        let newest = self.newest_seq.is_none_or(|newest| seq > newest);
        if newest {
            self.newest_seq = Some(seq);
        }
        newest
    }

    /// What the session gathered.
    pub fn cache_mut(&mut self) -> &mut Cache {
        &mut self.cache
    }

    /// Starts a slow action and returns its identity, such as `op-7`.
    pub fn begin_operation(&mut self) -> String {
        self.operations.started += 1;
        format!("op-{}", self.operations.started)
    }

    /// Keeps the result of a finished operation until the next request delivers it.
    pub fn complete_operation(&mut self, done: OperationDone) {
        self.operations.done.push(done);
    }

    /// The finished operations, delivered exactly once: the caller shows each and it is
    /// forgotten (design §1.5).
    #[must_use]
    pub fn take_operations_done(&mut self) -> Vec<OperationDone> {
        std::mem::take(&mut self.operations.done)
    }
}

/// The open sessions.
#[derive(Debug, Default)]
pub struct Sessions {
    open: HashMap<String, Session>,
}

impl Sessions {
    /// The session `id`, opened when it is new and marked as used at `now`. Every session idle
    /// for longer than `IDLE_EXPIRY` is closed first.
    pub fn session(&mut self, id: &str, now: Instant) -> &mut Session {
        self.open
            .retain(|_, session| now.saturating_duration_since(session.last_used) <= IDLE_EXPIRY);
        let session = self
            .open
            .entry(id.to_string())
            .or_insert_with(|| Session::new(now));
        session.last_used = now;
        session
    }

    /// Closes the session `id`, as `trust-lsp.devicesClose` does. Returns whether it was open.
    pub fn close(&mut self, id: &str) -> bool {
        self.open.remove(id).is_some()
    }

    /// Whether the session `id` is open.
    #[must_use]
    pub fn is_open(&self, id: &str) -> bool {
        self.open.contains_key(id)
    }
}

#[cfg(test)]
mod tests;
