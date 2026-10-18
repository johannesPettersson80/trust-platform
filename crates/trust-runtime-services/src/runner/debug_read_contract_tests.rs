//! The reads of the debug operations on a device (specification 11 §6.9, §6.9.5 and §6.13;
//! specification 13), through the shared dispatcher and the resource loop, with the board probe
//! program on the real engine. The program, its `LINE_TABLE` and the way the tests hold a scan are
//! described in `debug_contract_tests.rs`.
//!
//! Most tests hold the scan in `ProbeLeaf`, four frames deep. With `probe_depth` at 4 the second scan
//! runs `led := button;` and the `CASE` statement, whose branch 4 calls `ProbeOuter`; that calls
//! `ProbeMiddle`, and that calls `ProbeLeaf`, where a breakpoint on line 5 (`ProbeLeaf := INT#1;`,
//! 6:1) holds the scan. The call stack is then `Gpio` (POU 0), `ProbeOuter` (36), `ProbeMiddle` (35)
//! and `ProbeLeaf` (34), outermost first. Each frame last stood at the statement that was running in
//! it: `Gpio` at the branch statement 27:8, `ProbeOuter` at 14:1, `ProbeMiddle` at 10:1, `ProbeLeaf`
//! at 6:1 (zero-based: (26, 7), (13, 0), (9, 0) and (5, 0)). The identifiers of the frames and of the
//! program instance are the engine's, so the tests read them from the engine and write everything
//! else by hand.

use alloc::format;
use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde_json::{json, Value};
use trust_runtime_core::ports::PeerClass;

use super::debug_contract_tests::{
    after_first_scan, request, send_at, set_breakpoints, with_params, HELD_SCAN_AT, LABEL,
};
use crate::debug::projection::VariableHandle;
use crate::tests::runner_ports::{ms, Fixture, TestPlatform, TestRunner};

/// One frame of the held call stack, as the engine identifies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Level {
    /// The frame's identity.
    frame: u32,
    /// The POU the frame runs.
    pou: u32,
    /// The instance that receives the invocation.
    instance: Option<u32>,
}

type Stack = Rc<RefCell<Vec<Level>>>;

/// Records the call stack of the scan at every pass of a hold, outermost frame first.
fn record_stack(runner: &TestRunner<'_>) -> Stack {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&seen);
    runner.shell().set_probe(move |view| {
        *sink.borrow_mut() = view
            .frame
            .call_stack()
            .filter_map(|frame| {
                Some(Level {
                    frame: frame.activation()?.0,
                    pou: frame.pou_id()?,
                    instance: frame.runtime_instance().map(|instance| instance.0),
                })
            })
            .collect();
    });
    seen
}

/// What the scan held in `ProbeLeaf` saw of the requests sent to it.
struct DeepHold {
    /// The replies, in the order the requests were sent, ending with the reply to `resume`.
    replies: Vec<Value>,
    /// The call stack of the held scan, outermost frame first.
    stack: Vec<Level>,
}

/// Holds the board probe in `ProbeLeaf` and sends the requests that `requests` makes from the held
/// call stack, then a `resume`.
fn deep_hold(requests: impl FnOnce(&[Level]) -> Vec<Value> + 'static) -> DeepHold {
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = after_first_scan(&fixture, TestPlatform::default());
    let stack = record_stack(&runner);
    let queued = fixture.call_at(
        &mut runner,
        1,
        &with_params(
            1,
            "set",
            json!({"target": "global:probe_depth", "value": "4"}),
        ),
    );
    assert_eq!(queued["ok"], json!(true), "{queued}");
    let placed = fixture.call(&mut runner, &set_breakpoints(2, &[5]));
    assert_eq!(
        placed["result"]["resolved"],
        json!([{"line": 5, "column": 0}])
    );
    fixture.wait.advance_time();
    {
        let link = fixture.link.clone();
        let stack = Rc::clone(&stack);
        fixture.wait.at(ms(1_000), move || {
            for sent in requests(&stack.borrow()) {
                link.send(&sent);
            }
        });
    }
    send_at(&fixture, 2_000, vec![request(99, "resume")]);
    fixture.pass_at(&mut runner, HELD_SCAN_AT);
    let replies = fixture.link.take_replies();
    let stack = stack.borrow().clone();
    DeepHold { replies, stack }
}

