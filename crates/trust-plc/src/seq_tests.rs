//! The wait points' runtime of a `#[trust_sequence]` (specification 48 RU-72), worked by hand on a
//! simulated task's logical time.

use crate::seq::{step, Waiting};
use crate::test::{advance, task};
use crate::{FaultCode, Str, Time};

const TICK: Time = Time::from_nanos(100_000_000);

#[test]
fn a_wait_counts_from_the_activation_that_began_it() {
    // A 100 ms task begins a wait at its activation at 200 ms: waited 0 then, 100 ms at the next
    // activation. wait(500 ms) is over at 700 ms, the fifth activation after the one that began
    // it, and not at 600 ms. Catches a wait that counts from the task's start, or that is over a
    // cycle early.
    let mut cx = task(TICK);
    advance(&mut cx);
    advance(&mut cx);
    let mut waiting = Waiting::default();
    waiting.begin(&cx, 3);
    assert_eq!(
        (waiting.point, waiting.began),
        (3, Time::from_nanos(200_000_000))
    );
    assert_eq!(waiting.waited(&cx), Time::from_nanos(0));
    let half = Time::from_nanos(500_000_000);
    let mut over = Vec::new();
    for _ in 0..5 {
        advance(&mut cx);
        over.push((
            cx.now().as_nanos() / 1_000_000,
            waiting.waited_for(&cx, half),
        ));
    }
    assert_eq!(
        over,
        [
            (300, false),
            (400, false),
            (500, false),
            (600, false),
            (700, true)
        ]
    );
}

#[test]
fn a_wait_that_outlasts_its_limit_faults_with_its_point_and_times() {
    // .within(2 s) on the wait point 2, begun at 0: at 2000 ms it has lasted exactly 2 s, not
    // longer; at 2100 ms it has, and the fault is RustFault with the point 2, the limit 2000 ms and
    // the time waited 2100 ms. Catches a limit that faults at the limit, and numbers that name
    // another wait point or another time.
    let mut raw = trust_plc_abi::Cx::new(&trust_plc_abi::kernel::WITHOUT_DEBUGGER);
    raw.interval_ns = 100_000_000;
    let mut cx = crate::__private::enter(&raw);
    let mut waiting = Waiting::default();
    waiting.begin(&cx, 2);
    let limit = Time::from_nanos(2_000_000_000);
    for _ in 0..20 {
        advance(&mut cx);
    }
    assert_eq!(
        waiting.outlasted(&mut cx, limit),
        None,
        "at the limit itself"
    );
    advance(&mut cx);
    let fault = waiting.outlasted(&mut cx, limit).expect("past the limit");
    let _ = crate::__private::leave(cx, &mut raw);
    assert_eq!(fault.raw(), FaultCode::RustFault as u32);
    assert_eq!(raw.fault.detail, [2, 2000, 2100]);
}

#[test]
fn a_time_that_went_back_waits_nothing() {
    // A logical time before the wait's start, as after a restart that set the clock back, gives a
    // wait of 0, not a negative one that would never end. Catches a subtraction that wraps.
    let mut cx = task(TICK);
    let mut waiting = Waiting::default();
    advance(&mut cx);
    waiting.begin(&cx, 1);
    let mut earlier = task(TICK);
    assert_eq!(waiting.waited(&earlier), Time::from_nanos(0));
    advance(&mut earlier);
    assert!(waiting.waited_for(&earlier, Time::from_nanos(0)));
}

#[test]
fn a_step_names_itself_in_its_output() {
    // RU-70: s.step(&mut self.step, "WAIT_PART") writes the step's name into the step output; a
    // name longer than the output keeps its first characters. Catches an output left as it was.
    let mut output: Str<8> = Str::new();
    step(&mut output, "GRIP");
    assert_eq!(output.as_str(), Some("GRIP"));
    step(&mut output, "WAIT_PART");
    assert_eq!(output.as_str(), Some("WAIT_PAR"));
}
