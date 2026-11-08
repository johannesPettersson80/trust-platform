//! Simulation-time rules of the resource loop (specification 46, §6.1), driven by a deterministic host clock.
//!
//! `ManualClock` blocks in `sleep_until` until another thread advances it, so these tests use a host clock
//! whose every wake-up returns late, the same pattern as the late-wake-up test in `runner_loop.rs`.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use super::simulation_clock::AchievedSpeedWindow;
use super::*;
use crate::harness::TestHarness;
use crate::metrics::RuntimeMetrics;
use crate::value::Duration;
use crate::RestartMode;

const NANOS_PER_MS: i64 = 1_000_000;
const RESTART_SOURCE: &str = "PROGRAM Main\nEND_PROGRAM\n";

/// A host whose every wake-up returns `late_nanos` after the instant it was asked for. Each requested deadline
/// is recorded, and the host can send `Resume` after a number of sleeps.
#[derive(Clone, Debug)]
struct HostClock {
    state: Arc<Mutex<HostState>>,
}

#[derive(Debug)]
struct HostState {
    now_nanos: i64,
    late_nanos: i64,
    deadlines: Vec<i64>,
    resume_after_sleeps: Option<(u32, Sender<ResourceCommand>)>,
}

impl HostClock {
    fn with_late_ms(late_ms: i64) -> Self {
        Self {
            state: Arc::new(Mutex::new(HostState {
                now_nanos: 0,
                late_nanos: late_ms * NANOS_PER_MS,
                deadlines: Vec::new(),
                resume_after_sleeps: None,
            })),
        }
    }

    fn deadlines(&self) -> Vec<i64> {
        self.state.lock().expect("host clock").deadlines.clone()
    }

    /// Sends `Resume` once `sleeps` more calls to `sleep_until` have been made.
    fn resume_after(&self, sleeps: u32, sender: Sender<ResourceCommand>) {
        self.state.lock().expect("host clock").resume_after_sleeps = Some((sleeps, sender));
    }
}

impl Clock for HostClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(self.state.lock().expect("host clock").now_nanos)
    }

    fn sleep_until(&self, deadline: Duration) {
        let mut state = self.state.lock().expect("host clock");
        let requested = deadline.as_nanos();
        state.deadlines.push(requested);
        let next = state.now_nanos.max(requested) + state.late_nanos;
        state.now_nanos = next;
        let resume_now = match state.resume_after_sleeps.as_mut() {
            Some((remaining, _)) => {
                *remaining = remaining.saturating_sub(1);
                *remaining == 0
            }
            None => false,
        };
        if resume_now && let Some((_, sender)) = state.resume_after_sleeps.take() {
            let _ = sender.send(ResourceCommand::Resume);
        }
    }

    fn wake(&self) {}
}