/// The source of a frame or a scope in the board probe's file.
fn gpio_source() -> Value {
    json!({"name": "gpio.st", "path": LABEL})
}

#[test]
fn debug_stack_reply_on_a_chip_equals_the_trust_runtime_reply_for_the_same_stop() {
    // Specification 13, "Stack Trace and Navigation": the stack lists each frame with its own source
    // location, innermost first, and trust-runtime's reply carries `id`, `name`, `source` (`name`,
    // `path`), `line` and `column` of each frame, zero-based, with `total_frames` (specification 11
    // §6.13.3: a runtime without text names the file by its label). Worked from the program: the
    // stack is ProbeLeaf at 6:1 -> (5, 0), ProbeMiddle at 10:1 -> (9, 0), ProbeOuter at 14:1 ->
    // (13, 0) and Gpio at the branch 27:8 -> (26, 7); the engine gives the frame identities and the
    // POUs, whose order and names are asserted. Wrong behaviour caught: a frame that reports the top
    // location for every frame, an outermost-first order, a frame named after another POU, a
    // missing `total_frames`, one-based positions, and identities that are not the engine's.
    let held = deep_hold(|_| vec![request(10, "debug.stack")]);

    let pous = held.stack.iter().map(|level| level.pou).collect::<Vec<_>>();
    assert_eq!(
        pous,
        [0, 36, 35, 34],
        "Gpio, ProbeOuter, ProbeMiddle, ProbeLeaf"
    );
    let ids = held
        .stack
        .iter()
        .map(|level| level.frame)
        .collect::<Vec<_>>();
    for (position, id) in ids.iter().enumerate() {
        assert!(
            !ids[..position].contains(id),
            "the engine gives each frame its own identity: {ids:?}"
        );
    }
    let frame = |level: usize, name: &str, line: u32, column: u32| {
        json!({
            "id": held.stack[level].frame,
            "name": name,
            "source": gpio_source(),
            "line": line,
            "column": column,
        })
    };
    assert_eq!(
        held.replies[0]["result"],
        json!({
            "stack_frames": [
                frame(3, "ProbeLeaf", 5, 0),
                frame(2, "ProbeMiddle", 9, 0),
                frame(1, "ProbeOuter", 13, 0),
                frame(0, "Gpio", 26, 7),
            ],
            "total_frames": 4
        })
    );
}

