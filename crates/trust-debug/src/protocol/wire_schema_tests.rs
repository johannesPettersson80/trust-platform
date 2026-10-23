use super::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fmt::Debug};

fn assert_wire<T>(value: T, expected: Value)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let encoded = serde_json::to_value(&value).expect("serialize protocol value");
    assert_eq!(encoded, expected);
    let decoded: T = serde_json::from_value(encoded).expect("deserialize protocol value");
    assert_eq!(decoded, value);
}

#[test]
fn dap_envelopes_use_canonical_keys_and_legacy_input_only() {
    let request: Request<Value> = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "threads".to_string(),
        arguments: None,
    };
    assert_eq!(
        serde_json::to_value(request).expect("serialize request"),
        json!({"seq": 1, "type": "request", "command": "threads"})
    );

    let response: Response<Value> = Response {
        seq: 2,
        message_type: MessageType::Response,
        request_seq: 1,
        success: false,
        command: "threads".to_string(),
        message: Some("not ready".to_string()),
        body: None,
    };
    assert_eq!(
        serde_json::to_value(response).expect("serialize response"),
        json!({
            "seq": 2,
            "type": "response",
            "request_seq": 1,
            "success": false,
            "command": "threads",
            "message": "not ready"
        })
    );

    let event: Event<Value> = Event {
        seq: 3,
        message_type: MessageType::Event,
        event: "initialized".to_string(),
        body: None,
    };
    assert_eq!(
        serde_json::to_value(event).expect("serialize event"),
        json!({"seq": 3, "type": "event", "event": "initialized"})
    );

    let decoded: Response<Value> = serde_json::from_value(json!({
        "seq": 4,
        "type": "response",
        "requestSeq": 9,
        "success": true,
        "command": "initialize"
    }))
    .expect("legacy requestSeq alias must deserialize");
    assert_eq!(decoded.request_seq, 9);
    assert_eq!(decoded.message_type, MessageType::Response);

    assert_wire(MessageType::Request, json!("request"));
    assert_wire(MessageType::Response, json!("response"));
    assert_wire(MessageType::Event, json!("event"));
}

#[test]
fn custom_state_payloads_preserve_force_scan_frame_and_instance_identity() {
    let source = Source {
        name: Some("main.st".to_string()),
        path: Some("/workspace/main.st".to_string()),
        source_reference: Some(7),
    };
    assert_wire(
        StoppedEventBody {
            reason: "breakpoint".to_string(),
            thread_id: Some(1),
            all_threads_stopped: Some(true),
        },
        json!({"reason": "breakpoint", "threadId": 1, "allThreadsStopped": true}),
    );
    assert_wire(
        TerminatedEventBody {
            restart: Some(false),
        },
        json!({"restart": false}),
    );
    assert_wire(
        InvalidatedEventBody {
            areas: Some(vec!["variables".to_string()]),
            thread_id: Some(1),
            stack_frame_id: Some(11),
        },
        json!({"areas": ["variables"], "threadId": 1, "stackFrameId": 11}),
    );
    assert_wire(
        OutputEventBody {
            output: "cycle complete\n".to_string(),
            category: Some("console".to_string()),
            source: Some(source.clone()),
            line: Some(8),
            column: Some(3),
        },
        json!({
            "output": "cycle complete\n",
            "category": "console",
            "source": {"name": "main.st", "path": "/workspace/main.st", "sourceReference": 7},
            "line": 8,
            "column": 3
        }),
    );

    let entry = IoStateEntry {
        name: Some("MotorReady".to_string()),
        address: "%IX0.0".to_string(),
        source: Some("main.st".to_string()),
        value_type: Some("BOOL".to_string()),
        value: "TRUE".to_string(),
        forced: false,
        writable: Some(false),
    };
    assert_wire(
        IoStateEventBody {
            scan: Some(0),
            inputs: vec![entry.clone()],
            outputs: Vec::new(),
            memory: Vec::new(),
        },
        json!({
            "scan": 0,
            "inputs": [{
                "name": "MotorReady",
                "address": "%IX0.0",
                "source": "main.st",
                "valueType": "BOOL",
                "value": "TRUE",
                "forced": false,
                "writable": false
            }],
            "outputs": [],
            "memory": []
        }),
    );

    let var = VarStateEntry {
        name: "Counter".to_string(),
        value: "5".to_string(),
    };
    assert_wire(
        VarStateEventBody {
            locals: Vec::new(),
            globals: vec![var.clone()],
            instances: vec![VarStateInstance {
                id: 17,
                name: "Main".to_string(),
                vars: vec![var],
            }],
            retain: Vec::new(),
            frame_id: Some(0),
            paused: Some(false),
        },
        json!({
            "globals": [{"name": "Counter", "value": "5"}],
            "instances": [{
                "id": 17,
                "name": "Main",
                "vars": [{"name": "Counter", "value": "5"}]
            }],
            "retain": [],
            "frameId": 0,
            "paused": false
        }),
    );

    let default_entry: IoStateEntry =
        serde_json::from_value(json!({"address": "%QX0.0", "value": "FALSE"}))
            .expect("forced defaults to false");
    assert!(!default_entry.forced);
    let default_vars: VarStateEventBody =
        serde_json::from_value(json!({"globals": [], "retain": []}))
            .expect("locals and instances default empty");
    assert!(default_vars.locals.is_empty());
    assert!(default_vars.instances.is_empty());
}

