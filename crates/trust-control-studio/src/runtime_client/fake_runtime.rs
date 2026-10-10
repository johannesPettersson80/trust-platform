//! Test support for the runtime operations of Control Studio: a fake runtime control endpoint and the loops of the
//! `TypeScript` tests. The fake answers each request line with what its handler returns, so an answer carries exactly
//! the fields the runtime protocol annex names (E0.1, E0.4), and it keeps every request it received. A handler that
//! returns `None` closes the connection without an answer: a lost acknowledgement (E11.4).

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{json, Value};
use trust_control::loop_definition::LoopDefinition;
use trust_control::runtime::RuntimeConnection;

/// The successful answer to `request` with `result` (E0.1).
#[must_use]
pub fn answer(request: &Value, result: Value) -> Value {
    let mut answer = json!({ "id": request["id"].clone(), "ok": true });
    answer["result"] = result;
    answer
}

/// The refusal of `request` with the runtime's `text` and `code` (E0.4).
#[must_use]
pub fn refusal(request: &Value, code: &str, text: &str) -> Value {
    json!({ "id": request["id"].clone(), "ok": false, "error": text, "error_code": code })
}

type Handler = dyn Fn(&Value) -> Option<Value> + Send + Sync;

/// A fake runtime on 127.0.0.1 and a free port.
pub struct FakeRuntime {
    endpoint: String,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl FakeRuntime {
    /// Starts a fake runtime that answers each request with `handler`.
    pub fn start(handler: impl Fn(&Value) -> Option<Value> + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let address = listener.local_addr().expect("the port");
        let handler: Arc<Handler> = Arc::new(handler);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let served = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let handler = Arc::clone(&handler);
                let served = Arc::clone(&served);
                thread::spawn(move || serve(stream, handler.as_ref(), &served));
            }
        });
        Self {
            endpoint: format!("tcp://{address}"),
            requests,
        }
    }

    /// The connection a client uses: this runtime's endpoint and the test's control token, when given.
    #[must_use]
    pub fn connection(&self, token: Option<&str>) -> RuntimeConnection {
        RuntimeConnection {
            endpoint: self.endpoint.clone(),
            token: token.map(str::to_string),
        }
    }

    /// Every request received so far, in arrival order.
    #[must_use]
    pub fn requests(&self) -> Vec<Value> {
        self.requests.lock().expect("the requests").clone()
    }

    /// The requests received for one operation, in arrival order.
    #[must_use]
    pub fn requests_of(&self, operation: &str) -> Vec<Value> {
        self.requests()
            .into_iter()
            .filter(|request| request["type"] == operation)
            .collect()
    }
}

fn serve(stream: TcpStream, handler: &Handler, served: &Mutex<Vec<Value>>) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        if !matches!(reader.read_line(&mut line), Ok(read) if read > 0) {
            return;
        }
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            return;
        };
        served.lock().expect("the requests").push(request.clone());
        let Some(reply) = handler(&request) else {
            return;
        };
        if writeln!(writer, "{reply}").is_err() {
            return;
        }
    }
}

/// The loop of the `TypeScript` tests' `loop()` fixture: a level with a setpoint, a measurement and a manipulated
/// variable, output limits of 0 and 100 and a 10 ms task and nominal period. It has no parameter binding.
#[must_use]
pub fn tank_loop() -> LoopDefinition {
    decode(json!({
        "schemaVersion": 2,
        "id": "loop:tank",
        "name": "Tank level",
        "revision": "sha256:loop-1",
        "projectId": "project:fixture",
        "runtimeId": "runtime:local",
        "controller": {
            "instance": "TankController",
            "type": "Control.PID",
            "revision": "sha256:controller-1",
            "action": "direct",
            "taskPeriodMs": 10.0
        },
        "signals": {
            "setpoint": {
                "symbol": "LevelSP", "address": "%ML0", "iecType": "LREAL",
                "direction": "read", "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            },
            "processVariable": {
                "symbol": "LevelPV", "address": "%IL0", "iecType": "LREAL",
                "direction": "read", "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            },
            "manipulatedVariable": {
                "symbol": "ValveMV", "address": "%QL0", "iecType": "LREAL",
                "direction": "write", "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            }
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0 },
        "observedSampleTiming": { "nominalPeriodMs": 10.0 }
    }))
}

/// The tank loop with a project step test whose hold is the declared parameter `P1.HoldSeconds` (the `TypeScript`
/// tests' `holdLoop()`), in a 45.1 s project-scripted experiment.
#[must_use]
pub fn hold_loop() -> LoopDefinition {
    let mut value = json!(tank_loop());
    value["experiment"] = json!({
        "execution": "project-scripted",
        "kind": "observation",
        "amplitude": 0.0,
        "durationSeconds": 45.1,
        "seed": 1,
        "holdParameter": "P1.HoldSeconds"
    });
    decode(value)
}

fn decode(value: Value) -> LoopDefinition {
    serde_json::from_value(value).expect("the fixture loop decodes")
}
