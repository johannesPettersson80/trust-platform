//! The debug operations of a device that change or refuse (specification 11 §6.9 and §6.13;
//! specification 13), through the shared dispatcher and the resource loop, with the board probe
//! program on the real engine.
//!
//! The board probe `Gpio` (`portability/f401/gpio.st`, the one file of its `LINE_TABLE`, labelled
//! `portability/f401/gpio.st`) has these statements, as 1-based `line:column` spans of the table:
//!
//! | statement | span | code span |
//! | --- | --- | --- |
//! | 0 `led := button;` | 23:1-24:1 | 0..15 of the program |
//! | 1 the `CASE` statement | 24:1-30:9 | 15..184 |
//! | 2 the branch `2: depth_result := ProbeLeaf() + INT#1;` | 25:8-26:5 | 38..65 |
//! | 3 the branch `3: ...` | 26:8-27:5 | 88..115 |
//! | 4 the branch `4: ...` | 27:8-28:1 | 138..165 |
//! | 5 the `ELSE` branch `depth_result := INT#1;` | 29:5-30:1 | 171..184 |
//! | 6 `ProbeLeaf := INT#1;` | 6:1-7:1 | function `ProbeLeaf` |
//! | 7 `ProbeMiddle := ProbeLeaf() + INT#1;` | 10:1-11:1 | function `ProbeMiddle` |
//! | 8 `ProbeOuter := ProbeMiddle() + INT#1;` | 14:1-15:1 | function `ProbeOuter` |
//!
//! A request and a reply carry them zero-based, so statement 5 is line 28, column 4. With
//! `probe_depth` at its initial 1 a scan that runs the program runs statements 0, 1 and 5.
//!
//! A scan holds inside one call of `run_pass`, so the tests use an advancing wait that lets the time
//! of its sleep pass on the clock, and send what reaches the loop during the hold at a time on the
//! clock. The first scan, due at power-on, runs no program: the program first runs at the scan
//! 10 ms later, which the breakpoints hold.

use alloc::vec;
use alloc::vec::Vec;

use serde_json::{json, Value};
use trust_runtime_core::bytecode::{BytecodeModule, LINE_TABLE_SECTION_ID};
use trust_runtime_core::deployment::ControlMode;

use super::debugger::ChipDebug;
use crate::debug::BreakpointView;
use crate::tests::runner_ports::{ms, Fixture, TestPlatform, TestRunner, GPIO};

/// The time of the scan that the tests hold, in milliseconds.
pub(super) const HELD_SCAN_AT: u64 = 10;

/// The label of the one file of the board probe's `LINE_TABLE`.
pub(super) const LABEL: &str = "portability/f401/gpio.st";

/// The path a client holds for that file: below its project, so it ends with the label.
pub(super) const CLIENT_PATH: &str = "/work/trust/portability/f401/gpio.st";

pub(super) fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

pub(super) fn with_params(id: u64, kind: &str, params: Value) -> Value {
    let mut message = request(id, kind);
    message["params"] = params;
    message
}

pub(super) fn set_breakpoints(id: u64, lines: &[u32]) -> Value {
    with_params(
        id,
        "breakpoints.set",
        json!({"source": CLIENT_PATH, "lines": lines}),
    )
}

/// A loop that ran its first scan, which runs no program.
pub(super) fn after_first_scan(fixture: &Fixture, platform: TestPlatform) -> TestRunner<'_> {
    let mut runner = fixture.runner_with(platform);
    fixture.pass_at(&mut runner, 0);
    runner
}

/// Sends `requests` to the link when the advancing wait sleeps through `at` milliseconds, as a host
/// that answers a stop does.
pub(super) fn send_at(fixture: &Fixture, at: u64, requests: Vec<Value>) {
    let link = fixture.link.clone();
    fixture.wait.at(ms(at), move || {
        for request in &requests {
            link.send(request);
        }
    });
}

