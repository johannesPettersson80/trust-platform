//! Paused hosted FOR loops: the scratch rows of the published snapshot hold the loop's temporaries.
use super::*;
use trust_runtime::harness::{CompileSession, SourceFile};

/// Spec 13 "Breakpoints" (a breakpoint resolves to the first statement at or after its line, here
/// `Scan := Scan + i;`) and spec 13 "Variables / Evaluate" (a scratch row holds the value the running
/// code stored). At that body stop the published snapshot holds `i` = DINT#1 (the first pass) and the
/// loop's temporaries: `TO DINT#5` stores 5 in `@scratch/3` (the end) and `BY DINT#2` stores 2 in
/// `@scratch/4` (the step). Every mismatch at the first stop is collected. A Null, swapped or shifted
/// scratch row, or a stop on the FOR statement (whose span encloses the body, so the temporaries are
/// not stored yet) fails this test.
#[test]
fn a_breakpoint_in_a_for_body_reads_the_stored_end_and_step_temporaries() {
    let source = "FUNCTION Scan : DINT\nVAR_INPUT seed : DINT; END_VAR\nVAR i : DINT; END_VAR\nScan := seed;\nFOR i := DINT#1 TO DINT#5 BY DINT#2 DO\n    Scan := Scan + i;\nEND_FOR;\nEND_FUNCTION\nPROGRAM Main\nVAR result : DINT; END_VAR\nresult := Scan(DINT#3);\nEND_PROGRAM\n";
    let runtime = CompileSession::from_sources(vec![SourceFile::with_path("main.st", source)])
        .build_runtime()
        .unwrap();
    let mut session = DebugSession::new(runtime);
    session.register_source("main.st", 0, source);
    let mut adapter = DebugAdapter::new(session);
    let control = adapter.session().debug_control();
    let line = source
        .lines()
        .position(|line| line.contains("Scan := Scan + i;"))
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

    // Release the owner even if a native assertion fails while it is paused.
    let inspected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let stop = receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("breakpoint inside the FOR body");
        assert_eq!(stop.reason, DebugStopReason::Breakpoint);
        let snapshot = control.snapshot().expect("paused detached projection");
        let frame = snapshot
            .storage
            .frames()
            .iter()
            .find(|frame| frame.owner.eq_ignore_ascii_case("Scan"))
            .unwrap();
        let stopped = stop
            .location
            .as_ref()
            .and_then(|location| source.get(location.start as usize..location.end as usize));
        let mut wrong = Vec::new();
        if stopped.map(str::trim) != Some("Scan := Scan + i;") {
            wrong.push(format!(
                "the stop is at {stopped:?}, not at the body statement"
            ));
        }
        if frame.variables.get("i") != Some(&RuntimeValue::DInt(1)) {
            wrong.push(format!("i is {:?}, not DINT#1", frame.variables.get("i")));
        }
        if frame.variables.get("@scratch/3") != Some(&RuntimeValue::DInt(5)) {
            wrong.push(format!(
                "@scratch/3 is {:?}, not DINT#5",
                frame.variables.get("@scratch/3")
            ));
        }
        if frame.variables.get("@scratch/4") != Some(&RuntimeValue::DInt(2)) {
            wrong.push(format!(
                "@scratch/4 is {:?}, not DINT#2",
                frame.variables.get("@scratch/4")
            ));
        }
        assert!(wrong.is_empty(), "wrong at the first stop: {wrong:#?}");
    }));
    control.clear_breakpoints();
    control.continue_run();
    thread.join().unwrap().unwrap();
    if let Err(error) = inspected {
        std::panic::resume_unwind(error);
    }
}
