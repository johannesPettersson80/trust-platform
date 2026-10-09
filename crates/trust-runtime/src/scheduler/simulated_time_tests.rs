//! Simulation-time rules of the resource loop (specification 33, §6.1), driven by a deterministic host clock.
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
        if resume_now {
            if let Some((_, sender)) = state.resume_after_sleeps.take() {
                let _ = sender.send(ResourceCommand::Resume);
            }
        }
    }

    fn wake(&self) {}
}

/// Runs the resource loop until `cycles` cycles have run, then stops it with a fault. `on_cycle` receives the
/// index of each cycle and the runtime, whose process time the cycle sees.
fn run_cycles(
    runner: ResourceRunner<HostClock>,
    cycles: usize,
    mut on_cycle: impl FnMut(usize, &mut Runtime),
) {
    let mut run = 0_usize;
    run_resource_loop_core(
        runner,
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(ResourceState::Boot)),
        Arc::new(Mutex::new(None)),
        move |runtime| {
            on_cycle(run, runtime);
            run += 1;
            if run == cycles {
                return Err(RuntimeError::SimulationFault("test stop".into()));
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
        Runtime::new(),
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

#[test]
fn pausing_holds_simulation_time_and_resuming_continues_without_a_jump() {
    let host = HostClock::with_late_ms(0);
    let (command_tx, command_rx) = std::sync::mpsc::channel();
    let mut runner = ResourceRunner::new(Runtime::new(), host.clone(), Duration::from_millis(10))
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
    let runner = ResourceRunner::new(Runtime::new(), host.clone(), Duration::from_millis(10))
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
    let deadlines = host.deadlines();
    assert_eq!(
        deadlines[..5].to_vec(),
        millis(&[10, 20, 30, 40, 50]),
        "cycles before the change wait one cycle interval of 10 ms"
    );
    assert_eq!(
        deadlines[5],
        51 * NANOS_PER_MS,
        "the schedule restarts at the next cycle, which then waits 1 ms at speed 10"
    );
    for pair in deadlines[5..].windows(2) {
        assert_eq!(
            pair[1] - pair[0],
            NANOS_PER_MS,
            "every later cycle waits one paced interval of 1 ms"
        );
    }
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
        Runtime::new(),
        HostClock::with_late_ms(5),
        Duration::from_millis(10),
    )
    .with_simulation_speed(SimulationSpeed::new(10), Some(Arc::clone(&sink)));
    let reported = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&reported);
    let reader = Arc::clone(&sink);
    run_cycles(runner, 250, move |_, _| {
        let value = reader
            .lock()
            .expect("metrics")
            .simulation_achieved_time_scale();
        recorded.lock().expect("reported speeds").push(value);
    });
    let reported = reported.lock().expect("reported speeds");
    // Cycle n starts 6 n ms into wall time and advances simulation time by 10 ms, so the rate is 10 / 6.
    assert_eq!(
        reported[166], None,
        "996 ms of wall time is not yet one second"
    );
    let steady = reported[167].expect("one second of wall time is measured");
    assert!(
        (steady - 10.0 / 6.0).abs() < 1e-9,
        "achieved speed {steady}"
    );
    assert!(reported[249].is_some());
}

#[test]
fn without_simulation_process_time_follows_the_wall_clock_and_late_cycles_keep_the_grid() {
    let runner = ResourceRunner::new(
        Runtime::new(),
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