#[test]
fn breakpoints_set_resolves_each_line_as_the_line_table_names_its_statement() {
    // Specification 13, "Breakpoint evaluation, source resolution, and hook contract": on the
    // requested line the resolver takes the statement with the greatest start column not after the
    // request, else the smallest after it; with none on the line, the innermost statement that
    // contains the position; else the earliest statement after it. A request carries a zero-based
    // line and, as trust-runtime resolves it, the zero-based column 1, so the table is asked for
    // one-based line + 1 and column 2. The reply carries the resolved start zero-based, the file's
    // identifier and the set's generation (specification 11 §6.13.3).
    // Worked from the table above: line 22 is 23:1 `led := button;` -> (22, 0); 28 is 29:5 -> (28, 4);
    // 29 is 30, where no statement starts, and the `CASE` statement 24:1-30:9 contains 30:2 ->
    // (23, 0); 0 is 1, with no statement on or around it, so the earliest statement after it, 6:1 ->
    // (5, 0); 24 is 25:2 and the branch starts at 25:8 -> (24, 7); 99 has no statement at or after it
    // and is skipped. Wrong behaviour caught: a one-based reply (every line off by one), a request
    // line used as one-based, a line that resolves to the wrong neighbour, and an unresolved line
    // that fails the whole request.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());

    let first = fixture.call_at(&mut runner, 2, &set_breakpoints(1, &[22]));
    assert_eq!(
        first["result"],
        json!({"status": "ok", "file_id": 0, "resolved": [{"line": 22, "column": 0}], "generation": 1})
    );

    let second = fixture.call(&mut runner, &set_breakpoints(2, &[28, 29, 0, 99, 24]));
    assert_eq!(
        second["result"],
        json!({
            "status": "ok",
            "file_id": 0,
            "resolved": [
                {"line": 28, "column": 4},
                {"line": 23, "column": 0},
                {"line": 5, "column": 0},
                {"line": 24, "column": 7},
            ],
            "generation": 2
        }),
        "the second set replaced the first and advanced the file's generation"
    );
}

#[test]
fn breakpoints_list_reports_the_replaced_set_by_line_and_column() {
    // Specification 11 §6.13.3: a runtime without source text cannot count byte offsets, so a listed
    // breakpoint carries `file_id`, `line`, `column`, `end_line` and `end_column`, zero-based, in
    // registration order; replacing a file's breakpoints removes the old ones (specification 13,
    // "Runtime debug state, mutation, and projection invariants"). Lines [28, 24] resolve to
    // statement 5, 29:5-30:1 -> (28, 4)-(29, 0), and statement 2, 25:8-26:5 -> (24, 7)-(25, 4). The
    // statement of the first set, 23:1-24:1, is gone. Wrong behaviour caught: a list that keeps the
    // replaced breakpoint, one that reports the end one-based, and one that lists in another order.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());
    let _ = fixture.call_at(&mut runner, 2, &set_breakpoints(1, &[22]));
    let _ = fixture.call(&mut runner, &set_breakpoints(2, &[28, 24]));

    let listed = fixture.call(&mut runner, &request(3, "breakpoints.list"));

    assert_eq!(
        listed["result"],
        json!({"breakpoints": [
            {"file_id": 0, "line": 28, "column": 4, "end_line": 29, "end_column": 0},
            {"file_id": 0, "line": 24, "column": 7, "end_line": 25, "end_column": 4},
        ]})
    );
}

#[test]
fn breakpoints_list_has_the_same_fields_on_a_chip_and_a_computer() {
    // Specification 11 §6.13.3: every runtime lists a breakpoint with one entry shape, `file_id`,
    // `line`, `column`, `end_line` and `end_column`, zero-based, and a computer adds its byte offsets
    // `start` and `end` to it. The chip's entry is the five fields and no others, so a client reads
    // the same five from either runtime. Worked from the statement of the list above: the breakpoint
    // on line 28 is statement 5, 29:5-30:1, which lists as (28, 4) to (29, 0). Wrong behaviour caught:
    // a chip entry that drops or renames one of the five fields, a chip entry that carries byte
    // offsets it cannot count, and an entry that lists its positions one-based.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());
    let _ = fixture.call(&mut runner, &set_breakpoints(1, &[28]));

    let listed = fixture.call(&mut runner, &request(2, "breakpoints.list"));

    let entries = listed["result"]["breakpoints"]
        .as_array()
        .expect("the breakpoints");
    assert_eq!(entries.len(), 1);
    let mut fields = entries[0]
        .as_object()
        .expect("an entry")
        .keys()
        .map(alloc::string::String::as_str)
        .collect::<Vec<_>>();
    fields.sort_unstable();
    assert_eq!(
        fields,
        ["column", "end_column", "end_line", "file_id", "line"]
    );
    assert_eq!(
        entries[0],
        json!({"file_id": 0, "line": 28, "column": 4, "end_line": 29, "end_column": 0})
    );
}

