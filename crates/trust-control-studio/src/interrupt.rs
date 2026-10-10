//! Ctrl-C presses of the command line (specification 35, §0.1b). The first press asks a running
//! operation to cancel, and the verb keeps waiting for the operation's final update. The second
//! press leaves the experiment, its recording and the journal to the runtime, and the verb ends with
//! status 130. A verb that answers at once ends with 130 on any press. The presses are counted when
//! the verb asks, so two presses within one wait count as one.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use signal_hook::consts::signal::SIGINT;

/// The Ctrl-C presses of this process, counted as the verb asks.
#[derive(Debug)]
pub struct Presses {
    pending: Arc<AtomicBool>,
    counted: u32,
}

impl Presses {
    /// Starts counting Ctrl-C (SIGINT on Unix and Windows) for this process. From then on a press
    /// does not end the process by itself; the verb decides what it does.
    ///
    /// # Errors
    /// The platform refuses the signal handler.
    pub fn register() -> io::Result<Self> {
        let pending = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(SIGINT, Arc::clone(&pending))?;
        Ok(Self::from_flag(pending))
    }

    /// Counts the presses that the flag records. The signal handler sets the flag; nothing is
    /// registered here.
    #[must_use]
    pub fn from_flag(pending: Arc<AtomicBool>) -> Self {
        Self {
            pending,
            counted: 0,
        }
    }

    /// The presses counted so far, including one that arrived since the last call.
    pub fn count(&mut self) -> u32 {
        if self.pending.swap(false, Ordering::SeqCst) {
            self.counted = self.counted.saturating_add(1);
        }
        self.counted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_press_is_counted_once_however_often_the_verb_asks() {
        let pending = Arc::new(AtomicBool::new(false));
        let mut presses = Presses::from_flag(Arc::clone(&pending));
        assert_eq!(presses.count(), 0);
        pending.store(true, Ordering::SeqCst);
        assert_eq!(presses.count(), 1);
        assert_eq!(presses.count(), 1, "a press is counted once");
        pending.store(true, Ordering::SeqCst);
        assert_eq!(presses.count(), 2, "the second press is counted");
    }
}