#[test]
fn custom_mutation_payloads_use_required_fields_and_lowercase_discriminators() {
    assert_wire(
        IoWriteArguments {
            address: "%QW2".to_string(),
            value: "42".to_string(),
        },
        json!({"address": "%QW2", "value": "42"}),
    );
    assert_wire(
        IoReleaseArguments {
            address: "%QW2".to_string(),
        },
        json!({"address": "%QW2"}),
    );
    assert_wire(VarWriteAction::Write, json!("write"));
    assert_wire(VarWriteAction::Force, json!("force"));
    assert_wire(VarWriteAction::Release, json!("release"));
    assert_wire(VarWriteScope::Locals, json!("locals"));
    assert_wire(VarWriteScope::Globals, json!("globals"));
    assert_wire(VarWriteScope::Instances, json!("instances"));
    assert_wire(VarWriteScope::Retain, json!("retain"));
    assert_wire(
        VarWriteArguments {
            scope: VarWriteScope::Instances,
            name: "Drive.Speed".to_string(),
            value: Some("12.5".to_string()),
            action: Some(VarWriteAction::Force),
            instance_id: Some(3),
            frame_id: Some(0),
        },
        json!({
            "scope": "instances",
            "name": "Drive.Speed",
            "value": "12.5",
            "action": "force",
            "instanceId": 3,
            "frameId": 0
        }),
    );
    assert_wire(
        VarWriteArguments {
            scope: VarWriteScope::Globals,
            name: "EmergencyStop".to_string(),
            value: None,
            action: Some(VarWriteAction::Release),
            instance_id: None,
            frame_id: None,
        },
        json!({"scope": "globals", "name": "EmergencyStop", "action": "release"}),
    );
}