#[test]
fn clearing_breakpoints_removes_them_and_still_advances_the_generation() {
    // Specification 13, "Runtime debug state, mutation, and projection invariants": an empty
    // replacement removes the file's breakpoints while still advancing its generation, and clearing
    // all breakpoints also clears the generation table. The generations run 1 (set), 2 (set), 3
    // (`breakpoints.clear`), 4 (set), 5 (`breakpoints.clear_id`), 6 (set); `breakpoints.clear_all`
    // empties the table, so the next set is generation 1 again. An unknown file is refused with
    // trust-runtime's text. Wrong behaviour caught: a clear that keeps the generation (a stale stop
    // is honoured), a `clear_all` that keeps the table, and an unknown file that succeeds.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());
    let generation = |reply: &Value| reply["result"]["generation"].clone();
    let _ = fixture.call_at(&mut runner, 2, &set_breakpoints(1, &[22]));
    assert_eq!(
        generation(&fixture.call(&mut runner, &set_breakpoints(2, &[28]))),
        json!(2)
    );

    let cleared = fixture.call(
        &mut runner,
        &with_params(
            3,
            "breakpoints.clear",
            json!({"source": CLIENT_PATH, "lines": []}),
        ),
    );
    assert_eq!(cleared["result"], json!({"status": "cleared"}));
    let listed = fixture.call(&mut runner, &request(4, "breakpoints.list"));
    assert_eq!(listed["result"], json!({"breakpoints": []}));
    assert_eq!(
        generation(&fixture.call(&mut runner, &set_breakpoints(5, &[22]))),
        json!(4),
        "the clear advanced the generation to 3"
    );

    let by_id = fixture.call(
        &mut runner,
        &with_params(6, "breakpoints.clear_id", json!({"file_id": 0})),
    );
    assert_eq!(by_id["result"], json!({"status": "cleared", "file_id": 0}));
    assert_eq!(
        generation(&fixture.call(&mut runner, &set_breakpoints(7, &[22]))),
        json!(6),
        "the clear by id advanced the generation to 5"
    );

    let unknown = fixture.call(
        &mut runner,
        &with_params(8, "breakpoints.clear_id", json!({"file_id": 7})),
    );
    assert_eq!(unknown["ok"], json!(false));
    assert_eq!(unknown["error"], json!("unknown file id"));

    let all = fixture.call(&mut runner, &request(9, "breakpoints.clear_all"));
    assert_eq!(all["result"], json!({"status": "cleared"}));
    assert_eq!(
        fixture.call(&mut runner, &request(10, "breakpoints.list"))["result"],
        json!({"breakpoints": []})
    );
    assert_eq!(
        generation(&fixture.call(&mut runner, &set_breakpoints(11, &[22]))),
        json!(1),
        "clearing all breakpoints cleared the generation table"
    );
}

#[test]
fn breakpoint_locations_list_the_statement_starts_in_the_requested_range() {
    // Specification 13, "Breakpoint Locations": the valid statement start positions in the requested
    // range, zero-based, in the table's order. The file's statements start at (22, 0), (23, 0),
    // (24, 7), (25, 7), (26, 7) and (28, 4), and at (5, 0), (9, 0) and (13, 0) in the three functions.
    // Lines 23 to 26 hold four; the first line's column bound drops (23, 0) when it asks for column
    // 1 or more, and the last line's drops nothing at column 7 (the bound is inclusive). Without an
    // end line the range is the one line. Wrong behaviour caught: a one-based answer, an exclusive
    // end column, a range that ignores its column bounds, and a missing end line that lists the file.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());
    let positions = |reply: &Value| reply["result"]["breakpoints"].clone();
    let locations = |id: u64, range: Value| {
        let mut params = json!({"source": CLIENT_PATH});
        for (key, value) in range.as_object().expect("a range object") {
            params[key] = value.clone();
        }
        with_params(id, "debug.breakpoint_locations", params)
    };

    let lines = fixture.call_at(
        &mut runner,
        2,
        &locations(1, json!({"line": 23, "end_line": 26})),
    );
    assert_eq!(
        positions(&lines),
        json!([
            {"line": 23, "column": 0},
            {"line": 24, "column": 7},
            {"line": 25, "column": 7},
            {"line": 26, "column": 7},
        ])
    );

    let columns = fixture.call(
        &mut runner,
        &locations(
            2,
            json!({"line": 23, "column": 1, "end_line": 26, "end_column": 7}),
        ),
    );
    assert_eq!(
        positions(&columns),
        json!([
            {"line": 24, "column": 7},
            {"line": 25, "column": 7},
            {"line": 26, "column": 7},
        ])
    );

    let one_line = fixture.call(&mut runner, &locations(3, json!({"line": 28})));
    assert_eq!(positions(&one_line), json!([{"line": 28, "column": 4}]));
    let none = fixture.call(&mut runner, &locations(4, json!({"line": 100})));
    assert_eq!(positions(&none), json!([]));
}

