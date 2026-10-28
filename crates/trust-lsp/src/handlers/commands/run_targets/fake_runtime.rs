//! A runtime's control endpoint for the tests: a TCP listener on a loopback port that answers each
//! request line it reads with the line its responder gives, and records the connections it accepted
//! and the request lines it read.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use serde_json::Value;

/// The responder of a fake runtime: the answer line for a request, `None` to close the connection
/// without one.
type Responder = dyn Fn(&Value) -> Option<String> + Send + Sync;

/// A fake runtime listening on a loopback port.
pub(super) struct FakeRuntime {
    /// Its control endpoint, `tcp://127.0.0.1:<port>`.
    pub(super) endpoint: String,
    connections: Arc<Mutex<usize>>,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl FakeRuntime {
    /// A runtime that answers each request with `respond`.
    pub(super) fn start(
        respond: impl Fn(&Value) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let endpoint = format!(
            "tcp://{}",
            listener.local_addr().expect("the listener's address")
        );
        let connections = Arc::new(Mutex::new(0));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let respond: Arc<Responder> = Arc::new(respond);
        let (counted, read) = (Arc::clone(&connections), Arc::clone(&requests));
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else {
                    continue;
                };
                *counted.lock().expect("the count") += 1;
                let respond = Arc::clone(&respond);
                let read = Arc::clone(&read);
                std::thread::spawn(move || {
                    let mut writer = stream.try_clone().expect("the stream's writer");
                    for line in BufReader::new(stream).lines() {
                        let Ok(line) = line else {
                            return;
                        };
                        let request: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
                        read.lock().expect("the requests").push(request.clone());
                        let Some(answer) = respond(&request) else {
                            return;
                        };
                        if writer.write_all(format!("{answer}\n").as_bytes()).is_err() {
                            return;
                        }
                    }
                });
            }
        });
        Self {
            endpoint,
            connections,
            requests,
        }
    }

    /// A runtime that answers every request `ok` with `result`.
    pub(super) fn answering(result: Value) -> Self {
        Self::start(move |request| {
            Some(
                serde_json::json!({ "id": request["id"], "ok": true, "result": result })
                    .to_string(),
            )
        })
    }

    /// The connections it accepted so far.
    pub(super) fn connections(&self) -> usize {
        *self.connections.lock().expect("the count")
    }

    /// The request lines it read so far.
    pub(super) fn requests(&self) -> Vec<Value> {
        self.requests.lock().expect("the requests").clone()
    }
}

/// An endpoint where nothing listens: a loopback port that was free a moment ago.
pub(super) fn closed_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let address = listener.local_addr().expect("the listener's address");
    drop(listener);
    format!("tcp://{address}")
}