#[test]
fn debug_scopes_and_variables_list_the_stop_as_trust_runtime_lists_it() {
    // Specification 13, "Variables / Evaluate" and "Variable Visibility", and specification 11
    // §6.13.1: the scopes of a frame are its locals (with the frame's position), then the globals,
    // the retained variables, the I/O and the instances, each when it has content, with handles that
    // restart at 1; a handle lists its variables in the stable debug representation with the public
    // type name. Worked from the program at the stop: the locals of ProbeLeaf are its return slot
    // `ProbeLeaf`, INT 0; the globals are `probe_depth` (4) and the program instance `Plant`, whose
    // type is `Gpio`; the I/O has the input `%IX0.0` and the output `%QX0.0`, both false, and no
    // memory; the instances list `Gpio#<identity>`; the program instance holds `button`, `led`
    // and `depth_result`. The handles run Locals 1, Globals 2, I/O 3, Instances 4, then `Plant` 5, the
    // three I/O areas 6 to 8 and the instance entry 9. Wrong behaviour caught: a scope list in
    // another order or without the I/O, handles that do not restart at 1, a value in Rust's debug
    // text (`Int(4)`), an instance shown as `Instance`, an I/O row named by anything but its
    // address, and a frame's locals that include the compiler's scratch slots.
    let held = deep_hold(|stack| {
        let innermost = stack.last().map(|level| level.frame).expect("a held stack");
        vec![
            with_params(10, "debug.scopes", json!({"frame_id": innermost})),
            with_params(11, "debug.variables", json!({"variables_reference": 1})),
            with_params(12, "debug.variables", json!({"variables_reference": 2})),
            with_params(13, "debug.variables", json!({"variables_reference": 3})),
            with_params(14, "debug.variables", json!({"variables_reference": 4})),
            with_params(15, "debug.variables", json!({"variables_reference": 5})),
            with_params(16, "debug.variables", json!({"variables_reference": 6})),
        ]
    });
    let instance = held.stack[0]
        .instance
        .expect("the program frame runs in the program instance");

    assert_eq!(
        held.replies[0]["result"],
        json!({"scopes": [
            {"name": "Locals", "variablesReference": 1, "expensive": false, "source": gpio_source(), "line": 5, "column": 0},
            {"name": "Globals", "variablesReference": 2, "expensive": false},
            {"name": "I/O", "variablesReference": 3, "expensive": false},
            {"name": "Instances", "variablesReference": 4, "expensive": false},
        ]})
    );
    assert_eq!(
        held.replies[1]["result"],
        json!({"variables": [
            {"name": "ProbeLeaf", "value": "0", "type": "INT", "variablesReference": 0, "evaluateName": "ProbeLeaf"},
        ]})
    );
    assert_eq!(
        held.replies[2]["result"],
        json!({"variables": [
            {"name": "probe_depth", "value": "4", "type": "INT", "variablesReference": 0, "evaluateName": "probe_depth"},
            {"name": "Plant", "value": "Gpio", "type": "Gpio", "variablesReference": 5, "evaluateName": "Plant"},
        ]})
    );
    assert_eq!(
        held.replies[3]["result"],
        json!({"variables": [
            {"name": "Inputs", "value": "1 items", "variablesReference": 6},
            {"name": "Outputs", "value": "1 items", "variablesReference": 7},
            {"name": "Memory", "value": "0 items", "variablesReference": 8},
        ]})
    );
    assert_eq!(
        held.replies[4]["result"],
        json!({"variables": [
            {"name": format!("Gpio#{instance}"), "value": "Gpio", "type": "Gpio", "variablesReference": 9},
        ]})
    );
    assert_eq!(
        held.replies[5]["result"],
        json!({"variables": [
            {"name": "button", "value": "FALSE", "type": "BOOL", "variablesReference": 0, "evaluateName": "button"},
            {"name": "led", "value": "FALSE", "type": "BOOL", "variablesReference": 0, "evaluateName": "led"},
            {"name": "depth_result", "value": "0", "type": "INT", "variablesReference": 0, "evaluateName": "depth_result"},
        ]})
    );
    assert_eq!(
        held.replies[6]["result"],
        json!({"variables": [
            {"name": "%IX0.0", "value": "Bool(false)", "variablesReference": 0, "evaluateName": "%IX0.0"},
        ]})
    );
}

#[test]
fn the_locals_of_the_program_frame_are_the_variables_of_its_instance() {
    // Specification 13, "Variable Visibility": the local scope of a program lists the variables of the
    // program instance that the frame runs in, in declaration order, followed by the frame's own,
    // which `Gpio` has none of. The scopes of the outermost frame (the `Gpio` frame) name its last
    // position, the branch at 27:8 -> (26, 7). Wrong behaviour caught: locals that list only the
    // frame's own slots (nothing for a program), and a scope that reports the position of the top
    // frame.
    let held = deep_hold(|stack| {
        let outermost = stack
            .first()
            .map(|level| level.frame)
            .expect("a held stack");
        vec![
            with_params(10, "debug.scopes", json!({"frame_id": outermost})),
            with_params(11, "debug.variables", json!({"variables_reference": 1})),
        ]
    });

    assert_eq!(
        held.replies[0]["result"]["scopes"][0],
        json!({"name": "Locals", "variablesReference": 1, "expensive": false, "source": gpio_source(), "line": 26, "column": 7})
    );
    assert_eq!(
        held.replies[1]["result"],
        json!({"variables": [
            {"name": "button", "value": "FALSE", "type": "BOOL", "variablesReference": 0, "evaluateName": "button"},
            {"name": "led", "value": "FALSE", "type": "BOOL", "variablesReference": 0, "evaluateName": "led"},
            {"name": "depth_result", "value": "0", "type": "INT", "variablesReference": 0, "evaluateName": "depth_result"},
        ]})
    );
}