#[test]
fn a_request_names_its_file_by_a_path_that_ends_with_the_label() {
    // Specification 11 §6.13.3: a client holds a path and a label is relative to the project root,
    // so the file is the one whose label is the path or the longest tail of it after a `/`, a `\`
    // counting as `/`; a path that ends with no label is refused with `unknown source path`.
    // Wrong behaviour caught: a runtime that matches only an exact label (VS Code's absolute path
    // finds no file), one that matches a tail inside a name (`xportability/...`), and one that
    // fails to read a Windows path.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());
    let set = |id: u64, source: &str| {
        with_params(
            id,
            "breakpoints.set",
            json!({"source": source, "lines": [22]}),
        )
    };

    let exact = fixture.call_at(&mut runner, 2, &set(1, LABEL));
    assert_eq!(exact["result"]["file_id"], json!(0));
    let absolute = fixture.call(&mut runner, &set(2, CLIENT_PATH));
    assert_eq!(absolute["result"]["file_id"], json!(0));
    let windows = fixture.call(
        &mut runner,
        &set(3, "C:\\work\\trust\\portability\\f401\\gpio.st"),
    );
    assert_eq!(windows["result"]["file_id"], json!(0));

    for source in [
        "/work/xportability/f401/gpio.st",
        "/work/trust/f401/gpio.st",
        "gpio.st",
        "",
    ] {
        let refused = fixture.call(&mut runner, &set(4, source));
        assert_eq!(refused["ok"], json!(false), "source={source:?}");
        assert_eq!(
            refused["error"],
            json!("unknown source path"),
            "source={source:?}"
        );
    }
}

#[test]
fn a_malformed_request_is_refused_with_trust_runtimes_texts_and_changes_nothing() {
    // Specification 11 §6.8.1: missing parameters fail with `missing params` and a malformed object
    // with `invalid params: <reason>`, and a refused request changes nothing. The reasons are
    // serde's, as trust-runtime's. Wrong behaviour caught: a request without parameters that
    // clears the breakpoints, one that answers another text than trust-runtime's, and a refusal
    // that still advances the generation (the next set would then be generation 3).
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());
    let _ = fixture.call_at(&mut runner, 2, &set_breakpoints(1, &[22]));

    let missing = fixture.call(&mut runner, &request(2, "breakpoints.set"));
    assert_eq!(missing["ok"], json!(false));
    assert_eq!(missing["error"], json!("missing params"));
    let no_source = fixture.call(
        &mut runner,
        &with_params(3, "breakpoints.set", json!({"lines": [22]})),
    );
    assert_eq!(
        no_source["error"],
        json!("invalid params: missing field `source`")
    );
    let no_frame = fixture.call(&mut runner, &with_params(4, "debug.scopes", json!({})));
    assert_eq!(
        no_frame["error"],
        json!("invalid params: missing field `frame_id`")
    );
    let no_lines = fixture.call(
        &mut runner,
        &with_params(5, "breakpoints.clear", json!({"source": CLIENT_PATH})),
    );
    assert_eq!(
        no_lines["error"],
        json!("invalid params: missing field `lines`")
    );

    let listed = fixture.call(&mut runner, &request(6, "breakpoints.list"));
    assert_eq!(
        listed["result"]["breakpoints"].as_array().map(Vec::len),
        Some(1),
        "the refused requests left the breakpoint"
    );
    let next = fixture.call(&mut runner, &set_breakpoints(7, &[22]));
    assert_eq!(next["result"]["generation"], json!(2));
}

