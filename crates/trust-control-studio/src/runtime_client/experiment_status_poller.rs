//! Schedules one bounded status refresh while a runtime-owned Test remains active (`experimentStatusPoller.ts`).
//! The refresh runs on its own thread after `delay`. Scheduling while a refresh is pending does nothing, and
//! `dispose` cancels the pending refresh, so a refresh never follows a disposed poller.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;

/// The delay before a refresh runs: 250 ms (`experimentStatusPoller.ts`).
pub const DEFAULT_DELAY: Duration = Duration::from_millis(250);

type Refresh = dyn Fn() + Send + Sync;

/// One bounded status refresh at a time.
pub struct ExperimentStatusPoller {
    refresh: Arc<Refresh>,
    delay: Duration,
    /// The generation of the refresh that is due, while one is scheduled.
    due: Arc<Mutex<Option<u64>>>,
    generations: AtomicU64,
}

impl ExperimentStatusPoller {
    /// A poller that runs `refresh` after the default delay.
    pub fn new(refresh: impl Fn() + Send + Sync + 'static) -> Self {
        Self::with_delay(refresh, DEFAULT_DELAY)
    }

    /// A poller that runs `refresh` after `delay`.
    pub fn with_delay(refresh: impl Fn() + Send + Sync + 'static, delay: Duration) -> Self {
        Self {
            refresh: Arc::new(refresh),
            delay,
            due: Arc::new(Mutex::new(None)),
            generations: AtomicU64::new(0),
        }
    }

    /// Schedules one refresh unless one is already scheduled.
    pub fn schedule(&self) {
        let generation = {
            let mut due = self.due.lock();
            if due.is_some() {
                return;
            }
            let generation = self.generations.fetch_add(1, Ordering::SeqCst);
            *due = Some(generation);
            generation
        };
        let due = Arc::clone(&self.due);
        let refresh = Arc::clone(&self.refresh);
        let delay = self.delay;
        thread::spawn(move || {
            thread::sleep(delay);
            {
                let mut pending = due.lock();
                if *pending != Some(generation) {
                    return;
                }
                *pending = None;
            }
            refresh();
        });
    }

    /// Cancels the scheduled refresh, if any.
    pub fn dispose(&self) {
        *self.due.lock() = None;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    fn counting_poller(delay: Duration) -> (ExperimentStatusPoller, mpsc::Receiver<()>) {
        let (sender, receiver) = mpsc::channel();
        let sender = Mutex::new(sender);
        let poller = ExperimentStatusPoller::with_delay(
            move || {
                let _ = sender.lock().send(());
            },
            delay,
        );
        (poller, receiver)
    }

    #[test]
    fn a_scheduled_refresh_runs_once_after_the_delay() {
        let (poller, refreshes) = counting_poller(Duration::from_millis(5));
        poller.schedule();
        poller.schedule();
        refreshes
            .recv_timeout(Duration::from_secs(5))
            .expect("one refresh");
        assert!(
            refreshes.recv_timeout(Duration::from_millis(100)).is_err(),
            "a second schedule while one is pending adds no refresh"
        );
    }

    #[test]
    fn a_refresh_scheduled_after_the_last_one_ran_runs_again() {
        let (poller, refreshes) = counting_poller(Duration::from_millis(5));
        poller.schedule();
        refreshes
            .recv_timeout(Duration::from_secs(5))
            .expect("first refresh");
        poller.schedule();
        refreshes
            .recv_timeout(Duration::from_secs(5))
            .expect("second refresh");
    }

    #[test]
    fn dispose_cancels_the_pending_refresh() {
        let (poller, refreshes) = counting_poller(Duration::from_millis(50));
        poller.schedule();
        poller.dispose();
        assert!(
            refreshes.recv_timeout(Duration::from_millis(300)).is_err(),
            "a disposed poller never refreshes"
        );
    }
}
