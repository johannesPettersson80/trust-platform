//! The `variables` paging of an attach session: the adapter passes VS Code's `filter`, `start` and
//! `count` to the runtime's `debug.variables` and reports the child counts of the runtime's reply
//! (specification 13, "Variables / Evaluate"; specification 11 §6.13.3, "Variable pages"). A fake
//! runtime on the loopback records the control requests and answers each with the same page.

use super::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;

/// The page the fake runtime answers every `debug.variables` with: a structure that holds two
/// members, a scalar, and an array of three elements, as a runtime lists them.
fn page_reply() -> Value {
    json!([
        {
            "name": "[200]",
            "value": "Point {...}",
            "type": "Point",
            "variablesReference": 41,
            "evaluateName": "[200]",
            "namedVariables": 2
        },
        {
            "name": "[201]",
            "value": "7",
            "type": "DINT",
            "variablesReference": 0,
            "evaluateName": "[201]"
        },
        {
            "name": "[202]",
            "value": "[3]",
            "type": "ARRAY",
            "variablesReference": 42,
            "evaluateName": "[202]",
            "indexedVariables": 3
        }
    ])
}

/// A control server that records every request line and answers a `debug.variables` with
/// [`page_reply`].
fn spawn_paging_runtime() -> (SocketAddr, Arc<Mutex<Vec<Value>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind the fake runtime");
    let addr = listener.local_addr().expect("the fake runtime's address");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&requests);
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("the adapter connects");
        let mut reader = BufReader::new(stream.try_clone().expect("clone the stream"));
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let request: Value = serde_json::from_str(&line).expect("a control request");
            let reply = json!({
                "id": request["id"].clone(),
                "ok": true,
                "result": {"variables": page_reply()}
            });
            seen.lock().expect("requests lock").push(request);
            writeln!(stream, "{reply}").expect("write the control reply");
        }
    });
    (addr, requests, handle)
}

/// A `variables` request with the arguments `arguments`, as VS Code sends them.
fn variables_request(seq: u32, arguments: Value) -> Request<Value> {
    Request {
        seq,
        message_type: MessageType::Request,
        command: "variables".to_string(),
        arguments: Some(arguments),
    }
}

#[test]
fn the_adapter_passes_vs_codes_paging_through() {
    // Specification 13, "Variables / Evaluate": in an attach session the adapter passes `filter`,
    // `start` and `count` of a `variables` request unchanged to the runtime's `debug.variables`,
    // leaves out an argument the client left out (a `start` of 0 is a value and is sent), refuses
    // an unsupported `filter` without asking the runtime, and reports the runtime's
    // `indexedVariables` and `namedVariables` to the client. Worked from VS Code's behaviour for an
    // array of 250 elements held by handle 7: it shows the chunks `[0..99]`, `[100..199]` and
    // `[200..249]` and asks for the last as indexed children 200 to 249, for the first as indexed
    // children 0 to 99, and for the named members of a structure element (handle 41) with a filter
    // alone; a client that does not page sends the handle alone.
    // Wrong behaviour caught: a request that drops the paging, so that the runtime answers its first
    // page for every chunk; a zero `start` dropped; an absent argument sent as `null`; counts lost
    // from the reply, so that VS Code shows no chunks; and an unsupported `filter` forwarded.
    let (addr, requests, server) = spawn_paging_runtime();
    let runtime = trust_runtime::RuntimeBuilder::new();
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime.build().expect("a runtime")));
    adapter.remote_session = Some(
        super::super::remote::RemoteSession::connect(
            super::super::remote::RemoteEndpoint::Tcp(addr),
            None,
        )
        .expect("the adapter connects to the fake runtime"),
    );

    let asked = [
        json!({"variablesReference": 7, "filter": "indexed", "start": 200, "count": 50}),
        json!({"variablesReference": 7, "filter": "indexed", "start": 0, "count": 100}),
        json!({"variablesReference": 41, "filter": "named"}),
        json!({"variablesReference": 41}),
    ];
    for (offset, arguments) in (1_u32..).zip(asked) {
        let outcome = adapter.dispatch_request(variables_request(offset, arguments));
        assert_eq!(outcome.responses.len(), 1);
        assert_eq!(outcome.responses[0]["success"], json!(true));
        assert_eq!(
            outcome.responses[0]["body"],
            json!({"variables": page_reply()}),
            "the counts of the runtime's page reach the client"
        );
    }
    let refused = adapter.dispatch_request(variables_request(
        5,
        json!({"variablesReference": 7, "filter": "sideways"}),
    ));
    assert_eq!(refused.responses[0]["success"], json!(false));
    assert_eq!(
        refused.responses[0]["message"],
        json!("invalid variables args")
    );

    drop(adapter);
    server.join().expect("the fake runtime stops");
    let sent = requests.lock().expect("requests lock").clone();
    let params = sent
        .iter()
        .map(|request| {
            assert_eq!(request["type"], json!("debug.variables"), "{request}");
            request["params"].clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        params,
        [
            json!({"variables_reference": 7, "filter": "indexed", "start": 200, "count": 50}),
            json!({"variables_reference": 7, "filter": "indexed", "start": 0, "count": 100}),
            json!({"variables_reference": 41, "filter": "named"}),
            json!({"variables_reference": 41}),
        ],
        "the refused request reached the runtime as a fifth"
    );
}
