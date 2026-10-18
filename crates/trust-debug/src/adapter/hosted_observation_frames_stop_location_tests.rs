//! Paused hosted compound statements: a breakpoint stops at the statement it names, not at the
//! compound statement around it (specification 13, "Breakpoint evaluation, source resolution, and
//! hook contract").
use super::*;
use trust_runtime::debug::DebugStop;
use trust_runtime::harness::{CompileSession, SourceFile};

const STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(10);
const SCAN_POLL: std::time::Duration = std::time::Duration::from_millis(10);

const IF_SOURCE: &str = "FUNCTION Pick : DINT\nVAR_INPUT flag : BOOL; END_VAR\nVAR n : DINT; END_VAR\nn := DINT#1;\nIF flag THEN\n    n := DINT#7;\nEND_IF;\nPick := n;\nEND_FUNCTION\nPROGRAM Main\nVAR result : DINT; END_VAR\nresult := Pick(TRUE);\nEND_PROGRAM\n";

const WHILE_SOURCE: &str = "FUNCTION Count : DINT\nVAR_INPUT limit : DINT; END_VAR\nVAR k : DINT; END_VAR\nk := DINT#0;\nWHILE k < limit DO\n    k := k + DINT#1;\nEND_WHILE;\nCount := k;\nEND_FUNCTION\nPROGRAM Main\nVAR result : DINT; END_VAR\nresult := Count(DINT#2);\nEND_PROGRAM\n";

const CASE_SOURCE: &str = "FUNCTION Pick : DINT\nVAR_INPUT sel : DINT; END_VAR\nVAR n : DINT; END_VAR\nn := DINT#0;\nCASE sel OF\n    1: n := DINT#10;\n    2: n := DINT#20;\nELSE\n    n := DINT#30;\nEND_CASE;\nPick := n;\nEND_FUNCTION\nPROGRAM Main\nVAR result : DINT; END_VAR\nresult := Pick(DINT#2);\nEND_PROGRAM\n";

const FOR_SOURCE: &str = "FUNCTION Scan : DINT\nVAR_INPUT seed : DINT; END_VAR\nVAR i : DINT; END_VAR\nScan := seed;\nFOR i := DINT#1 TO DINT#5 BY DINT#2 DO\n    Scan := Scan + i;\nEND_FOR;\nEND_FUNCTION\nPROGRAM Main\nVAR result : DINT; END_VAR\nresult := Scan(DINT#3);\nEND_PROGRAM\n";

/// Runs `source` with one breakpoint on the first line that contains `needle`. While the scan is
/// paused, `inspect` gets the control and the stop receiver and returns its value. The scan then
/// runs to its end with the breakpoint still set. The second value holds the stops the scan emitted
/// after `inspect` returned: none, when the breakpoint stops once per scan (spec 13).
fn with_breakpoint_on<T>(
    source: &str,
    needle: &str,
    inspect: impl FnOnce(&DebugControl, &std::sync::mpsc::Receiver<DebugStop>) -> T,
) -> (T, Vec<DebugStop>) {
    let runtime = CompileSession::from_sources(vec![SourceFile::with_path("main.st", source)])
        .build_runtime()
        .unwrap();
    let mut session = DebugSession::new(runtime);
    session.register_source("main.st", 0, source);
    let mut adapter = DebugAdapter::new(session);
    let control = adapter.session().debug_control();
    let line = source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap() as u32
        + 1;
    let outcome = adapter.dispatch_request(Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setBreakpoints".into(),
        arguments: Some(
            serde_json::to_value(SetBreakpointsArguments {
                source: Source {
                    name: Some("main.st".into()),
                    path: Some("main.st".into()),
                    source_reference: None,
                },
                breakpoints: Some(vec![SourceBreakpoint {
                    line,
                    column: Some(1),
                    condition: None,
                    hit_condition: None,
                    log_message: None,
                }]),
                lines: None,
                source_modified: None,
            })
            .unwrap(),
        ),
    });
    let response: Response<SetBreakpointsResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.body.unwrap().breakpoints[0].verified);
    let (sender, receiver) = std::sync::mpsc::channel();
    control.set_stop_sender(sender);
    let runtime_handle = adapter.session().runtime_handle();
    let executing = Arc::clone(&runtime_handle);
    let thread = std::thread::spawn(move || executing.lock().unwrap().execute_cycle());

    // Release the scan thread even if an assertion in `inspect` fails while it is paused.
    let inspected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        inspect(&control, &receiver)
    }));
    control.continue_run();
    // A further stop pauses the scan again: record it and resume, until the scan has finished. A
    // stop that should not come is then reported, instead of blocking the test.
    let mut later = Vec::new();
    while !thread.is_finished() {
        if let Ok(stop) = receiver.recv_timeout(SCAN_POLL) {
            later.push(stop);
            control.continue_run();
        }
    }
    thread.join().unwrap().unwrap();
    later.extend(receiver.try_iter());
    match inspected {
        Ok(value) => (value, later),
        Err(error) => std::panic::resume_unwind(error),
    }
}

