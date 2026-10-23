//! `cycle.step` as specification 11 §6.8.1 ("Stepped scans") writes it: the parameters a request may
//! carry, the refusals before the platform runs the step, the time rule, and the reply. Every
//! expected value is written out here from the specification and the requests below.

use alloc::vec;

use serde_json::{json, Value as JsonValue};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::{Duration, Value};

use super::{request, restart_time, StepOutcome, StepRefusal, StepRequest};
use crate::tests::fake_host::FakeHost;

/// A stopped resource whose platform answers every step with `answer`.
fn stopped(answer: Result<StepOutcome, StepRefusal>) -> FakeHost {
    let host = FakeHost {
        step_answer: Some(answer),
        ..FakeHost::running()
    };
    host.state.set(ResourceState::Stopped);
    host
}

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("a process-image address")
}

#[test]
fn a_stopped_resource_steps_with_what_the_request_names_and_replies_in_the_artifact_encoding() {
    // The parameters reach the platform as the specification names them: the time in
    // nanoseconds, the restart in either case, the inputs and the direct inputs read from their
    // value texts and in the order of their names, and the watches in the request's order. The
    // reply is `time_ns`, the values and the direct values in the conformance encoding, and the
    // text of the scan's error. Catches inputs written in the request's order instead of the
    // names' order, a restart mode read case-sensitively, a text `1_000` read as anything but
    // 1000, and an error reported other than by its text.
    let host = stopped(Ok(StepOutcome {
        values: vec![
            ("b".into(), Value::Int(0)),
            ("a".into(), Value::Int(32_767)),
        ],
        direct: vec![("%QX0.1".into(), Value::Bool(true))],
        error: Some(RuntimeError::Overflow),
    }));
    let params = json!({
        "time_ns": 20_000_000,
        "restart": "WARM",
        "inputs": {"b": "INT:0", "a": "INT:1_000"},
        "direct_inputs": {"%IX0.0": "BOOL:true"},
        "watch": ["b", "a"],
        "watch_direct": ["%QX0.1"],
    });
    let reply = request(7, Some(&params), &host);
    assert_eq!(
        *host.steps.borrow(),
        [StepRequest {
            time: Duration::from_millis(20),
            restart: Some(RestartMode::Warm),
            inputs: vec![("a".into(), Value::Int(1_000)), ("b".into(), Value::Int(0))],
            direct_inputs: vec![("%IX0.0".into(), address("%IX0.0"), Value::Bool(true))],
            watch: vec!["b".into(), "a".into()],
            watch_direct: vec![("%QX0.1".into(), address("%QX0.1"))],
        }]
    );
    assert!(reply.ok, "{:?}", reply.error);
    assert_eq!(
        reply.result_value(),
        Some(json!({
            "time_ns": 20_000_000,
            "values": {
                "a": {"type": "INT", "value": 32_767},
                "b": {"type": "INT", "value": 0},
            },
            "direct": {"%QX0.1": {"type": "BOOL", "value": true}},
            "errors": ["arithmetic overflow"],
        }))
    );
}

#[test]
fn only_a_stopped_resource_with_a_program_takes_a_step() {
    // A running, paused or faulted resource refuses the step as `program.install` is refused, with
    // `resource_not_running`, and a device without a program refuses it with its reason; the
    // platform never sees the step. Catches a step that scans a running program between its
    // cycles, and one that runs where no program is installed.
    let params = json!({"time_ns": 0, "restart": "cold"});
    for state in [
        ResourceState::Running,
        ResourceState::Paused,
        ResourceState::Faulted,
    ] {
        let host = stopped(Err(StepRefusal::Unsupported));
        host.state.set(state);
        let reply = request(1, Some(&params), &host);
        assert!(!reply.ok, "{state:?}");
        assert_eq!(
            reply.error_code.as_deref(),
            Some("resource_not_running"),
            "{state:?}"
        );
        assert!(host.steps.borrow().is_empty(), "{state:?}");
    }
    let host = FakeHost {
        missing_program: Some("the application region holds no program"),
        ..stopped(Err(StepRefusal::Unsupported))
    };
    let reply = request(2, Some(&params), &host);
    assert_eq!(
        reply.error.as_deref(),
        Some("no installed program: the application region holds no program")
    );
    assert_eq!(reply.error_code.as_deref(), Some("resource_not_running"));
    assert!(host.steps.borrow().is_empty());
}

