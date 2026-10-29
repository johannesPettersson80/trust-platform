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

/// A clock whose sleeps wake at their deadlines, except the second, which wakes 15 ms late, as an
/// operating system that delays one wake by more than a cycle interval, and the third, which wakes
/// 6 ms late.
#[derive(Clone, Debug, Default)]
struct LateWakesClock {
    now: Arc<Mutex<i64>>,
    sleeps: Arc<Mutex<u32>>,
}

impl Clock for LateWakesClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(*recover_mutex_lock(self.now.lock()))
    }

    fn sleep_until(&self, deadline: Duration) {
        let mut sleeps = recover_mutex_lock(self.sleeps.lock());
        *sleeps += 1;
        let late = match *sleeps {
            2 => 15_000_000,
            3 => 6_000_000,
            _ => 0,
        };
        *recover_mutex_lock(self.now.lock()) = deadline.as_nanos() + late;
    }

    fn wake(&self) {}
}

/// A clock whose sleeps wake at their deadlines; a test moves it on to make a cycle run long.
#[derive(Clone, Debug, Default)]
struct ExactClock(Arc<Mutex<i64>>);

impl Clock for ExactClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(*recover_mutex_lock(self.0.lock()))
    }

    fn sleep_until(&self, deadline: Duration) {
        *recover_mutex_lock(self.0.lock()) = deadline.as_nanos();
    }

    fn wake(&self) {}
}

/// Runs six cycles of a 10 ms resource on `clock`, calling `during` with the number of each cycle
/// as it runs, and returns the cycles' start times in ms.
fn six_cycle_starts<C: Clock + Clone>(clock: C, during: impl Fn(usize)) -> Vec<i64> {
    let runtime = crate::harness::TestHarness::from_source("PROGRAM Main\nEND_PROGRAM\n")
        .expect("build the runner fixture")
        .into_runtime();
    let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(10));
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
            during(starts.len());
            if starts.len() == 6 {
                stop_after.store(true, Ordering::SeqCst);
            }
            Ok(())
        },
    );
    recover_mutex_lock(starts.lock()).clone()
}

/// Whether a 10 ms task, whose timing begins at the first cycle, is due in each later cycle that
/// starts at `starts` ms (specification 10 §4.3), and the activations it counts as missed.
fn task_due_in_each_later_cycle(starts: &[i64]) -> (Vec<bool>, u64) {
    task_due_in_each_cycle(starts[0], &starts[1..])
}

/// Whether a 10 ms task, whose timing begins at `origin` ms, is due in each cycle that starts at
/// `starts` ms (specification 10 §4.3), and the activations it counts as missed.
fn task_due_in_each_cycle(origin: i64, starts: &[i64]) -> (Vec<bool>, u64) {
    use trust_runtime_core::task::{evaluate_task_readiness, TaskState};
    let interval = Duration::from_millis(10);
    let mut state = TaskState::new(Duration::from_millis(origin));
    let mut missed = 0;
    let due = starts
        .iter()
        .map(|start| {
            let readiness =
                evaluate_task_readiness(&mut state, interval, false, Duration::from_millis(*start));
            missed += readiness.missed_intervals;
            readiness.due_at.is_some()
        })
        .collect();
    (due, missed)
}

/// A clock that first reads 37 ms, as a program's first cycle that starts that long after the
/// program's time began, after the start gate or a slow start, and whose second sleep wakes 4 ms
/// late.
#[derive(Clone, Debug)]
struct SlowStartClock {
    now: Arc<Mutex<i64>>,
    sleeps: Arc<Mutex<u32>>,
}

impl Default for SlowStartClock {
    fn default() -> Self {
        Self {
            now: Arc::new(Mutex::new(37_000_000)),
            sleeps: Arc::default(),
        }
    }
}

impl Clock for SlowStartClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(*recover_mutex_lock(self.now.lock()))
    }

    fn sleep_until(&self, deadline: Duration) {
        let mut sleeps = recover_mutex_lock(self.sleeps.lock());
        *sleeps += 1;
        let late = if *sleeps == 2 { 4_000_000 } else { 0 };
        *recover_mutex_lock(self.now.lock()) = deadline.as_nanos() + late;
    }

    fn wake(&self) {}
}