#[test]
fn step_in_without_a_hold_is_refused() {
    // Specification 11 §6.9: `step_in`, `step_over` and `step_out` act only while the debugger holds
    // the scan; otherwise they are rejected with a stable error and arm nothing. The text is
    // trust-runtime's (`program_handlers.rs`). With no hold, the later scans run their statements
    // unheld: `depth_result` becomes 1 and the loop is never held. Wrong behaviour caught: a step
    // that arms the debugger without a hold (the next scan would hold at its first statement, and
    // the test's wait would spin until the platform stops it), and a refusal in another text.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());

    for (id, kind) in [(1, "step_in"), (2, "step_over"), (3, "step_out")] {
        let refused = fixture.call_at(&mut runner, 2, &request(id, kind));
        assert_eq!(refused["ok"], json!(false), "{kind}");
        assert_eq!(
            refused["error"],
            json!(
                "stepping requires the scan to be held by the debugger (a breakpoint or a debug-mode pause)"
            ),
            "{kind}"
        );
    }

    fixture.pass_at(&mut runner, HELD_SCAN_AT);
    fixture.pass_at(&mut runner, 2 * HELD_SCAN_AT);
    assert_eq!(fixture.take_scan_times(), [0, 10, 20]);
    let state = fixture.call(&mut runner, &request(4, "debug.state"));
    assert_eq!(
        state["result"],
        json!({"paused": false, "last_stop": null}),
        "nothing held the scan, and nothing was armed"
    );
}

#[test]
fn debug_requests_are_refused_while_debugging_is_disabled() {
    // Specification 11 §6.9: debug requests are rejected when `runtime.control.debug_enabled` is
    // off, and the debug hooks are side-effect-free. Every debug operation of the role table
    // answers `debug disabled` with the stable code `feature_disabled`, and changes nothing: no
    // breakpoint is stored and nothing is held. Wrong behaviour caught: an operation that is routed
    // before the gate (it would answer its own refusal or succeed), and a refused set that still
    // stores its breakpoint.
    let fixture = Fixture::board_probe().with_debugger();
    let platform = TestPlatform {
        debug_enabled: false,
        ..TestPlatform::default()
    };
    let mut runner = after_first_scan(&fixture, platform);

    let operations = [
        ("debug.state", json!(null)),
        ("debug.stops", json!(null)),
        ("debug.stack", json!(null)),
        ("debug.scopes", json!({"frame_id": 0})),
        ("debug.variables", json!({"variables_reference": 1})),
        (
            "debug.breakpoint_locations",
            json!({"source": CLIENT_PATH, "line": 0}),
        ),
        ("debug.evaluate", json!({"expression": "led"})),
        ("eval", json!({"expr": "led"})),
        (
            "breakpoints.set",
            json!({"source": CLIENT_PATH, "lines": [22]}),
        ),
        (
            "breakpoints.clear",
            json!({"source": CLIENT_PATH, "lines": []}),
        ),
        ("breakpoints.list", json!(null)),
        ("breakpoints.clear_all", json!(null)),
        ("breakpoints.clear_id", json!({"file_id": 0})),
        ("step_in", json!(null)),
        ("step_over", json!(null)),
        ("step_out", json!(null)),
    ];
    for (id, (kind, params)) in (1_u64..).zip(operations) {
        let refused = fixture.call_at(
            &mut runner,
            2,
            &json!({"id": id, "type": kind, "params": params}),
        );
        assert_eq!(refused["ok"], json!(false), "{kind}");
        assert_eq!(refused["error"], json!("debug disabled"), "{kind}");
        assert_eq!(refused["error_code"], json!("feature_disabled"), "{kind}");
    }

    assert_eq!(
        runner.shell().debug.breakpoints(),
        Vec::<BreakpointView>::new()
    );
    assert!(!runner.shell().debug.is_held());
}