/// The source text of the statement a stop reports, without its surrounding whitespace.
fn stopped_statement<'a>(source: &'a str, stop: &DebugStop) -> Option<&'a str> {
    stop.location
        .as_ref()
        .and_then(|location| source.get(location.start as usize..location.end as usize))
        .map(str::trim)
}

/// Spec 13 "Breakpoint evaluation, source resolution, and hook contract": a breakpoint on a body
/// statement stops at that statement. It catches a stop on the IF statement instead, which the old
/// overlap rule reached first, since the IF statement's span covers the body.
#[test]
fn a_breakpoint_in_an_if_body_stops_in_the_body() {
    with_breakpoint_on(IF_SOURCE, "n := DINT#7;", |control, receiver| {
        let stop = receiver
            .recv_timeout(STOP_WAIT)
            .expect("breakpoint stop inside the IF body");
        assert_eq!(stop.reason, DebugStopReason::Breakpoint);
        assert_eq!(stopped_statement(IF_SOURCE, &stop), Some("n := DINT#7;"));
        let snapshot = control.snapshot().expect("paused detached projection");
        let frame = snapshot
            .storage
            .frames()
            .iter()
            .find(|frame| frame.owner.eq_ignore_ascii_case("Pick"))
            .unwrap();
        // `n := DINT#1;` ran before the IF, and `flag` is the TRUE argument of `Pick(TRUE)`.
        assert_eq!(frame.variables.get("n"), Some(&RuntimeValue::DInt(1)));
        assert_eq!(frame.variables.get("flag"), Some(&RuntimeValue::Bool(true)));
    });
}

/// Spec 13 "Breakpoint evaluation, source resolution, and hook contract": a breakpoint on a WHILE
/// body statement stops at that statement before its first pass changes `k`. It catches a stop on
/// the WHILE statement instead.
#[test]
fn a_breakpoint_in_a_while_body_stops_in_the_body() {
    with_breakpoint_on(WHILE_SOURCE, "k := k + DINT#1;", |control, receiver| {
        let stop = receiver
            .recv_timeout(STOP_WAIT)
            .expect("breakpoint stop inside the WHILE body");
        assert_eq!(stop.reason, DebugStopReason::Breakpoint);
        assert_eq!(
            stopped_statement(WHILE_SOURCE, &stop),
            Some("k := k + DINT#1;")
        );
        let snapshot = control.snapshot().expect("paused detached projection");
        let frame = snapshot
            .storage
            .frames()
            .iter()
            .find(|frame| frame.owner.eq_ignore_ascii_case("Count"))
            .unwrap();
        // `k := DINT#0;` ran before the loop, and `limit` is the DINT#2 argument.
        assert_eq!(frame.variables.get("k"), Some(&RuntimeValue::DInt(0)));
        assert_eq!(frame.variables.get("limit"), Some(&RuntimeValue::DInt(2)));
    });
}