#[test]
fn a_stale_frame_or_variable_reference_fails_closed() {
    // Specification 11 §6.9.5, "Runtime control audit and debug projection": a stale frame or an
    // unknown variable reference fails closed with a negative response and an error text, never an
    // empty list. The texts are trust-runtime's: `unknown frame id <n>` and `unknown variables
    // reference <n>`. Wrong behaviour caught: an empty successful list for a frame or a handle
    // that does not exist.
    let held = deep_hold(|_| {
        vec![
            with_params(10, "debug.scopes", json!({"frame_id": 9_999})),
            with_params(11, "debug.variables", json!({"variables_reference": 4_242})),
        ]
    });

    assert_eq!(held.replies[0]["ok"], json!(false));
    assert_eq!(held.replies[0]["error"], json!("unknown frame id 9999"));
    assert_eq!(held.replies[1]["ok"], json!(false));
    assert_eq!(
        held.replies[1]["error"],
        json!("unknown variables reference 4242")
    );
}

#[test]
fn a_listed_handle_stays_valid_and_each_listing_hands_out_new_member_handles() {
    // Specification 13, "Runtime debug state, mutation, and projection invariants": a handle stays
    // valid until the next clear, and allocation never aliases two live handles. Worked from the
    // stop: the scopes are Locals 1, Globals 2, I/O 3 and Instances 4. Listing the globals hands out
    // 5 to the program instance `Plant`; listing them again lists the same two variables with 6 for
    // `Plant`, and handle 5 still lists the program instance's variables. Wrong behaviour caught: a
    // handle that its own listing consumes (the second listing fails with `unknown variables
    // reference 2`), a member handle that reuses the identifier of a live handle, and a listing that
    // loses the member handles an earlier listing handed out.
    let held = deep_hold(|stack| {
        let innermost = stack.last().map(|level| level.frame).expect("a held stack");
        vec![
            with_params(10, "debug.scopes", json!({"frame_id": innermost})),
            with_params(11, "debug.variables", json!({"variables_reference": 2})),
            with_params(12, "debug.variables", json!({"variables_reference": 2})),
            with_params(13, "debug.variables", json!({"variables_reference": 5})),
        ]
    });

    assert_eq!(
        held.replies[1]["result"],
        json!({"variables": [
            {"name": "probe_depth", "value": "4", "type": "INT", "variablesReference": 0, "evaluateName": "probe_depth"},
            {"name": "Plant", "value": "Gpio", "type": "Gpio", "variablesReference": 5, "evaluateName": "Plant"},
        ]})
    );
    assert_eq!(
        held.replies[2]["result"],
        json!({"variables": [
            {"name": "probe_depth", "value": "4", "type": "INT", "variablesReference": 0, "evaluateName": "probe_depth"},
            {"name": "Plant", "value": "Gpio", "type": "Gpio", "variablesReference": 6, "evaluateName": "Plant"},
        ]})
    );
    assert_eq!(
        held.replies[3]["result"],
        json!({"variables": [
            {"name": "button", "value": "FALSE", "type": "BOOL", "variablesReference": 0, "evaluateName": "button"},
            {"name": "led", "value": "FALSE", "type": "BOOL", "variablesReference": 0, "evaluateName": "led"},
            {"name": "depth_result", "value": "0", "type": "INT", "variablesReference": 0, "evaluateName": "depth_result"},
        ]})
    );
}

/// The board probe held at the `ELSE` branch, with the globals' handle allocated as handle 1.
fn held_at_the_else_branch(fixture: &Fixture) -> TestRunner<'_> {
    let mut runner = after_first_scan(fixture, TestPlatform::default());
    let _ = fixture.call_at(&mut runner, 2, &set_breakpoints(1, &[28]));
    runner.shell().debug.with_handles(&mut |handles| {
        handles.alloc(VariableHandle::Globals);
    });
    runner
}

/// The stop at the `ELSE` branch, as the replies carry it: breakpoint generation 1, line 28, column 4.
fn else_branch_stop() -> Value {
    json!({
        "reason": "breakpoint",
        "thread_id": 1,
        "breakpoint_generation": 1,
        "file_id": 0,
        "line": 28,
        "column": 4,
        "path": LABEL,
    })
}