#[test]
fn parameters_that_name_no_step_are_refused_before_the_platform_sees_them() {
    // Each refusal names what is wrong with the parameters. Catches a negative, fractional or
    // too large time taken as a time, an unknown restart mode taken as cold, a number taken as a
    // value text, and an unknown member ignored.
    let cases: [(Option<JsonValue>, &str); 12] = [
        (None, "invalid params: missing params"),
        (Some(json!({})), "invalid params: time_ns is required"),
        (
            Some(json!({"time_ns": -1})),
            "invalid params: time_ns must be a whole number of nanoseconds",
        ),
        (
            Some(json!({"time_ns": 1.5})),
            "invalid params: time_ns must be a whole number of nanoseconds",
        ),
        (
            Some(json!({"time_ns": 9_223_372_036_854_775_808_u64})),
            "invalid params: time_ns must be a whole number of nanoseconds",
        ),
        (
            Some(json!({"time_ns": 0, "restart": "hot"})),
            "invalid params: restart must be \"cold\" or \"warm\"",
        ),
        (
            Some(json!({"time_ns": 0, "inputs": {"a": "INT:x"}})),
            "invalid params: inputs.a: parse INT: 'x'",
        ),
        (
            Some(json!({"time_ns": 0, "inputs": {"a": 7}})),
            "invalid params: inputs.a must be a value text",
        ),
        (
            Some(json!({"time_ns": 0, "inputs": ["a"]})),
            "invalid params: inputs must be an object",
        ),
        (
            Some(json!({"time_ns": 0, "watch": "a"})),
            "invalid params: watch must be an array of texts",
        ),
        (
            Some(json!({"time_ns": 0, "cycles": 2})),
            "invalid params: unknown parameter field",
        ),
        (
            Some(json!({"time_ns": null})),
            "invalid params: parameter fields must not be null",
        ),
    ];
    for (params, expected) in cases {
        let host = stopped(Err(StepRefusal::Unsupported));
        let reply = request(3, params.as_ref(), &host);
        assert_eq!(reply.error.as_deref(), Some(expected), "{params:?}");
        assert!(host.steps.borrow().is_empty(), "{params:?}");
    }
    let host = stopped(Err(StepRefusal::Unsupported));
    let reply = request(
        4,
        Some(&json!({"time_ns": 0, "watch_direct": ["%ZX0.0"]})),
        &host,
    );
    let error = reply.error.unwrap_or_default();
    assert!(
        error.starts_with("invalid params: '%ZX0.0': "),
        "an address that names no process image is refused with the address: {error}"
    );
}

#[test]
fn the_first_step_since_the_stop_restarts_and_no_step_goes_back_in_time() {
    // The first step since the resource stopped must restart the program, and its restart is at
    // time zero; every later step's time is at least the previous step's, a restarting one's too,
    // and its restart is at the previous step's time. Catches a first step that scans the program
    // the run left, a restart at the step's own time, and a time that runs backwards.
    let step = |time: i64, restart: Option<RestartMode>| StepRequest {
        time: Duration::from_millis(time),
        restart,
        inputs: vec![],
        direct_inputs: vec![],
        watch: vec![],
        watch_direct: vec![],
    };
    let ten = Some(Duration::from_millis(10));
    assert_eq!(
        restart_time(&step(5, None), None),
        Err(StepRefusal::RestartRequired)
    );
    assert_eq!(
        restart_time(&step(5, Some(RestartMode::Cold)), None),
        Ok(Duration::ZERO)
    );
    assert_eq!(
        restart_time(&step(5, None), ten),
        Err(StepRefusal::TimeBefore)
    );
    assert_eq!(
        restart_time(&step(5, Some(RestartMode::Warm)), ten),
        Err(StepRefusal::TimeBefore)
    );
    assert_eq!(
        restart_time(&step(10, None), ten),
        Ok(Duration::from_millis(10))
    );
    assert_eq!(
        restart_time(&step(30, Some(RestartMode::Warm)), ten),
        Ok(Duration::from_millis(10))
    );
}

#[test]
fn a_refused_step_answers_in_the_words_of_the_specification() {
    // Catches a refusal reported without its name, and a deferred step answered as a refusal.
    let cases = [
        (StepRefusal::RestartRequired, "restart required"),
        (StepRefusal::TimeBefore, "time before the last step"),
        (
            StepRefusal::name("nope", "names no variable of the program"),
            "'nope' names no variable of the program",
        ),
        (
            StepRefusal::Restart(RuntimeError::Overflow),
            "restart failed: arithmetic overflow",
        ),
        (
            StepRefusal::write("a", &RuntimeError::TypeMismatch),
            "set 'a': type mismatch",
        ),
        (
            StepRefusal::read("%QX0.1", &RuntimeError::TypeMismatch),
            "read '%QX0.1': type mismatch",
        ),
    ];
    for (refusal, expected) in cases {
        let reply = refusal.response(9);
        assert_eq!(reply.error.as_deref(), Some(expected), "{refusal:?}");
    }
    let busy = StepRefusal::Busy.response(9);
    assert_eq!(busy.error_code.as_deref(), Some("resource_busy"));
    let deferred = StepRefusal::Deferred.response(9);
    assert!(deferred.ok);
    assert_eq!(deferred.result_value(), Some(json!({"status": "loading"})));
}