#[test]
fn a_pause_without_a_line_table_is_refused_before_the_debugger_pauses() {
    // Specification 11 §6.13.3 (decision O8): a program without a `LINE_TABLE` has no statement start
    // to hold at, so a debug-mode `pause` is refused with the stable code `no_line_table` and the
    // message that says to rebuild, before the debugger is asked to hold anything; the other debug
    // operations are refused alike, and `resume` is unaffected. The scans that follow run unheld.
    // Wrong behaviour caught: a pause that answers `paused` and holds nothing (the debugger would
    // report a hold that never comes), one that arms the debugger, and an operation that answers an
    // error of its own instead of the rebuild error.
    let mut module = BytecodeModule::decode(GPIO).expect("the board probe decodes");
    module
        .sections
        .retain(|section| section.id != LINE_TABLE_SECTION_ID);
    let stripped = module.encode().expect("the stripped container encodes");
    let fixture = Fixture::with_program(&stripped).with_debugger();
    let platform = TestPlatform {
        control_mode: ControlMode::Debug,
        ..TestPlatform::default()
    };
    let mut runner = after_first_scan(&fixture, platform);

    let paused = fixture.call_at(&mut runner, 2, &request(1, "pause"));

    assert_eq!(paused["ok"], json!(false));
    assert_eq!(paused["error_code"], json!("no_line_table"));
    assert_eq!(
        paused["error"],
        json!("the program has no line table, so it cannot be debugged: rebuild the project")
    );
    assert!(
        !runner.shell().debug.is_held(),
        "the debugger was not asked"
    );
    assert_eq!(runner.shell().debug.hold_label(), None);
    for (id, kind, params) in [
        (2, "debug.state", json!(null)),
        (3, "debug.stack", json!(null)),
        (
            4,
            "breakpoints.set",
            json!({"source": CLIENT_PATH, "lines": [22]}),
        ),
        (5, "breakpoints.list", json!(null)),
        (6, "step_in", json!(null)),
    ] {
        let refused = fixture.call(
            &mut runner,
            &json!({"id": id, "type": kind, "params": params}),
        );
        assert_eq!(refused["error_code"], json!("no_line_table"), "{kind}");
    }
    let resumed = fixture.call(&mut runner, &request(7, "resume"));
    assert_eq!(resumed["result"], json!({"status": "running"}));

    fixture.pass_at(&mut runner, HELD_SCAN_AT);
    assert_eq!(fixture.take_scan_times(), [0, 10], "no scan was held");
}

#[test]
fn a_runner_without_a_debugger_leaves_the_debug_operations_to_the_platform() {
    // Specification 34 STATE-08: a request the shared services do not answer goes to the platform,
    // which answers `unsupported request` when it has no such operation. A loop whose program has no
    // debugger attached (the transcript composition of trust-runtime) therefore answers the debug
    // operations as before, and the platform sees them. Wrong behaviour caught: a dispatcher that
    // answers a debug operation for a program that has no debugger.
    let fixture = Fixture::board_probe();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());

    let answer = fixture.call_at(&mut runner, 2, &request(1, "debug.state"));

    assert_eq!(answer["ok"], json!(false));
    assert_eq!(answer["error"], json!("unsupported request"));
}

#[test]
fn eval_answers_the_stable_refusal_until_compiled_expressions_exist() {
    // Specification 11 §6.13.2 ("Before a runtime implements a version"): `eval`, `debug.evaluate`
    // and a `breakpoints.set` that carries a `condition` or a `log_message` are refused with the
    // stable code `expr_version_unsupported`; a refused set is refused whole, so it stores nothing
    // and advances no generation, and a `null` member is no condition. Wrong behaviour caught: a
    // condition that is dropped silently (the breakpoint would stop unconditionally), a refusal
    // that still stores the breakpoint, and an `eval` that answers a name read.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());
    let message = "this runtime evaluates no debug expressions yet: the compiled expression form is not implemented";

    for (id, kind, params) in [
        (1, "eval", json!({"expr": "probe_depth"})),
        (2, "debug.evaluate", json!({"expression": "probe_depth"})),
        (
            3,
            "breakpoints.set",
            json!({"source": CLIENT_PATH, "lines": [22], "condition": "probe_depth = 1"}),
        ),
        (
            4,
            "breakpoints.set",
            json!({"source": CLIENT_PATH, "lines": [22], "log_message": "depth {probe_depth}"}),
        ),
    ] {
        let refused = fixture.call_at(
            &mut runner,
            2,
            &json!({"id": id, "type": kind, "params": params}),
        );
        assert_eq!(refused["ok"], json!(false), "{kind}");
        assert_eq!(refused["error"], json!(message), "{kind}");
        assert_eq!(
            refused["error_code"],
            json!("expr_version_unsupported"),
            "{kind}"
        );
    }
    let listed = fixture.call(&mut runner, &request(5, "breakpoints.list"));
    assert_eq!(listed["result"], json!({"breakpoints": []}));

    let plain = fixture.call(
        &mut runner,
        &with_params(
            6,
            "breakpoints.set",
            json!({"source": CLIENT_PATH, "lines": [22], "condition": null}),
        ),
    );
    assert_eq!(
        plain["result"]["generation"],
        json!(1),
        "the refused sets advanced no generation, and a null condition is none"
    );
}