#[test]
fn debug_variables_is_a_viewer_operation() {
    // Specification 11 §6.9.5: the debug state, stack, scope, variable and location reads and
    // `breakpoints.list` need Viewer; `pause` needs Operator; the step operations, breakpoint
    // mutation and `debug.evaluate` need Engineer; and a role denial carries the stable code
    // `insufficient_role`, changes nothing and is answered before the debugger is reached. A client
    // on the network without a token is a Viewer. Worked from the registry: the Viewer reads the
    // globals (`probe_depth`, 1, and `Plant`, whose handle is 2 because handle 1 is the list
    // itself), the state of the hold and the one breakpoint, and is refused the rest. Wrong
    // behaviour caught: a read classified above Viewer (the Viewer is refused), a mutation
    // classified at Viewer (the breakpoint set would store a second breakpoint, or the step would
    // release the hold), and a role denial that is answered after the effect.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = held_at_the_else_branch(&fixture);
    fixture.wait.advance_time();
    let network = PeerClass::Network(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
        9000,
    ));
    let link = fixture.link.clone();
    fixture.wait.at(ms(1_000), move || {
        for sent in [
            with_params(10, "debug.variables", json!({"variables_reference": 1})),
            request(11, "debug.state"),
            request(12, "breakpoints.list"),
            set_breakpoints(13, &[22]),
            request(14, "step_in"),
            request(15, "pause"),
            with_params(16, "debug.evaluate", json!({"expression": "led"})),
        ] {
            link.send_bytes(sent.to_string().as_bytes(), network);
        }
    });
    send_at(&fixture, 2_000, vec![request(99, "resume")]);

    fixture.pass_at(&mut runner, HELD_SCAN_AT);

    let replies = fixture.link.take_replies();
    assert_eq!(replies.len(), 8, "{replies:?}");
    assert_eq!(
        replies[0]["result"],
        json!({"variables": [
            {"name": "probe_depth", "value": "1", "type": "INT", "variablesReference": 0, "evaluateName": "probe_depth"},
            {"name": "Plant", "value": "Gpio", "type": "Gpio", "variablesReference": 2, "evaluateName": "Plant"},
        ]})
    );
    assert_eq!(
        replies[1]["result"],
        json!({"paused": true, "last_stop": else_branch_stop()})
    );
    assert_eq!(
        replies[2]["result"],
        json!({"breakpoints": [
            {"file_id": 0, "line": 28, "column": 4, "end_line": 29, "end_column": 0},
        ]})
    );
    for (reply, role) in [
        (&replies[3], "engineer"),
        (&replies[4], "engineer"),
        (&replies[5], "operator"),
        (&replies[6], "engineer"),
    ] {
        assert_eq!(reply["ok"], json!(false), "{reply}");
        assert_eq!(
            reply["error"],
            json!(format!("forbidden: requires role {role}")),
            "{reply}"
        );
        assert_eq!(reply["error_code"], json!("insufficient_role"), "{reply}");
    }
    assert_eq!(
        replies[7]["result"],
        json!({"status": "running"}),
        "the refused step released nothing: the hold lasted until the local resume"
    );
    assert_eq!(runner.shell().debug.breakpoints().len(), 1);
}

#[test]
fn a_breakpoint_holds_the_scan_and_the_stops_name_it_once() {
    // Specification 13, "Stopped Events" and "Runtime Debug-Control Execution": a breakpoint hit
    // pauses before the statement and emits one `breakpoint` stop carrying the current breakpoint
    // generation (specification 11 §6.13.3: the file by identifier and label, line and column
    // zero-based). While held, `debug.state` says paused and names the stop, `debug.stops` returns it
    // once and then nothing, and after the resume the scan has left the hold but the latest stop
    // stays. Worked: the breakpoint at line 28 resolves to the `ELSE` branch (28, 4) in generation 1,
    // on thread 1. Wrong behaviour caught: a state that is not paused, a stop without its
    // generation, a buffer read that does not empty it, and a latest stop that vanishes with the
    // hold.
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = held_at_the_else_branch(&fixture);
    fixture.wait.advance_time();
    send_at(
        &fixture,
        1_000,
        vec![
            request(10, "debug.state"),
            request(11, "debug.stops"),
            request(12, "debug.stops"),
            request(13, "breakpoints.clear_all"),
            request(14, "resume"),
        ],
    );

    fixture.pass_at(&mut runner, HELD_SCAN_AT);

    let replies = fixture.link.take_replies();
    assert_eq!(
        replies[0]["result"],
        json!({"paused": true, "last_stop": else_branch_stop()})
    );
    assert_eq!(replies[1]["result"], json!({"stops": [else_branch_stop()]}));
    assert_eq!(replies[2]["result"], json!({"stops": []}));
    let after = answer_between_scans(&fixture, &mut runner, &request(20, "debug.state"));
    assert_eq!(
        after["result"],
        json!({"paused": false, "last_stop": else_branch_stop()})
    );
}

