//! The session's browse answers (design §1.5, §4.3 B8 and §3.3 J52): one slot per browse, filled
//! by a background thread, so that `open` answers at once and a later poll reads the tree. An
//! answer whose number is older than the cached one never replaces it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use super::run::Browsed;

/// The browse answers of one session. A copy shares the answers, so an action's copy of the session
/// reads the same browse answers (design §1.5).
#[derive(Debug, Default, Clone)]
pub(crate) struct BrowseCache {
    slots: Arc<BrowseSlots>,
}

/// The shared table of slots, which the background run fills.
#[derive(Debug, Default)]
struct BrowseSlots {
    slots: Mutex<HashMap<String, Slot>>,
}

/// One browse: the number of the request that owns it, and its answer once the run finished.
#[derive(Debug, Default)]
struct Slot {
    /// The number of the request that owns the slot.
    seq: Option<u64>,
    /// The answer, once the run finished.
    answer: Option<Result<Browsed, String>>,
    /// Whether the run is still out.
    asking: bool,
}

impl BrowseCache {
    /// The answer of the browse `key`, read at once: the finished answer when it is at least as
    /// new as `seq`, else `None` while the background run fills it. An answer older than the
    /// cached one never replaces it, and a request older than the cached one starts no run.
    pub(crate) fn open<F>(&self, key: &str, seq: u64, run: F) -> Option<Result<Browsed, String>>
    where
        F: FnOnce() -> Result<Browsed, String> + Send + 'static,
    {
        {
            let mut table = lock(&self.slots.slots);
            if table
                .get(key)
                .and_then(|slot| slot.seq)
                .is_some_and(|current| current >= seq)
            {
                return table.get(key).and_then(|slot| slot.answer.clone());
            }
            table.insert(
                key.to_owned(),
                Slot {
                    seq: Some(seq),
                    answer: None,
                    asking: true,
                },
            );
        }
        let slots = Arc::clone(&self.slots);
        let owned = key.to_owned();
        let started = thread::Builder::new()
            .name("devices-browse".to_owned())
            .spawn(move || {
                let answer = run();
                let mut table = lock(&slots.slots);
                if let Some(slot) = table.get_mut(&owned).filter(|slot| slot.seq == Some(seq)) {
                    slot.answer = Some(answer);
                    slot.asking = false;
                }
            });
        if started.is_err() {
            let mut table = lock(&self.slots.slots);
            if let Some(slot) = table.get_mut(key) {
                slot.asking = false;
            }
        }
        None
    }

    /// The finished answer of the browse `key`, when one is there.
    pub(crate) fn answer(&self, key: &str) -> Option<Result<Browsed, String>> {
        lock(&self.slots.slots)
            .get(key)
            .and_then(|slot| slot.answer.clone())
    }

    /// Every browse of this session whose key starts with `prefix` and ends in a port, with that
    /// port's finished answer, or `None` while its run is out (design §3.3 J55): the ports a pane
    /// shows besides its own.
    pub(crate) fn answers_under(
        &self,
        prefix: &str,
    ) -> Vec<(u16, Option<Result<Browsed, String>>)> {
        let mut answers: Vec<(u16, Option<Result<Browsed, String>>)> = lock(&self.slots.slots)
            .iter()
            .filter_map(|(key, slot)| {
                let port = key.strip_prefix(prefix)?.parse::<u16>().ok()?;
                Some((port, slot.answer.clone()))
            })
            .collect();
        answers.sort_by_key(|(port, _)| *port);
        answers
    }

    /// Whether any browse run is still out, so a test can wait for the next poll's answer.
    #[cfg(test)]
    pub(crate) fn asking(&self) -> bool {
        lock(&self.slots.slots).values().any(|slot| slot.asking)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
