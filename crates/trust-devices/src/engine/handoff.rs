//! The hand-over of a background job's answer to the request that started it (design §1.5). A devices
//! request waits a short time for its own answer. An answer that comes later is given back to the
//! job, which delivers it another way, so the answer is never lost and never delivered twice. A request
//! that the panel asks again, as it asks a runtime's search again (specification 25 §2), waits with
//! [`Handoff::wait_again`] instead: an answer that comes later is kept for its next wait.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// The answer of one background job, handed to the request that started the job.
#[derive(Debug)]
pub(crate) struct Handoff<T> {
    state: Mutex<State<T>>,
    ready: Condvar,
}

/// Where a job's answer stands.
#[derive(Debug)]
enum State<T> {
    /// The job is still running, and the request waits for it.
    Running,
    /// The job answered in time, and the request has not taken the answer yet.
    Done(Box<T>),
    /// The request has left without the answer, or has taken it.
    Left,
}

impl<T> Handoff<T> {
    /// A hand-over whose job has not answered yet.
    pub(crate) fn open() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State::Running),
            ready: Condvar::new(),
        })
    }

    /// The job's answer. It is kept for the request while the request waits. Otherwise it is given
    /// back, so the job can deliver it another way.
    pub(crate) fn finish(&self, answer: T) -> Result<(), T> {
        let mut state = lock(&self.state);
        if matches!(*state, State::Running) {
            *state = State::Done(Box::new(answer));
            self.ready.notify_all();
            Ok(())
        } else {
            Err(answer)
        }
    }

    /// The request's wait: the job's answer when it comes within `wait`. Otherwise the request
    /// leaves with `None`, and the job's later answer is given back to it.
    pub(crate) fn wait(&self, wait: Duration) -> Option<T> {
        let state = lock(&self.state);
        let (mut state, _) = self
            .ready
            .wait_timeout_while(state, wait, |state| matches!(state, State::Running))
            .unwrap_or_else(PoisonError::into_inner);
        match std::mem::replace(&mut *state, State::Left) {
            State::Done(answer) => Some(*answer),
            State::Running | State::Left => None,
        }
    }

    /// A wait that the request may repeat: the job's answer when it comes within `wait`. Otherwise
    /// `None`, and an answer that comes later is kept for the next wait, never given back to the job.
    pub(crate) fn wait_again(&self, wait: Duration) -> Option<T> {
        let state = lock(&self.state);
        let (mut state, _) = self
            .ready
            .wait_timeout_while(state, wait, |state| matches!(state, State::Running))
            .unwrap_or_else(PoisonError::into_inner);
        if matches!(*state, State::Running) {
            return None;
        }
        match std::mem::replace(&mut *state, State::Left) {
            State::Done(answer) => Some(*answer),
            State::Running | State::Left => None,
        }
    }
}

/// Locks `mutex`, recovering a poisoned lock: the hand-over holds plain values only.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "handoff_tests.rs"]
mod tests;
