//! A panel's session (design §1.5): what it gathered, the newest request it has seen, and when it
//! was last used. A session that stays idle for longer than ten minutes is closed.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::gather::Cache;

/// How long a session may stay idle before it is closed.
pub const IDLE_EXPIRY: Duration = Duration::from_mins(10);

/// One panel's session.
#[derive(Debug)]
pub struct Session {
    cache: Cache,
    newest_seq: Option<u64>,
    last_used: Instant,
}

impl Session {
    fn new(now: Instant) -> Self {
        Self {
            cache: Cache::default(),
            newest_seq: None,
            last_used: now,
        }
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