#[test]
fn debug_stops_keeps_the_newest_stops_up_to_its_bound() {
    // Specification 11 §6.13.3: `debug.stops` returns the stops since its last call, oldest first,
    // from a buffer whose bound is the event store's capacity (3 here), and a stop beyond the bound
    // replaces the oldest. Five stops happen: statement 0 at the breakpoint (line 22), then
    // `step_in` stops at statement 1 (line 23), at statement 5 (line 28), and, in the next scan, at
    // statement 0 again (22) and at statement 1 (23). The buffer keeps the newest three, the steps at
    // 28, 22 and 23, none of them with a breakpoint generation; a second read finds it empty. Wrong
    // behaviour caught: a buffer that keeps the oldest (the first entry would be the breakpoint stop,
    // with generation 1), one without a bound (five entries), one that keeps only the latest stop, and
    // a read that does not empty the buffer.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = fixture.runner_with_capacity(3);
    fixture.pass_at(&mut runner, 0);
    let set = fixture.call_at(&mut runner, 2, &set_breakpoints(1, &[22]));
    assert_eq!(set["result"]["generation"], json!(1));
    fixture.wait.advance_time();
    for (at, id) in [(1_000, 2), (2_000, 3), (3_000, 4)] {
        send_at(&fixture, at, vec![request(id, "step_in")]);
    }
    fixture.pass_at(&mut runner, HELD_SCAN_AT);
    let first_scan = fixture.link.take_replies();
    assert_eq!(first_scan.len(), 3, "{first_scan:?}");
    for reply in &first_scan {
        assert_eq!(reply["result"], json!({"status": "stepping"}));
    }

    send_at(&fixture, 4_000, vec![request(5, "step_in")]);
    send_at(&fixture, 4_500, vec![request(6, "debug.stops")]);
    send_at(&fixture, 4_600, vec![request(7, "debug.stops")]);
    send_at(&fixture, 5_000, vec![request(8, "resume")]);
    runner.run_pass().expect("the test wait does not fail");

    let replies = fixture.link.take_replies();
    assert_eq!(replies.len(), 4, "{replies:?}");
    let stop = |line: u32, column: u32| {
        json!({
            "reason": "step",
            "thread_id": 1,
            "breakpoint_generation": null,
            "file_id": 0,
            "line": line,
            "column": column,
            "path": LABEL,
        })
    };
    assert_eq!(
        replies[1]["result"],
        json!({"stops": [stop(28, 4), stop(22, 0), stop(23, 0)]})
    );
    assert_eq!(replies[2]["result"], json!({"stops": []}));
}

#[test]
fn thread_ids_are_per_task() {
    // Specification 13, "Multi-Task Thread Model", and specification 11 §6.13.3: each task is one
    // debugger thread, set when the task starts. The board probe has one task, `Scan`, which is
    // thread 1. The debugger is left on thread 9 before the scan, so a stop that reports thread 1
    // proves that the task start set it. Wrong behaviour caught: a thread that is never set at the
    // task start (the stop would report 9), and one set to the wrong task.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());
    let _ = fixture.call_at(&mut runner, 2, &set_breakpoints(1, &[22]));
    runner.shell().debug.set_thread(Some(9));
    fixture.wait.advance_time();
    send_at(
        &fixture,
        1_000,
        vec![request(2, "debug.state"), request(3, "resume")],
    );

    fixture.pass_at(&mut runner, HELD_SCAN_AT);

    let replies = fixture.link.take_replies();
    assert_eq!(replies[0]["result"]["last_stop"]["thread_id"], json!(1));
}

#[test]
fn a_debugger_has_no_published_state_until_a_scan_ends() {
    // Specification 11 §6.13.1: before the first scan ends there is no published state; a restart
    // clears it again. The flag starts false, a scan that ended sets it, and a restart clears it.
    // Wrong behaviour caught: a read that answers an invented empty state before the first scan.
    let debug = ChipDebug::new(0);
    assert!(!debug.published());
    debug.scan_ended();
    assert!(debug.published());
    debug.restarted();
    assert!(!debug.published());
    assert!(!debug.take_busy(), "no operation found the state in use");
}