#[test]
fn standard_session_inspection_and_mutation_payloads_keep_dap_field_names() {
    assert_wire(InitializeArguments::default(), json!({}));
    let capabilities = Capabilities {
        supports_configuration_done_request: Some(true),
        supports_conditional_breakpoints: Some(false),
        supports_hit_conditional_breakpoints: None,
        supports_log_points: Some(true),
        supports_breakpoint_locations_request: Some(true),
        supports_function_breakpoints: None,
        supports_evaluate_for_hovers: Some(true),
        supports_set_variable: Some(true),
        supports_set_expression: Some(false),
        supports_pause_request: Some(true),
        supports_terminate_request: Some(true),
    };
    assert_wire(
        InitializeResponseBody {
            capabilities: capabilities.clone(),
        },
        json!({
            "supportsConfigurationDoneRequest": true,
            "supportsConditionalBreakpoints": false,
            "supportsLogPoints": true,
            "supportsBreakpointLocationsRequest": true,
            "supportsEvaluateForHovers": true,
            "supportsSetVariable": true,
            "supportsSetExpression": false,
            "supportsPauseRequest": true,
            "supportsTerminateRequest": true
        }),
    );
    assert_wire(
        capabilities,
        json!({
            "supportsConfigurationDoneRequest": true,
            "supportsConditionalBreakpoints": false,
            "supportsLogPoints": true,
            "supportsBreakpointLocationsRequest": true,
            "supportsEvaluateForHovers": true,
            "supportsSetVariable": true,
            "supportsSetExpression": false,
            "supportsPauseRequest": true,
            "supportsTerminateRequest": true
        }),
    );

    let additional = BTreeMap::from([
        ("program".to_string(), json!("main.st")),
        ("stopOnEntry".to_string(), json!(false)),
    ]);
    assert_wire(
        LaunchArguments {
            additional: additional.clone(),
        },
        json!({"program": "main.st", "stopOnEntry": false}),
    );
    assert_wire(
        AttachArguments { additional },
        json!({"program": "main.st", "stopOnEntry": false}),
    );
    assert_wire(
        DisconnectArguments {
            restart: Some(false),
            terminate_debuggee: Some(true),
        },
        json!({"restart": false, "terminateDebuggee": true}),
    );
    assert_wire(
        TerminateArguments {
            restart: Some(false),
        },
        json!({"restart": false}),
    );
    assert_wire(ContinueArguments { thread_id: 1 }, json!({"threadId": 1}));
    assert_wire(
        ContinueResponseBody {
            all_threads_continued: Some(false),
        },
        json!({"allThreadsContinued": false}),
    );
    assert_wire(PauseArguments { thread_id: 1 }, json!({"threadId": 1}));
    assert_wire(NextArguments { thread_id: 1 }, json!({"threadId": 1}));
    assert_wire(StepInArguments { thread_id: 1 }, json!({"threadId": 1}));
    assert_wire(StepOutArguments { thread_id: 1 }, json!({"threadId": 1}));

    assert_wire(
        EvaluateArguments {
            expression: "Motor.Speed".to_string(),
            frame_id: Some(0),
            context: Some("watch".to_string()),
        },
        json!({"expression": "Motor.Speed", "frameId": 0, "context": "watch"}),
    );
    assert_wire(
        EvaluateResponseBody {
            result: "12.5".to_string(),
            r#type: Some("REAL".to_string()),
            variables_reference: 0,
            named_variables: Some(0),
            indexed_variables: Some(0),
        },
        json!({
            "result": "12.5",
            "type": "REAL",
            "variablesReference": 0,
            "namedVariables": 0,
            "indexedVariables": 0
        }),
    );

    let source = Source {
        name: Some("main.st".to_string()),
        path: Some("/workspace/main.st".to_string()),
        source_reference: Some(0),
    };
    assert_wire(
        source.clone(),
        json!({
            "name": "main.st",
            "path": "/workspace/main.st",
            "sourceReference": 0
        }),
    );
    assert_wire(
        ThreadsResponseBody {
            threads: vec![Thread {
                id: 1,
                name: "MainTask".to_string(),
            }],
        },
        json!({"threads": [{"id": 1, "name": "MainTask"}]}),
    );
    assert_wire(
        StackTraceArguments {
            thread_id: 1,
            start_frame: Some(0),
            levels: Some(0),
        },
        json!({"threadId": 1, "startFrame": 0, "levels": 0}),
    );
    let frame = StackFrame {
        id: 0,
        name: "Main".to_string(),
        source: Some(source.clone()),
        line: 1,
        column: 1,
        end_line: Some(1),
        end_column: Some(8),
    };
    assert_wire(
        StackTraceResponseBody {
            stack_frames: vec![frame],
            total_frames: Some(1),
        },
        json!({
            "stackFrames": [{
                "id": 0,
                "name": "Main",
                "source": {
                    "name": "main.st",
                    "path": "/workspace/main.st",
                    "sourceReference": 0
                },
                "line": 1,
                "column": 1,
                "endLine": 1,
                "endColumn": 8
            }],
            "totalFrames": 1
        }),
    );
    assert_wire(ScopesArguments { frame_id: 0 }, json!({"frameId": 0}));
    assert_wire(
        ScopesResponseBody {
            scopes: vec![Scope {
                name: "Globals".to_string(),
                variables_reference: 9,
                expensive: false,
                source: Some(source),
                line: Some(1),
                column: Some(1),
                end_line: None,
                end_column: None,
            }],
        },
        json!({
            "scopes": [{
                "name": "Globals",
                "variablesReference": 9,
                "expensive": false,
                "source": {
                    "name": "main.st",
                    "path": "/workspace/main.st",
                    "sourceReference": 0
                },
                "line": 1,
                "column": 1
            }]
        }),
    );
    assert_wire(
        VariablesArguments {
            variables_reference: 9,
        },
        json!({"variablesReference": 9}),
    );
    assert_wire(
        VariablesResponseBody {
            variables: vec![Variable {
                name: "Counter".to_string(),
                value: "5".to_string(),
                r#type: Some("DINT".to_string()),
                variables_reference: 0,
                evaluate_name: Some("Globals.Counter".to_string()),
            }],
        },
        json!({
            "variables": [{
                "name": "Counter",
                "value": "5",
                "type": "DINT",
                "variablesReference": 0,
                "evaluateName": "Globals.Counter"
            }]
        }),
    );
    assert_wire(
        SetVariableArguments {
            variables_reference: 9,
            name: "Counter".to_string(),
            value: "6".to_string(),
        },
        json!({"variablesReference": 9, "name": "Counter", "value": "6"}),
    );
    assert_wire(
        SetVariableResponseBody {
            value: "6".to_string(),
            r#type: Some("DINT".to_string()),
            variables_reference: 0,
            named_variables: Some(0),
            indexed_variables: Some(0),
        },
        json!({
            "value": "6",
            "type": "DINT",
            "variablesReference": 0,
            "namedVariables": 0,
            "indexedVariables": 0
        }),
    );
    assert_wire(
        SetExpressionArguments {
            expression: "Globals.Counter".to_string(),
            value: "7".to_string(),
            frame_id: Some(0),
        },
        json!({"expression": "Globals.Counter", "value": "7", "frameId": 0}),
    );
    assert_wire(
        SetExpressionResponseBody {
            value: "7".to_string(),
            r#type: Some("DINT".to_string()),
            variables_reference: 0,
            named_variables: None,
            indexed_variables: None,
        },
        json!({"value": "7", "type": "DINT", "variablesReference": 0}),
    );
}