/// Spec 13 "Breakpoint evaluation, source resolution, and hook contract": a breakpoint on the CASE
/// arm that runs stops at that arm's statement. It catches a stop on the CASE statement instead.
#[test]
fn a_breakpoint_in_a_case_body_stops_in_the_body() {
    with_breakpoint_on(CASE_SOURCE, "n := DINT#20;", |control, receiver| {
        let stop = receiver
            .recv_timeout(STOP_WAIT)
            .expect("breakpoint stop inside the CASE arm");
        assert_eq!(stop.reason, DebugStopReason::Breakpoint);
        assert_eq!(stopped_statement(CASE_SOURCE, &stop), Some("n := DINT#20;"));
        let snapshot = control.snapshot().expect("paused detached projection");
        let frame = snapshot
            .storage
            .frames()
            .iter()
            .find(|frame| frame.owner.eq_ignore_ascii_case("Pick"))
            .unwrap();
        // `n := DINT#0;` ran before the CASE, and `sel` is the DINT#2 argument of `Pick(DINT#2)`.
        assert_eq!(frame.variables.get("n"), Some(&RuntimeValue::DInt(0)));
        assert_eq!(frame.variables.get("sel"), Some(&RuntimeValue::DInt(2)));
    });
}

/// Spec 13 "Breakpoint evaluation, source resolution, and hook contract": a breakpoint on the FOR
/// line still stops at the FOR statement, before the header stores its control value and its
/// temporaries. It catches a FOR line that is skipped, and a header stop that shows the header's
/// stores (`i` = DINT#1, end DINT#5, step DINT#2) before they run.
#[test]
fn a_breakpoint_on_a_for_line_stops_at_the_for_statement() {
    with_breakpoint_on(
        FOR_SOURCE,
        "FOR i := DINT#1 TO DINT#5",
        |control, receiver| {
            let stop = receiver
                .recv_timeout(STOP_WAIT)
                .expect("breakpoint stop on the FOR line");
            assert_eq!(stop.reason, DebugStopReason::Breakpoint);
            assert!(stopped_statement(FOR_SOURCE, &stop)
                .is_some_and(|text| text.starts_with("FOR i := DINT#1 TO DINT#5")));
            let snapshot = control.snapshot().expect("paused detached projection");
            let frame = snapshot
                .storage
                .frames()
                .iter()
                .find(|frame| frame.owner.eq_ignore_ascii_case("Scan"))
                .unwrap();
            // Before the header runs, `i` holds its declared default and the temporaries hold no value.
            assert_eq!(frame.variables.get("i"), Some(&RuntimeValue::DInt(0)));
            assert_eq!(frame.variables.get("@scratch/3"), Some(&RuntimeValue::Null));
            assert_eq!(frame.variables.get("@scratch/4"), Some(&RuntimeValue::Null));
        },
    );
}

/// Spec 13, the IF-line paragraph and the cycle guard: a breakpoint on an IF line resolves to the IF
/// statement. Its body statements start inside that span and match the breakpoint too, but the guard
/// allows one stop per breakpoint per scan, at the first matching statement, which is the IF
/// statement. It catches a second stop in that scan, at the body statement.
#[test]
fn a_breakpoint_on_an_if_line_stops_once_per_scan_at_the_if_statement() {
    let (first, later) = with_breakpoint_on(IF_SOURCE, "IF flag THEN", |_, receiver| {
        receiver
            .recv_timeout(STOP_WAIT)
            .expect("breakpoint stop on the IF line")
    });
    assert_eq!(first.reason, DebugStopReason::Breakpoint);
    assert!(
        stopped_statement(IF_SOURCE, &first).is_some_and(|text| text.starts_with("IF flag THEN"))
    );
    assert!(
        later.is_empty(),
        "the scan stopped again after the IF statement: {later:?}"
    );
}
