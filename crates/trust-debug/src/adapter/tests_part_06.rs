use super::*;

#[test]
fn dispatch_evaluate_returns_value() {
    let mut runtime = Runtime::new();
    let frame_id = runtime.storage_mut().push_frame("MAIN");
    runtime
        .storage_mut()
        .set_local("foo", RuntimeValue::Int(41));

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let eval_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "foo + 1".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(eval_req);
    let response: Response<EvaluateResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.success);
    let body = response.body.unwrap();
    assert_eq!(body.result, "42");
}

#[test]
fn dispatch_evaluate_rejects_calls() {
    let runtime = Runtime::new();
    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let eval_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "foo()".to_string(),
                frame_id: None,
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(eval_req);
    let response: Response<Value> = serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(!response.success);
}

#[test]
fn dispatch_evaluate_allows_pure_stdlib_calls() {
    let mut runtime = Runtime::new();
    let frame_id = runtime.storage_mut().push_frame("MAIN");

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let eval_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "ABS(-1)".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(eval_req);
    let response: Response<EvaluateResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.success);
    assert_eq!(response.body.unwrap().result, "1");
}

#[test]
fn dispatch_evaluate_resolves_instance_and_retain() {
    let mut runtime = Runtime::new();
    let instance_id = runtime.storage_mut().create_instance("MyFB");
    runtime
        .storage_mut()
        .set_instance_var(instance_id, "iv", RuntimeValue::Int(7));
    let frame_id = runtime
        .storage_mut()
        .push_frame_with_instance("METHOD", instance_id);
    runtime.storage_mut().set_retain("r", RuntimeValue::DInt(9));

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let eval_instance_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "iv".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let instance_outcome = adapter.dispatch_request(eval_instance_req);
    let instance_response: Response<EvaluateResponseBody> =
        serde_json::from_value(instance_outcome.responses[0].clone()).unwrap();
    assert!(instance_response.success);
    assert_eq!(instance_response.body.unwrap().result, "7");

    let eval_this_req = Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "THIS".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let this_outcome = adapter.dispatch_request(eval_this_req);
    let this_response: Response<EvaluateResponseBody> =
        serde_json::from_value(this_outcome.responses[0].clone()).unwrap();
    assert!(this_response.success);
    assert_eq!(
        this_response.body.unwrap().result,
        format!("Instance({})", instance_id.0)
    );

    let eval_retain_req = Request {
        seq: 3,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "r".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let retain_outcome = adapter.dispatch_request(eval_retain_req);
    let retain_response: Response<EvaluateResponseBody> =
        serde_json::from_value(retain_outcome.responses[0].clone()).unwrap();
    assert!(retain_response.success);
    assert_eq!(retain_response.body.unwrap().result, "9");
}

#[test]
fn dispatch_evaluate_honors_using_for_types() {
    let mut runtime = Runtime::new();
    let type_name = SmolStr::new("UTIL.MYINT");
    runtime.registry_mut().register(
        type_name.clone(),
        Type::Alias {
            name: type_name,
            target: TypeId::INT,
        },
    );
    runtime
        .register_program(ProgramDef {
            name: SmolStr::new("MAIN"),
            vars: Vec::new(),
            temps: Vec::new(),
            using: vec![SmolStr::new("UTIL")],
            body: Vec::new(),
        })
        .unwrap();
    let frame_id = runtime.storage_mut().push_frame("MAIN");

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let eval_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "SIZEOF(MYINT)".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(eval_req);
    let response: Response<EvaluateResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.success);
    assert_eq!(response.body.unwrap().result, "2");
}
