//! The cycle grid of the hosted scheduler (specification 10 §4.3; specification 34 CYCLE-03): a
//! wake that comes late does not move the next cycle. The tests build a program, which needs the
//! toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

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
    let runtime = crate::harness::TestHarness::from_source("PROGRAM Main\nEND_PROGRAM\n")
        .expect("build the runner fixture")
        .into_runtime();
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

#[test]
fn a_restart_begins_the_cycle_grid_again_at_its_first_cycle() {
    // Specification 10 §4.3: a restart begins the grid again at the start of its first cycle,
    // where the tasks' timing begins again. Every sleep wakes 1 ms late. The first three cycles start
    // at 0, 11 and 21 ms on the grid of 0; a warm restart asked for in the third is taken at the
    // next pass, whose cycle starts at 31 ms, so the next nominal starts are 41 and 51 ms, and the
    // late wakes start those cycles at 42 and 52 ms. Catches a restart that keeps the old grid (41
    // and 51 ms), whose first cycle after the restart's comes less than an interval after the
    // restart, before the restarted tasks are due (the transcript of
    // `runner_transcript_parity.rs`, 23 October 2026).
    let runtime = crate::harness::TestHarness::from_source("PROGRAM Main\nEND_PROGRAM\n")
        .expect("build the runner fixture")
        .into_runtime();
    let restart = Arc::new(Mutex::new(None));
    let runner = ResourceRunner::new(runtime, LateClock::default(), Duration::from_millis(10))
        .with_restart_signal(Arc::clone(&restart));
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
            if starts.len() == 3 {
                *recover_mutex_lock(restart.lock()) = Some(crate::RestartMode::Warm);
            }
            if starts.len() == 6 {
                stop_after.store(true, Ordering::SeqCst);
            }
            Ok(())
        },
    );
    assert_eq!(*recover_mutex_lock(starts.lock()), [0, 11, 21, 31, 42, 52]);
}

#[test]
fn a_start_after_a_stop_begins_the_cycle_grid_again_at_its_first_cycle() {
    // Specification 10 §4.3 and specification 34 STATE-08: a start of the stopped program begins the
    // grid again at the start of its first cycle. Every sleep wakes 1 ms late, and the stopped wait
    // polls on its own, so the clock stands at 31 ms while the program is stopped. The first three
    // cycles start at 0, 11 and 21 ms; a `shutdown` asked for in the third stops the program at the
    // next pass, and a `restart` taken while it is stopped starts it at 31 ms, so the next nominal
    // starts are 41 and 51 ms, and the late wakes start those cycles at 42 and 52 ms. Catches a start
    // that keeps the grid of the program it stopped (41 and 51 ms) or the boundary of the last cycle
    // before the stop, which a long stop puts in the past and which then starts a second cycle at
    // once after the first.
    let runtime = crate::harness::TestHarness::from_source("PROGRAM Main\nEND_PROGRAM\n")
        .expect("build the runner fixture")
        .into_runtime();
    let restart = Arc::new(Mutex::new(None));
    let runner = ResourceRunner::new(runtime, LateClock::default(), Duration::from_millis(10))
        .with_restart_signal(Arc::clone(&restart));
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
    let stop_program = Arc::clone(&supervision);
    std::thread::scope(|scope| {
        // Takes the place of the control plane: once the program reads Stopped, `restart cold`.
        let watched = Arc::clone(&state);
        let start = Arc::clone(&restart);
        scope.spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while *recover_mutex_lock(watched.lock()) != ResourceState::Stopped
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            *recover_mutex_lock(start.lock()) = Some(crate::RestartMode::Cold);
        });
        run_resource_loop_core(
            runner,
            stop,
            state,
            last_error,
            supervision,
            move |runtime| {
                let mut starts = recover_mutex_lock(starts_of_cycles.lock());
                starts.push(runtime.current_time().as_millis());
                if starts.len() == 3 {
                    stop_program.request_program_stop();
                }
                if starts.len() == 6 {
                    stop_after.store(true, Ordering::SeqCst);
                }
                Ok(())
            },
        );
    });
    assert_eq!(*recover_mutex_lock(starts.lock()), [0, 11, 21, 31, 42, 52]);
}