/// Sends `request` after a hold has ended and returns its reply. The scan that is long due runs
/// first, and the request waits for the pass after it (specification 34, STATE-08).
fn answer_between_scans(fixture: &Fixture, runner: &mut TestRunner<'_>, request: &Value) -> Value {
    fixture.link.send(request);
    runner.run_pass().expect("the test wait does not fail");
    runner.run_pass().expect("the test wait does not fail");
    let mut replies = fixture.link.take_replies();
    assert_eq!(replies.len(), 1, "one reply to {request}: {replies:?}");
    replies.remove(0)
}

#[test]
fn a_read_between_scans_answers_from_the_last_scan_without_a_call_stack() {
    // Specification 11 §6.13.1: when no hold is in effect a read answers from the published state of
    // the last scan that ended, with no call stack, and a scan in progress is never visible; the
    // synthetic `Main` frame stands for the latest stop's position, as trust-runtime's `debug.stack`
    // reports it, and no frame has scopes (`unknown frame id 0`). Worked: the scans after the hold
    // ran the `ELSE` branch, so `depth_result` is 1, and `probe_depth` is still 1. Wrong behaviour
    // caught: a stack with the held frames after the hold ended, scopes of a frame that is gone,
    // and values read from a copy made at the stop (`depth_result` would still be 0).
    let fixture = Fixture::board_probe().with_debugger();
    let mut runner = held_at_the_else_branch(&fixture);
    fixture.wait.advance_time();
    send_at(
        &fixture,
        1_000,
        vec![request(10, "breakpoints.clear_all"), request(11, "resume")],
    );
    fixture.pass_at(&mut runner, HELD_SCAN_AT);
    let _ = fixture.link.take_replies();

    let stack = answer_between_scans(&fixture, &mut runner, &request(20, "debug.stack"));
    assert_eq!(
        stack["result"],
        json!({
            "stack_frames": [
                {"id": 0, "name": "Main", "source": gpio_source(), "line": 28, "column": 4},
            ],
            "total_frames": 1
        })
    );
    let scopes = answer_between_scans(
        &fixture,
        &mut runner,
        &with_params(21, "debug.scopes", json!({"frame_id": 0})),
    );
    assert_eq!(scopes["ok"], json!(false));
    assert_eq!(scopes["error"], json!("unknown frame id 0"));
    let globals = answer_between_scans(
        &fixture,
        &mut runner,
        &with_params(22, "debug.variables", json!({"variables_reference": 1})),
    );
    assert_eq!(
        globals["result"],
        json!({"variables": [
            {"name": "probe_depth", "value": "1", "type": "INT", "variablesReference": 0, "evaluateName": "probe_depth"},
            {"name": "Plant", "value": "Gpio", "type": "Gpio", "variablesReference": 2, "evaluateName": "Plant"},
        ]})
    );
    let plant = answer_between_scans(
        &fixture,
        &mut runner,
        &with_params(23, "debug.variables", json!({"variables_reference": 2})),
    );
    assert_eq!(
        plant["result"],
        json!({"variables": [
            {"name": "button", "value": "FALSE", "type": "BOOL", "variablesReference": 0, "evaluateName": "button"},
            {"name": "led", "value": "FALSE", "type": "BOOL", "variablesReference": 0, "evaluateName": "led"},
            {"name": "depth_result", "value": "1", "type": "INT", "variablesReference": 0, "evaluateName": "depth_result"},
        ]})
    );
}
