//! The simulator's clock (specification 11 §6.8.1, "Simulator for machine tests") with a stand-in
//! for the scan loop, which uses the clock as the resource's loop does: before each cycle it waits
//! until the clock allows the cycle's time, a wake makes it wait again, and its cycles come one
//! interval of 10 ms apart.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use trust_runtime_services::simulator_advance::{Advance, Advanced};

use super::SteppedClock;
use crate::value::Duration;

const INTERVAL_NS: i64 = 10_000_000;

struct Loop {
    clock: SteppedClock,
    cycles: Arc<Mutex<Vec<i64>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Loop {
    fn start(clock: &SteppedClock) -> Self {
        let cycles = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (clock, cycles, stop) = (clock.clone(), cycles.clone(), stop.clone());
            std::thread::spawn(move || {
                let mut next = Duration::ZERO;
                while !stop.load(Ordering::SeqCst) {
                    if !clock.may_run_at(next) {
                        clock.sleep_until(next);
                        continue;
                    }
                    cycles.lock().unwrap().push(clock.now().as_nanos());
                    next = Duration::from_nanos(next.as_nanos() + INTERVAL_NS);
                    clock.sleep_until(next);
                }
            })
        };
        Self {
            clock: clock.clone(),
            cycles,
            stop,
            thread: Some(thread),
        }
    }

    fn cycles(&self) -> Vec<i64> {
        self.cycles.lock().unwrap().clone()
    }
}

impl Drop for Loop {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.clock.wake();
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn ms(millis: i64) -> i64 {
    millis * 1_000_000
}

fn held_for_a_while() {
    std::thread::sleep(std::time::Duration::from_millis(60));
}

#[test]
fn cycles_run_only_as_the_test_allows_them_one_interval_apart() {
    // The time starts at 0 and only the test moves it: no cycle runs before the first advance,
    // three cycles run at 0, 10 and 20 ms, the time is then 20 ms, and no cycle follows while the
    // time is held. Catches a clock that lets the loop run on its own, a first cycle before the
    // test asks, and a cycle at another time than its boundary.
    let clock = SteppedClock::new();
    let scan = Loop::start(&clock);
    held_for_a_while();
    assert_eq!(scan.cycles(), Vec::<i64>::new());

    let advanced = clock.advance(Advance::Cycles(3), &|| true).unwrap();
    assert_eq!(
        advanced,
        Advanced {
            time: Duration::from_millis(20),
            cycles: 3
        }
    );
    assert_eq!(scan.cycles(), [ms(0), ms(10), ms(20)]);

    held_for_a_while();
    assert_eq!(scan.cycles(), [ms(0), ms(10), ms(20)]);
    assert_eq!(clock.now(), Duration::from_millis(20));
}

#[test]
fn a_duration_runs_the_cycles_due_within_it_and_ends_at_its_end() {
    // 25 ms from 0 hold the cycles at 0, 10 and 20 ms, and the time ends at 25 ms; 5 ms more reach
    // the boundary at 30 ms, whose cycle runs, and end there. Catches a duration that runs the
    // cycle past its end, stops at the last cycle's time, or loses the cycle on its end.
    let clock = SteppedClock::new();
    let scan = Loop::start(&clock);
    let advanced = clock
        .advance(Advance::Duration(Duration::from_millis(25)), &|| true)
        .unwrap();
    assert_eq!(
        advanced,
        Advanced {
            time: Duration::from_millis(25),
            cycles: 3
        }
    );
    let advanced = clock
        .advance(Advance::Duration(Duration::from_millis(5)), &|| true)
        .unwrap();
    assert_eq!(
        advanced,
        Advanced {
            time: Duration::from_millis(30),
            cycles: 1
        }
    );
    assert_eq!(scan.cycles(), [ms(0), ms(10), ms(20), ms(30)]);
}

#[test]
fn a_wake_lets_the_loop_answer_without_a_cycle_or_a_move_of_the_time() {
    // A queued request wakes the loop, which answers it and waits again at the same boundary.
    // Catches a wake that runs a cycle or moves the time, which a test's read between two cycles
    // would then change.
    let clock = SteppedClock::new();
    let scan = Loop::start(&clock);
    clock.advance(Advance::Cycles(1), &|| true).unwrap();
    for _ in 0..5 {
        clock.wake();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    held_for_a_while();
    assert_eq!(scan.cycles(), [ms(0)]);
    assert_eq!(clock.now(), Duration::ZERO);
}

#[test]
fn an_advance_ends_when_the_resource_no_longer_runs() {
    // An advance of 5 cycles whose resource leaves running after its second cycle, as after a
    // halting fault, ends there: 2 cycles, at 0 and 10 ms. Catches an advance that runs on past a
    // fault.
    let clock = SteppedClock::new();
    let scan = Loop::start(&clock);
    let cycles = scan.cycles.clone();
    let runs = move || cycles.lock().unwrap().len() < 2;
    let advanced = clock.advance(Advance::Cycles(5), &runs).unwrap();
    assert_eq!(
        advanced,
        Advanced {
            time: Duration::from_millis(10),
            cycles: 2
        }
    );
    assert_eq!(scan.cycles(), [ms(0), ms(10)]);
}
