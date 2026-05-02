/// Clock interface for resource scheduling.
pub trait Clock: Send + Sync + 'static {
    /// Return the current time for scheduling.
    fn now(&self) -> Duration;

    /// Sleep until the given deadline.
    fn sleep_until(&self, deadline: Duration);

    /// Wake any sleepers (best-effort).
    fn wake(&self) {
        // Default: no-op for clocks without a wait mechanism.
    }
}

/// Monotonic clock based on `std::time::Instant`.
#[derive(Debug, Clone)]
pub struct StdClock {
    start: std::time::Instant,
}

impl StdClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }
}

impl Default for StdClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for StdClock {
    fn now(&self) -> Duration {
        let elapsed = self.start.elapsed();
        let nanos = i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX);
        Duration::from_nanos(nanos)
    }

    fn sleep_until(&self, deadline: Duration) {
        let now = self.now();
        let delta = deadline.as_nanos() - now.as_nanos();
        if delta <= 0 {
            return;
        }
        let delta = u64::try_from(delta).unwrap_or(u64::MAX);
        thread::sleep(std::time::Duration::from_nanos(delta));
    }
}

/// Monotonic clock with simulation time acceleration.
#[derive(Debug, Clone)]
pub struct ScaledClock {
    start: std::time::Instant,
    scale: u32,
}

impl ScaledClock {
    #[must_use]
    pub fn new(scale: u32) -> Self {
        Self {
            start: std::time::Instant::now(),
            scale: scale.max(1),
        }
    }

    #[must_use]
    pub fn scale(&self) -> u32 {
        self.scale
    }
}

impl Clock for ScaledClock {
    fn now(&self) -> Duration {
        let elapsed = self.start.elapsed();
        let scaled_nanos = elapsed
            .as_nanos()
            .saturating_mul(self.scale as u128)
            .min(i64::MAX as u128);
        Duration::from_nanos(scaled_nanos as i64)
    }

    fn sleep_until(&self, deadline: Duration) {
        let now = self.now();
        let delta_sim = deadline.as_nanos().saturating_sub(now.as_nanos());
        if delta_sim <= 0 {
            return;
        }
        let scale = i64::from(self.scale.max(1));
        let delta_real = (delta_sim + scale - 1) / scale;
        if delta_real <= 0 {
            thread::yield_now();
            return;
        }
        let nanos = u64::try_from(delta_real).unwrap_or(u64::MAX);
        thread::sleep(std::time::Duration::from_nanos(nanos));
    }
}

#[derive(Debug)]
struct ManualClockState {
    now: Duration,
    sleep_calls: u64,
    interrupted: bool,
}

/// Deterministic clock for tests and simulations.
#[derive(Debug, Clone)]
pub struct ManualClock {
    inner: Arc<(Mutex<ManualClockState>, Condvar)>,
}

impl ManualClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new((
                Mutex::new(ManualClockState {
                    now: Duration::ZERO,
                    sleep_calls: 0,
                    interrupted: false,
                }),
                Condvar::new(),
            )),
        }
    }

    /// Return the current manual time.
    #[must_use]
    pub fn current_time(&self) -> Duration {
        let (lock, _) = &*self.inner;
        let state = recover_manual_clock_lock(lock.lock());
        state.now
    }

    /// Advance time by the given delta.
    pub fn advance(&self, delta: Duration) -> Duration {
        let (lock, cvar) = &*self.inner;
        let mut state = recover_manual_clock_lock(lock.lock());
        let next = state.now.as_nanos().saturating_add(delta.as_nanos());
        state.now = Duration::from_nanos(next);
        cvar.notify_all();
        state.now
    }

    /// Set the current time explicitly.
    pub fn set_time(&self, time: Duration) {
        let (lock, cvar) = &*self.inner;
        let mut state = recover_manual_clock_lock(lock.lock());
        state.now = time;
        cvar.notify_all();
    }

    /// Number of sleep calls issued to this clock.
    #[must_use]
    pub fn sleep_calls(&self) -> u64 {
        let (lock, _) = &*self.inner;
        let state = recover_manual_clock_lock(lock.lock());
        state.sleep_calls
    }

    /// Interrupt sleepers so they can exit.
    pub fn interrupt(&self) {
        let (lock, cvar) = &*self.inner;
        let mut state = recover_manual_clock_lock(lock.lock());
        state.interrupted = true;
        cvar.notify_all();
    }
}

fn recover_manual_clock_lock<T>(
    result: std::sync::LockResult<std::sync::MutexGuard<'_, T>>,
) -> std::sync::MutexGuard<'_, T> {
    result.unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        self.current_time()
    }

    fn sleep_until(&self, deadline: Duration) {
        let (lock, cvar) = &*self.inner;
        let mut state = recover_manual_clock_lock(lock.lock());
        state.sleep_calls = state.sleep_calls.saturating_add(1);
        while !state.interrupted && state.now.as_nanos() < deadline.as_nanos() {
            state = recover_manual_clock_lock(cvar.wait(state));
        }
    }

    fn wake(&self) {
        self.interrupt();
    }
}

#[cfg(test)]
mod clock_poison_tests {
    use super::*;

    #[test]
    fn manual_clock_recovers_from_poisoned_lock() {
        let clock = ManualClock::new();
        let inner = Arc::clone(&clock.inner);
        let _ = std::thread::spawn(move || {
            let (lock, _) = &*inner;
            let mut state = lock.lock().expect("test lock should be available");
            state.now = Duration::from_millis(7);
            panic!("poison manual clock lock");
        })
        .join();

        assert_eq!(clock.current_time(), Duration::from_millis(7));
        assert_eq!(
            clock.advance(Duration::from_millis(5)),
            Duration::from_millis(12)
        );
    }
}