/// Runs the resource loop until `cycles` cycles have run, then stops it as STOP does: a faulted
/// resource keeps serving until it is stopped. `on_cycle` receives the index of each cycle and the
/// runtime, whose process time the cycle sees.
fn run_cycles(
    runner: ResourceRunner<HostClock>,
    cycles: usize,
    mut on_cycle: impl FnMut(usize, &mut crate::CompiledRuntime),
) {
    let mut run = 0_usize;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_after = Arc::clone(&stop);
    let state = Arc::new(Mutex::new(ResourceState::Boot));
    let last_error = Arc::new(Mutex::new(None));
    let supervision = Arc::new(super::supervisor::ScanSupervision::new(
        Arc::clone(&state),
        Arc::clone(&last_error),
    ));
    run_resource_loop_core(
        runner,
        stop,
        state,
        last_error,
        supervision,
        move |runtime| {
            on_cycle(run, runtime);
            run += 1;
            if run == cycles {
                stop_after.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            Ok(())
        },
    );
}

fn millis(values: &[i64]) -> Vec<i64> {
    values.iter().map(|ms| ms * NANOS_PER_MS).collect()
}

#[test]
fn late_simulated_cycles_advance_one_interval_each_and_none_are_skipped() {
    let runner = ResourceRunner::new(
        crate::runtime::empty_runtime(),
        HostClock::with_late_ms(5),
        Duration::from_millis(10),
    )
    .with_simulation_speed(SimulationSpeed::new(10), None);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    run_cycles(runner, 50, move |_, runtime| {
        recorded
            .lock()
            .expect("cycle times")
            .push(runtime.current_time().as_nanos());
    });
    let expected: Vec<i64> = (0_i64..50).map(|cycle| cycle * 10 * NANOS_PER_MS).collect();
    assert_eq!(*seen.lock().expect("cycle times"), expected);
}

/// Specification 46 §6.1 item 1 (the CODESYS toolkit's B9): simulation time is never read from the wall clock, so
/// the first cycle runs at simulation time zero however long the runtime took to start. Here the host clock reads
/// 180 ms when the loop starts, at speed 10; the cycles of a 10 ms resource see 0, 10 and 20 ms. Catches a first
/// cycle at the scaled wall clock, 1,800 ms here, whose distance from time zero the tasks counted as missed
/// intervals right after the start (the toolkit saw 17 of a 100 ms task).
#[test]
fn the_first_simulated_cycle_runs_at_time_zero_however_long_the_start_took() {
    let host = HostClock::with_late_ms(0);
    host.state.lock().expect("host clock").now_nanos = 180 * NANOS_PER_MS;
    let runner = ResourceRunner::new(
        crate::runtime::empty_runtime(),
        host,
        Duration::from_millis(10),
    )
    .with_simulation_speed(SimulationSpeed::new(10), None);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    run_cycles(runner, 3, move |_, runtime| {
        recorded
            .lock()
            .expect("cycle times")
            .push(runtime.current_time().as_nanos());
    });
    assert_eq!(*seen.lock().expect("cycle times"), millis(&[0, 10, 20]));
}

#[test]
fn pausing_holds_simulation_time_and_resuming_continues_without_a_jump() {
    let host = HostClock::with_late_ms(0);
    let (command_tx, command_rx) = std::sync::mpsc::channel();
    let mut runner = ResourceRunner::new(
        crate::runtime::empty_runtime(),
        host.clone(),
        Duration::from_millis(10),
    )
    .with_simulation_speed(SimulationSpeed::new(1), None);
    runner.command_rx = Some(command_rx);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    run_cycles(runner, 6, move |cycle, runtime| {
        recorded
            .lock()
            .expect("cycle times")
            .push(runtime.current_time().as_nanos());
        if cycle == 2 {
            command_tx
                .send(ResourceCommand::Pause)
                .expect("pause command");
            host.resume_after(3, command_tx.clone());
        }
    });
    let expected: Vec<i64> = (0_i64..6).map(|cycle| cycle * 10 * NANOS_PER_MS).collect();
    assert_eq!(
        *seen.lock().expect("cycle times"),
        expected,
        "a paused simulation holds its time, and the cycle after resume continues from it"
    );
}

#[test]
fn a_speed_change_repaces_the_next_cycles_and_never_moves_simulation_time() {
    let host = HostClock::with_late_ms(0);
    let speed = SimulationSpeed::new(1);
    let runner = ResourceRunner::new(
        crate::runtime::empty_runtime(),
        host.clone(),
        Duration::from_millis(10),
    )
    .with_simulation_speed(speed.clone(), None);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    run_cycles(runner, 12, move |cycle, runtime| {
        recorded
            .lock()
            .expect("cycle times")
            .push(runtime.current_time().as_nanos());
        if cycle == 4 {
            speed.set(10);
        }
    });
    let expected: Vec<i64> = (0_i64..12).map(|cycle| cycle * 10 * NANOS_PER_MS).collect();
    assert_eq!(
        *seen.lock().expect("cycle times"),
        expected,
        "a speed change must not move simulation time"
    );
    // Specification 46 §6.1 item 2: at speed 10 a cycle is due every 1 ms, below the 2 ms wait floor, so the scheduler
    // waits only every second cycle.
    let deadlines = host.deadlines();
    assert_eq!(
        deadlines[..5].to_vec(),
        millis(&[10, 20, 30, 40, 50]),
        "cycles before the change wait one cycle interval of 10 ms"
    );
    assert_eq!(
        deadlines[5],
        52 * NANOS_PER_MS,
        "the schedule restarts at the next cycle, due at 51 ms, which runs at once; the wait is the cycle after's, for 52 ms"
    );
    for pair in deadlines[5..].windows(2) {
        assert_eq!(
            pair[1] - pair[0],
            2 * NANOS_PER_MS,
            "every later wait covers two paced intervals of 1 ms"
        );
    }
    assert_eq!(
        deadlines.len(),
        8,
        "twelve cycles wait five times at speed 1 and three times at speed 10"
    );
}

/// Specification 46 §6.1 item 2 (the CODESYS toolkit's B18): the scheduler waits only for a due time at least the 2 ms
/// wait floor ahead, and a cycle within that floor of its due time keeps the schedule, so a wait that wakes late costs no
/// speed. Worked by hand for a 10 ms resource at speed 100, a cycle due every 0.1 ms, on a host whose waits wake 1 ms
/// late, as Windows' do: the first 20 cycles run at once and the scheduler waits for 2 ms, waking at 3 ms; each later
/// wait comes 30 cycles on, for 3 ms of schedule, and wakes 1 ms late. After 1,000 cycles, 9.99 s of simulation time,
/// the wall clock reads 99 ms after 33 waits. Catches a wait for every cycle, each waking 1 ms late and restarting the
/// schedule: 1,000 waits and 1.1 s, a speed of 9 instead of 100.
#[test]
fn a_simulation_paced_below_the_wait_floor_runs_due_cycles_back_to_back_and_keeps_its_speed() {
    let host = HostClock::with_late_ms(1);
    let runner = ResourceRunner::new(
        crate::runtime::empty_runtime(),
        host.clone(),
        Duration::from_millis(10),
    )
    .with_simulation_speed(SimulationSpeed::new(100), None);
    let last = Arc::new(Mutex::new(None));
    let recorded = Arc::clone(&last);
    run_cycles(runner, 1000, move |_, runtime| {
        *recorded.lock().expect("cycle time") = Some(runtime.current_time().as_nanos());
    });
    assert_eq!(
        *last.lock().expect("cycle time"),
        Some(9_990 * NANOS_PER_MS),
        "the last cycle runs at 9.99 s of simulation time"
    );
    assert_eq!(host.deadlines().len(), 33);
    assert_eq!(
        host.state.lock().expect("host clock").now_nanos,
        99 * NANOS_PER_MS
    );
}

#[test]
fn a_warm_restart_continues_simulation_time_from_its_last_value() {
    let harness_runtime = TestHarness::from_source(RESTART_SOURCE)
        .expect("restart fixture")
        .into_runtime();
    let restart = Arc::new(Mutex::new(None));
    let runner = ResourceRunner::new(
        harness_runtime,
        HostClock::with_late_ms(0),
        Duration::from_millis(10),
    )
    .with_restart_signal(Arc::clone(&restart))
    .with_simulation_speed(SimulationSpeed::new(10), None);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    run_cycles(runner, 6, move |cycle, runtime| {
        recorded
            .lock()
            .expect("cycle times")
            .push(runtime.current_time().as_nanos());
        if cycle == 2 {
            *restart.lock().expect("restart request") = Some(RestartMode::Warm);
        }
    });
    let expected: Vec<i64> = (0_i64..6).map(|cycle| cycle * 10 * NANOS_PER_MS).collect();
    assert_eq!(
        *seen.lock().expect("cycle times"),
        expected,
        "the cycle after a warm restart sees the simulation time that the restart continues from"
    );
}

#[test]
fn achieved_speed_reports_the_rate_of_a_host_that_cannot_keep_up() {
    let sink = Arc::new(Mutex::new(RuntimeMetrics::new()));
    let runner = ResourceRunner::new(
        crate::runtime::empty_runtime(),
        HostClock::with_late_ms(5),
        Duration::from_millis(10),
    )
    .with_simulation_speed(SimulationSpeed::new(10), Some(Arc::clone(&sink)));
    let reported = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&reported);
    let reader = Arc::clone(&sink);
    run_cycles(runner, 300, move |_, _| {
        let value = reader
            .lock()
            .expect("metrics")
            .simulation_achieved_time_scale();
        recorded.lock().expect("reported speeds").push(value);
    });
    let reported = reported.lock().expect("reported speeds");
    // Specification 46 §6.1 items 2 and 3. A cycle is due every 1 ms, below the 2 ms wait floor, so the scheduler waits
    // at every second cycle, for 2 ms ahead, and wakes 5 ms late, beyond the floor, which restarts the schedule: cycles
    // 2k and 2k + 1 start 7 k ms into wall time, and every 7 ms advance simulation time by 20 ms.
    assert_eq!(
        reported[285], None,
        "994 ms of wall time is not yet one second"
    );
    let steady = reported[286].expect("one second of wall time is measured");
    assert!(
        (steady - 20.0 / 7.0).abs() < 1e-9,
        "achieved speed {steady}: 2,860 ms of simulation time in 1,001 ms"
    );
    assert!(reported[299].is_some());
}

