//! The cycle grid of the hosted scheduler (specification 10 §4.3; specification 34 CYCLE-03): a
//! wake that comes late does not move the next cycle.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::*;

/// A clock whose sleep wakes 1 ms after its deadline, as a late wake of the operating system.
#[derive(Clone, Debug, Default)]
struct LateClock(Arc<Mutex<i64>>);

impl Clock for LateClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(*recover_mutex_lock(self.0.lock()))
    }

    fn sleep_until(&self, deadline: Duration) {
        *recover_mutex_lock(self.0.lock()) = deadline.as_nanos() + 1_000_000;
    }

    fn wake(&self) {}
}

#[test]
fn a_late_wake_does_not_move_the_cycle_grid() {
    // Specification 10 §4.3 and specification 34 CYCLE-03: the resource samples its tasks on a
    // fixed grid of the cycle interval. Every sleep here wakes 1 ms late: the cycles start 1 ms
    // after each 10 ms boundary, never later. Catches pacing one interval after the cycle's
    // start, which drifts 1 ms per cycle (0, 11, 22, 33 ms) and loses a 10 ms activation every
    // ten cycles.
    let runtime = crate::RuntimeBuilder::new()
        .build()
        .expect("admit runner fixture");
    let runner = ResourceRunner::new(runtime, LateClock::default(), Duration::from_millis(10));
    let stop = Arc::new(AtomicBool::new(false));
    let state = Arc::new(Mutex::new(ResourceState::Boot));
    let last_error = Arc::new(Mutex::new(None));
    let supervision = Arc::new(supervisor::ScanSupervision::new(
        Arc::clone(&state),
        Arc::clone(&last_error),
    ));
    let starts = Arc::new(Mutex::new(Vec::new()));
    let starts_of_cycles = Arc::clone(&starts);
    let stop_after = Arc::clone(&stop);
    run_resource_loop_core(
        runner,
        stop,
        state,
        last_error,
        supervision,
        move |runtime| {
            let mut starts = recover_mutex_lock(starts_of_cycles.lock());
            starts.push(runtime.current_time().as_millis());
            if starts.len() == 6 {
                stop_after.store(true, Ordering::SeqCst);
            }
            Ok(())
        },
    );
    assert_eq!(*recover_mutex_lock(starts.lock()), [0, 11, 21, 31, 41, 51]);
}
