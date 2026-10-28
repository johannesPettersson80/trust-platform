//! A scripted runtime for the tests. It accepts one connection on a local TCP port and answers
//! the first request with the first prepared line, the second with the second, and so on. Once
//! the lines run out it reads on without answering, until the client closes the connection.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};

use serde_json::Value;

/// A running fake runtime.
pub(crate) struct FakeRuntime {
    /// Its control endpoint, `tcp://127.0.0.1:<port>`.
    pub(crate) endpoint: String,
    server: JoinHandle<Vec<Value>>,
}

impl FakeRuntime {
    /// The requests that the runtime received. Waits until the client has closed the connection.
    pub(crate) fn requests(self) -> Vec<Value> {
        self.server
            .join()
            .expect("the fake runtime ends without a panic")
    }
}

/// Starts a fake runtime that writes each of `lines` as one answer, followed by a newline.
pub(crate) fn answering(lines: Vec<String>) -> FakeRuntime {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("local address"));
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept the client");
        let mut writer = stream.try_clone().expect("clone the connection");
        let mut reader = BufReader::new(stream);
        let mut lines = lines.into_iter();
        let mut received: Vec<Value> = Vec::new();
        let mut request = String::new();
        while reader.read_line(&mut request).is_ok_and(|read| read > 0) {
            received.push(serde_json::from_str(&request).expect("a JSON request line"));
            request.clear();
            if let Some(line) = lines.next() {
                // The client may already have given up on this answer.
                let _ = writer.write_all(format!("{line}\n").as_bytes());
            }
        }
        received
    });
    FakeRuntime { endpoint, server }
}