#[test]
fn without_simulation_process_time_follows_the_wall_clock_and_late_cycles_keep_the_grid() {
    let runner = ResourceRunner::new(
        crate::runtime::empty_runtime(),
        HostClock::with_late_ms(5),
        Duration::from_millis(10),
    );
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    run_cycles(runner, 4, move |_, runtime| {
        recorded
            .lock()
            .expect("cycle times")
            .push(runtime.current_time().as_nanos());
    });
    assert_eq!(
        *seen.lock().expect("cycle times"),
        millis(&[0, 15, 25, 35]),
        "each cycle sees the wall clock; a 5 ms late wake-up keeps the 10 ms grid"
    );
}

#[test]
fn achieved_speed_is_null_until_one_second_of_wall_time_is_measured() {
    let mut window = AchievedSpeedWindow::default();
    let mut last = None;
    for cycle in 0_i64..=100 {
        last = window.record(cycle * 10 * NANOS_PER_MS, cycle * 10 * NANOS_PER_MS);
        if cycle == 99 {
            assert_eq!(last, None, "990 ms of wall time is not yet a measurement");
        }
    }
    assert_eq!(last, Some(1.0));
}

#[test]
fn achieved_speed_follows_only_the_last_five_seconds() {
    let mut window = AchievedSpeedWindow::default();
    let mut wall = 0_i64;
    let mut simulation = 0_i64;
    let mut last = None;
    // Ten seconds at speed 1, then ten seconds at speed 2, in 10 ms cycles.
    for step in 0_i64..2000 {
        last = window.record(wall, simulation);
        let rate: i64 = if step < 1000 { 1 } else { 2 };
        wall += 10 * NANOS_PER_MS;
        simulation += rate * 10 * NANOS_PER_MS;
    }
    let value = last.expect("measured");
    assert!(
        (value - 2.0).abs() < 1e-9,
        "the last five seconds run at speed 2, got {value}"
    );
}

#[test]
fn clearing_the_window_starts_a_new_measurement() {
    let mut window = AchievedSpeedWindow::default();
    for cycle in 0_i64..=100 {
        window.record(cycle * 10 * NANOS_PER_MS, cycle * 10 * NANOS_PER_MS);
    }
    window.clear();
    assert_eq!(
        window.record(2_000 * NANOS_PER_MS, 2_000 * NANOS_PER_MS),
        None,
        "a pause, a restart or a speed change starts a new measurement"
    );
}

#[test]
fn simulation_speed_accepts_one_to_one_hundred_and_names_the_range_otherwise() {
    assert_eq!(validate_time_scale(1), Ok(1));
    assert_eq!(validate_time_scale(100), Ok(100));
    for value in [0_i64, 101, -1, i64::from(u32::MAX) + 1] {
        let message = validate_time_scale(value).expect_err("outside the permitted range");
        assert!(message.contains(">= 1 and <= 100"), "{message}");
    }
}

#[test]
fn simulation_warning_states_the_speed_in_force() {
    assert!(simulation_warning(7).contains("time scale x7"));
    assert!(simulation_warning(1).contains("Not for live hardware"));
}