#[test]
fn breakpoint_and_reload_payloads_preserve_positions_and_optional_inputs() {
    let source = Source {
        name: Some("main.st".to_string()),
        path: Some("/workspace/main.st".to_string()),
        source_reference: None,
    };
    let requested = SourceBreakpoint {
        line: 0,
        column: Some(0),
        condition: Some("Counter > 5".to_string()),
        hit_condition: Some("== 3".to_string()),
        log_message: Some("Counter={Counter}".to_string()),
    };
    assert_wire(
        requested.clone(),
        json!({
            "line": 0,
            "column": 0,
            "condition": "Counter > 5",
            "hitCondition": "== 3",
            "logMessage": "Counter={Counter}"
        }),
    );

    let verified = Breakpoint::verified(0, 0, Some(source.clone()));
    assert_wire(
        verified.clone(),
        json!({
            "verified": true,
            "source": {"name": "main.st", "path": "/workspace/main.st"},
            "line": 0,
            "column": 0
        }),
    );
    let unverified = Breakpoint::unverified(
        0,
        None,
        Some(source.clone()),
        Some("no statement".to_string()),
    );
    assert_wire(
        unverified,
        json!({
            "verified": false,
            "message": "no statement",
            "source": {"name": "main.st", "path": "/workspace/main.st"},
            "line": 0
        }),
    );
    assert_wire(
        BreakpointEventBody {
            reason: "changed".to_string(),
            breakpoint: verified.clone(),
        },
        json!({
            "reason": "changed",
            "breakpoint": {
                "verified": true,
                "source": {"name": "main.st", "path": "/workspace/main.st"},
                "line": 0,
                "column": 0
            }
        }),
    );
    assert_wire(
        ReloadArguments {
            program: Some("config.st".to_string()),
            runtime_include_globs: Some(vec!["src/**/*.st".to_string()]),
            runtime_exclude_globs: Some(vec!["generated/**".to_string()]),
            runtime_ignore_pragmas: Some(vec!["vendor".to_string()]),
            runtime_root: Some("/workspace".to_string()),
        },
        json!({
            "program": "config.st",
            "runtimeIncludeGlobs": ["src/**/*.st"],
            "runtimeExcludeGlobs": ["generated/**"],
            "runtimeIgnorePragmas": ["vendor"],
            "runtimeRoot": "/workspace"
        }),
    );
    assert_wire(
        SetBreakpointsArguments {
            source: source.clone(),
            breakpoints: Some(vec![requested]),
            lines: Some(vec![0]),
            source_modified: Some(false),
        },
        json!({
            "source": {"name": "main.st", "path": "/workspace/main.st"},
            "breakpoints": [{
                "line": 0,
                "column": 0,
                "condition": "Counter > 5",
                "hitCondition": "== 3",
                "logMessage": "Counter={Counter}"
            }],
            "lines": [0],
            "sourceModified": false
        }),
    );
    let location = BreakpointLocation {
        line: 0,
        column: Some(0),
        end_line: Some(0),
        end_column: Some(8),
    };
    assert_wire(
        BreakpointLocationsArguments {
            source,
            line: 0,
            column: Some(0),
            end_line: Some(0),
            end_column: Some(8),
        },
        json!({
            "source": {"name": "main.st", "path": "/workspace/main.st"},
            "line": 0,
            "column": 0,
            "endLine": 0,
            "endColumn": 8
        }),
    );
    assert_wire(
        BreakpointLocationsResponseBody {
            breakpoints: vec![location],
        },
        json!({
            "breakpoints": [{"line": 0, "column": 0, "endLine": 0, "endColumn": 8}]
        }),
    );
    assert_wire(
        SetBreakpointsResponseBody {
            breakpoints: vec![verified],
        },
        json!({
            "breakpoints": [{
                "verified": true,
                "source": {"name": "main.st", "path": "/workspace/main.st"},
                "line": 0,
                "column": 0
            }]
        }),
    );
}

