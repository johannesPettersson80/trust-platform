//! A service's heartbeat on a connection and a thread of their own (specification 48 RU-63): it
//! sends `service.heartbeat` at the service's `heartbeat` period until it is dropped, so that the
//! service's own work, and a slow request of it, never delay the heartbeat by which the program
//! judges the service alive. A heartbeat that came late, as a slow runtime answers it, is followed
//! one period later by the next: the ones it missed are not sent, as a task does not replay missed
//! activations (specification 10 §4.3).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::{Service, ServiceError};

/// The longest the thread sleeps at once, so that a drop stops it promptly.
const STEP: Duration = Duration::from_millis(50);

/// A service's heartbeat while it lives; dropping it stops the heartbeat.
#[derive(Debug)]
pub struct Heartbeat {
    stop: Arc<AtomicBool>,
    last_error: Arc<Mutex<Option<ServiceError>>>,
    thread: Option<JoinHandle<()>>,
}

impl Heartbeat {
    /// Starts sending the heartbeat of `service`, its own connection, at its period.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Settings`] for a period of zero, which would send heartbeats without pause,
    /// and [`ServiceError::Thread`] when the thread does not start.
    pub(crate) fn start(service: Service) -> Result<Self, ServiceError> {
        let period = service.settings().heartbeat;
        if period.is_zero() {
            return Err(ServiceError::Settings(format!(
                "[services.{}] heartbeat is zero: a heartbeat needs a period such as \"500ms\"",
                service.settings().name
            )));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let last_error = Arc::new(Mutex::new(None));
        let thread = std::thread::Builder::new()
            .name(format!("{}-heartbeat", service.settings().name))
            .spawn({
                let stop = Arc::clone(&stop);
                let last_error = Arc::clone(&last_error);
                move || {
                    let mut next = Instant::now();
                    while !stop.load(Ordering::Acquire) {
                        let sent = service.heartbeat();
                        *last_error.lock().unwrap_or_else(PoisonError::into_inner) = sent.err();
                        next += period;
                        // A late heartbeat is followed one period later, not by the ones it missed.
                        let now = Instant::now();
                        if next < now {
                            next = now + period;
                        }
                        while !stop.load(Ordering::Acquire) {
                            let now = Instant::now();
                            if now >= next {
                                break;
                            }
                            std::thread::sleep((next - now).min(STEP));
                        }
                    }
                }
            })
            .map_err(|error| ServiceError::Thread(error.to_string()))?;
        Ok(Self {
            stop,
            last_error,
            thread: Some(thread),
        })
    }

    /// The refusal or the failed connection of the last heartbeat; `None` while the heartbeats
    /// reach the runtime.
    #[must_use]
    pub fn last_error(&self) -> Option<ServiceError> {
        self.last_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for Heartbeat {
    /// Stops the heartbeat and waits for its thread to end.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
