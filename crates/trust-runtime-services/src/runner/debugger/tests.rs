//! A breakpoint's condition and hit condition and a logpoint at a statement start (specification
//! 13, "Breakpoint evaluation, source resolution, and hook contract"; specification 11 §6.13.2), on
//! the debugger alone, with a stand-in for the names of the frame that runs the statement.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use core::marker::PhantomData;

use smol_str::SmolStr;
use trust_runtime_core::program_model::BinaryOp;

use super::*;
use crate::debug::logpoint::LogPart;

/// The names of a frame whose `count` holds a value.
struct Frame(Value);

impl Names for Frame {
    fn read(&self, name: &str) -> Option<Value> {
        name.eq_ignore_ascii_case("count").then(|| self.0.clone())
    }

    fn enumeration(&self, _type_name: &str, _value: &str) -> Option<Value> {
        None
    }
}

/// `count` holding `value`.
fn count(value: i16) -> Frame {
    Frame(Value::Int(value))
}

/// The statement on line 2 of file 0, from column 1 to 10, at call depth 1.
fn statement() -> StatementStart<'static> {
    StatementStart {
        depth: 1,
        span: StatementSpan {
            file_index: 0,
            start: (2, 1),
            end: (2, 10),
        },
        frame: Some(FrameId(1)),
        call: PhantomData,
    }
}

/// `count > 5`.
fn count_above_five() -> Expression {
    Expression::Binary(
        BinaryOp::Gt,
        Box::new(Expression::Name(SmolStr::new("count"))),
        Box::new(Expression::Literal(Value::Int(5))),
    )
}

/// A debugger that keeps `capacity` stops and messages, with one breakpoint on the statement and
/// `options`.
fn debugger(capacity: usize, options: BreakpointOptions) -> ChipDebug {
    let debug = ChipDebug::new(capacity);
    let generation = debug.replace_breakpoints(0, vec![(Span::new((2, 1), (2, 10)), options)]);
    assert_eq!(generation, 1);
    debug
}

/// The statement starts with `count` holding each value in turn, one scan each; the boundaries it
/// gives.
fn run(debug: &ChipDebug, counts: &[Option<i16>]) -> Vec<Boundary> {
    counts
        .iter()
        .map(|value| {
            debug.begin_cycle();
            let frame = value.map(count);
            let names = frame.as_ref().map(|frame| frame as &dyn Names);
            let boundary = debug
                .at_statement(&statement(), names)
                .expect("the state is free");
            if boundary == Boundary::Hold {
                debug.resume();
            }
            boundary
        })
        .collect()
}

#[test]
fn a_condition_stops_only_where_it_gives_true() {
    // Specification 13: a condition that is FALSE, that cannot be evaluated (no state, or a name
    // that names nothing) or that gives no BOOL does not stop; a TRUE one stops as a breakpoint
    // does. With `count > 5` and `count` 3, then no state, then 7: run, run, hold, and the one
    // stop is a breakpoint stop of generation 1. Catches a condition ignored (three stops), a
    // condition read as true without the state, and a stop that names no breakpoint generation.
    let debug = debugger(
        4,
        BreakpointOptions {
            condition: Some(count_above_five()),
            ..BreakpointOptions::default()
        },
    );
    assert_eq!(
        run(&debug, &[Some(3), None, Some(7)]),
        [Boundary::Run, Boundary::Run, Boundary::Hold]
    );
    let stops = debug.drain_stops();
    assert_eq!(stops.len(), 1);
    assert_eq!(stops[0].reason, StopReason::Breakpoint);
    assert_eq!(stops[0].breakpoint_generation, Some(1));

    let missing = debugger(
        4,
        BreakpointOptions {
            condition: Some(Expression::Name(SmolStr::new("missing"))),
            ..BreakpointOptions::default()
        },
    );
    assert_eq!(run(&missing, &[Some(7)]), [Boundary::Run]);
}

#[test]
fn the_hit_is_counted_before_the_condition_is_evaluated() {
    // Specification 13: the hit counter counts every matching start, the hit condition is applied
    // to the count, and only then the condition. With hit condition `= 2`, `count > 5` and
    // `count` 7, 3, 7: the first start is hit 1 (the hit condition fails), the second hit 2 (the
    // condition is FALSE), the third hit 3 (the hit condition fails), so nothing stops. A debugger
    // that counted only the starts whose condition held would count the third as hit 2 and stop
    // there.
    let debug = debugger(
        4,
        BreakpointOptions {
            hit_condition: Some(HitCondition::Equal(2)),
            condition: Some(count_above_five()),
            ..BreakpointOptions::default()
        },
    );
    assert_eq!(
        run(&debug, &[Some(7), Some(3), Some(7)]),
        [Boundary::Run, Boundary::Run, Boundary::Run]
    );
    assert_eq!(debug.drain_stops(), Vec::<StopRecord>::new());
}

#[test]
fn a_logpoint_records_its_message_at_every_hit_and_never_stops() {
    // Specification 13 and specification 11 §6.13.2: a logpoint never stops and the cycle
    // suppression never holds it back, so two starts in one scan give two messages; its message
    // is its parts with each expression's value as a logpoint shows it (`Int(7)`), and it carries
    // the statement and the thread. A third message in a buffer of two replaces the oldest, and
    // the buffer is empty after it is read. Catches a logpoint that stops, one suppressed after its
    // first message in a scan, a message without its statement, and a buffer without a bound.
    let debug = debugger(
        2,
        BreakpointOptions {
            log_message: Some(vec![
                LogPart::Text(String::from("count is ")),
                LogPart::Expression(Expression::Name(SmolStr::new("count"))),
            ]),
            ..BreakpointOptions::default()
        },
    );
    debug.begin_cycle();
    for value in [5, 6, 7] {
        let frame = count(value);
        assert_eq!(
            debug.at_statement(&statement(), Some(&frame)),
            Ok(Boundary::Run)
        );
    }
    assert_eq!(debug.drain_stops(), Vec::<StopRecord>::new());
    let logs = debug.drain_logs();
    let texts: Vec<&str> = logs.iter().map(|log| log.text.as_str()).collect();
    assert_eq!(texts, ["count is Int(6)", "count is Int(7)"]);
    assert_eq!(logs[0].statement, statement().span);
    assert_eq!(logs[0].thread, Some(1));
    assert_eq!(debug.drain_logs(), Vec::<LogRecord>::new());
}
