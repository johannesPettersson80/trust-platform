//! A resource on a simulator's clock (specification 11 §6.8.1, "Simulator for machine tests";
//! specification 48 RU-58): its cycles run only as the test allows them, at the logical times of its
//! cycle interval, and it answers reads while its time is held.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ResourceRunner, ResourceState, StdClock, SteppedClock};
use trust_runtime::value::{Duration, Value};
use trust_runtime_services::simulator_advance::{Advance, Advanced};
use trust_runtime_services::variables::ReadRefusal;

const COUNTER: &str = r#"
PROGRAM Main
VAR
    counter : DINT := 0;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;

#[test]
fn a_simulators_resource_runs_its_cycles_as_the_test_allows_them() {
    // The counter counts the cycles of a resource whose cycle interval is 10 ms. Worked by hand:
    // before the first advance no cycle has run, so a read is refused, since a read takes the
    // values the last completed scan left (specification 11 §6.8.1, "Reading variables"); three
    // cycles run at 0, 10 and 20 ms, so the counter is 3 after the scan at 20 ms; 100 ms of wall
    // time later it still is, read while the time is held; 25 ms more hold the cycles at 30 and
    // 40 ms and end at 45 ms, so the counter is 5 after the scan at 40 ms. Catches a resource that
    // runs on the wall clock, a first cycle before the test asks, a read that waits for a cycle or
    // adds one, and a duration that runs a cycle past its end.
    let runtime = TestHarness::from_source(COUNTER).unwrap().into_runtime();
    let clock = SteppedClock::new();
    let runner = ResourceRunner::new(
        runtime,
        StdClock::stepped(clock.clone()),
        Duration::from_millis(10),
    );
    let mut handle = runner.spawn("simulator").unwrap();
    let control = handle.control();
    let read = || {
        let read = control
            .read_variables(&["Main.counter".to_string()])
            .unwrap();
        (read.time_ns, read.values[0].1.clone())
    };
    let runs = || control.state() == ResourceState::Running;

    assert!(matches!(
        control.read_variables(&["Main.counter".to_string()]),
        Err(ReadRefusal::NoCompletedScan)
    ));

    let advanced = clock.advance(Advance::Cycles(3), &runs).unwrap();
    assert_eq!(
        advanced,
        Advanced {
            time: Duration::from_millis(20),
            cycles: 3
        }
    );
    assert_eq!(read(), (20_000_000, Value::DInt(3)));

    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(read(), (20_000_000, Value::DInt(3)));

    let advanced = clock
        .advance(Advance::Duration(Duration::from_millis(25)), &runs)
        .unwrap();
    assert_eq!(
        advanced,
        Advanced {
            time: Duration::from_millis(45),
            cycles: 2
        }
    );
    assert_eq!(read(), (40_000_000, Value::DInt(5)));

    handle.stop();
    handle.join().unwrap();
}