#[test]
fn missing_required_fields_and_invalid_discriminators_fail_closed() {
    macro_rules! assert_rejected {
        ($ty:ty, $value:expr) => {
            assert!(
                serde_json::from_value::<$ty>($value).is_err(),
                "{} unexpectedly accepted malformed JSON",
                stringify!($ty)
            );
        };
    }

    assert_rejected!(Request<Value>, json!({"seq": 1, "type": "request"}));
    assert_rejected!(
        Response<Value>,
        json!({
            "seq": 1,
            "type": "response",
            "request_seq": 1,
            "command": "threads"
        })
    );
    assert_rejected!(Event<Value>, json!({"seq": 1, "type": "event"}));
    assert_rejected!(IoStateEventBody, json!({"inputs": [], "outputs": []}));
    assert_rejected!(VarStateEventBody, json!({"globals": []}));
    assert_rejected!(IoWriteArguments, json!({"address": "%QX0.0"}));
    assert_rejected!(IoReleaseArguments, json!({}));
    assert_rejected!(VarWriteArguments, json!({"scope": "globals"}));
    assert_rejected!(ContinueArguments, json!({}));
    assert_rejected!(EvaluateArguments, json!({"frameId": 0}));
    assert_rejected!(Thread, json!({"name": "MainTask"}));
    assert_rejected!(StackFrame, json!({"id": 0, "name": "Main", "line": 1}));
    assert_rejected!(Scope, json!({"name": "Globals", "variablesReference": 1}));
    assert_rejected!(
        Variable,
        json!({"name": "Counter", "variablesReference": 0})
    );
    assert_rejected!(SourceBreakpoint, json!({}));
    assert_rejected!(Breakpoint, json!({"line": 1}));
    assert_rejected!(SetBreakpointsArguments, json!({}));
    assert_rejected!(BreakpointLocationsArguments, json!({"source": {}}));
    assert_rejected!(MessageType, json!("Request"));
    assert_rejected!(VarWriteAction, json!("FORCE"));
    assert_rejected!(VarWriteScope, json!("instance"));
}
