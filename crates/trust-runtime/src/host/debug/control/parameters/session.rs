//! Session epochs (A2.2). Each runtime process starts its epoch counter at its own start time, so a restarted runtime
//! cannot repeat an epoch of an earlier process. Every reload or restart takes a new epoch.
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// A session: its start instant in nanoseconds, its process, and its epoch number.
pub(super) type Session = (u128, u32, u64);

/// The counter of epoch numbers. It starts once per process, at the process's start time.
static NEXT_EPOCH: OnceLock<AtomicU64> = OnceLock::new();

/// A session that begins now.
pub(super) fn new_session() -> Session {
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    (started, std::process::id(), next_epoch())
}

fn next_epoch() -> u64 {
    NEXT_EPOCH
        .get_or_init(|| AtomicU64::new(process_start_micros()))
        .fetch_add(1, Ordering::Relaxed)
}

/// Microseconds since the Unix epoch at this process's first session. Microseconds keep every epoch below 2^53, the
/// largest integer that a JSON client holds exactly (E0.3), and still differ between processes.
fn process_start_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_micros()).ok())
        .unwrap_or(0)
}