#[test]
fn the_grid_begins_where_the_tasks_timing_begins() {
    // Specification 10 §4.3: the grid begins where the tasks' timing begins, so its boundaries are
    // the tasks' deadlines. The program's time and its 10 ms task begin at 0 ms, and its first
    // cycle starts at 37 ms, on the boundary of 30 ms; the cycles go on at 40, 54 (a wake 4 ms
    // late), 60, 70 and 80 ms, and the task is due in every one. Catches a grid that begins at the
    // first cycle's start, 37 ms, off the task's deadlines: its wake 4 ms late then comes at
    // 61 ms, past the deadline of 60 ms, and the cycle of 67 ms finds the task not yet due yet
    // counts as a scan (the macOS runs of `tutorial_ctl_commands.rs` and `runtime_ctl_journeys.rs`,
    // 28 October 2026).
    let starts = six_cycle_starts(SlowStartClock::default(), |_| {});
    assert_eq!(starts, [37, 40, 54, 60, 70, 80]);
    assert_eq!(task_due_in_each_cycle(0, &starts), (vec![true; 6], 2));
}

#[test]
fn a_wake_late_by_more_than_an_interval_skips_the_boundaries_it_missed() {
    // Specification 10 §4.3: a cycle that starts a whole interval or more after its nominal start
    // takes the last boundary of the grid at or before its start, so the grid stays on the tasks'
    // deadlines. On a 10 ms grid the cycles start at 0 and 10 ms; the wake for 20 ms comes at
    // 35 ms, a cycle on the boundary of 30 ms, and the wake for 40 ms comes 6 ms late. A 10 ms task
    // is then due in every cycle after the first, and counts the activation of 20 ms as missed.
    // Catches a grid that begins again at 35 ms: the 6 ms late wake then comes at 51 ms, past the
    // task's deadline of 50 ms, and the cycle of 55 ms finds the task not yet due yet counts as a
    // scan.
    let starts = six_cycle_starts(LateWakesClock::default(), |_| {});
    assert_eq!(starts, [0, 10, 35, 46, 50, 60]);
    assert_eq!(task_due_in_each_later_cycle(&starts), (vec![true; 5], 1));
}

#[test]
fn a_cycle_that_overruns_its_interval_keeps_the_grid() {
    // Specification 10 §4.3: a cycle that ends after the next nominal start makes the next cycle
    // start at once, on the last boundary of the grid at or before its start. On a 10 ms grid the
    // cycle of 10 ms runs until 35 ms, past the boundaries of 20 and 30 ms: the next cycle starts
    // at once at 35 ms on the boundary of 30 ms, and the grid goes on at 40, 50 and 60 ms, where a
    // 10 ms task is due in every cycle and counts the activation of 20 ms as missed. Catches a grid
    // that goes on from the moment the overrun ended (45, 55 and 65 ms), which the following cycles
    // reach a little after their nominal starts, off the task's deadlines, so that one that wakes
    // just before a deadline finds the task not yet due yet counts as a scan (the macOS runs of
    // `computer_stopped_keeps_runtime_up.rs` and `runner_transcript_parity.rs`, 27 October 2026).
    let clock = ExactClock::default();
    let moved = clock.clone();
    let starts = six_cycle_starts(clock, move |cycle| {
        if cycle == 2 {
            *recover_mutex_lock(moved.0.lock()) = 35_000_000;
        }
    });
    assert_eq!(starts, [0, 10, 35, 40, 50, 60]);
    assert_eq!(task_due_in_each_later_cycle(&starts), (vec![true; 5], 1));
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
fn an_automatic_restart_begins_the_tasks_timing_when_its_first_cycle_starts() {
    // Specification 10 §4.3: a recovery from a fault by a restart begins the tasks' timing, and the
    // grid with it, at the moment its first cycle starts. Every sleep wakes 1 ms late. The first
    // three cycles start at 0, 11 and 21 ms, and the third fails; the fault policy restarts the
    // program at 21 ms and waits its backoff of one cycle interval (specification 11 §6.6), until
    // 32 ms. The first cycle after the restart starts at 32 ms, where the timing begins, so the
    // next nominal starts are 42 and 52 ms, the late wakes start those cycles at 43 and 53 ms, and
    // a 10 ms task is not yet due in the first cycle and due in the two after it. Catches a restart
    // that begins the timing when it ends, at 21 ms, before its backoff: the grid of 21 ms puts
    // those cycles at 42 and 52 ms, and its task runs in the first cycle, 11 ms after the timing
    // began (the Windows run of `runner_transcript_parity.rs`, 29 October 2026, where the
    // operating system held the scan thread between a restart's end and its first cycle).
    let mut runtime = crate::harness::TestHarness::from_source("PROGRAM Main\nEND_PROGRAM\n")
        .expect("build the runner fixture")
        .into_runtime();
    runtime.set_fault_policy(crate::watchdog::FaultPolicy::Restart);
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
            if starts.len() == 3 {
                return Err(RuntimeError::DivisionByZero);
            }
            Ok(())
        },
    );
    let starts = recover_mutex_lock(starts.lock()).clone();
    assert_eq!(starts, [0, 11, 21, 32, 43, 53]);
    assert_eq!(
        task_due_in_each_cycle(32, &starts[3..]),
        (vec![false, true, true], 0)
    );
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
