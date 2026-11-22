//! The language server's lifecycle (specification 14 §7.9; LSP 3.17, "Shutdown Request" and
//! "Exit Notification"): `exit` ends the process at once, with status 0 after `shutdown` and 1
//! without it, while the client keeps the server's standard input open.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// How long the process may take to end after `exit`; it ends at once, so this only bounds a slow
/// runner.
const ENDS_WITHIN: Duration = Duration::from_secs(10);

/// The trust-lsp binary as a language server on stdio, its standard input kept open.
struct LanguageServer {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl LanguageServer {
    fn start() -> Self {
        let mut child = Command::new(
            std::env::var_os("CARGO_BIN_EXE_trust-lsp")
                .expect("Cargo must provide trust-lsp binary while executing integration tests"),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the trust-lsp binary");
        let stdin = child.stdin.take().expect("its stdin");
        let stdout = BufReader::new(child.stdout.take().expect("its stdout"));
        let mut server = Self {
            child,
            stdin,
            stdout,
        };
        server.request(
            1,
            "initialize",
            json!({ "processId": null, "rootUri": null, "capabilities": {} }),
        );
        server.send(&json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }));
        server
    }

    fn send(&mut self, message: &Value) {
        let body = message.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).expect("a write");
        self.stdin.flush().expect("a flush");
    }

    /// Sends a request with `params` (none when `Null`) and reads until its answer, answering the
    /// server's own requests with `null` and passing over its notifications.
    fn request(&mut self, id: i64, method: &str, params: Value) -> Value {
        let mut message = json!({ "jsonrpc": "2.0", "id": id, "method": method });
        if !params.is_null() {
            message["params"] = params;
        }
        self.send(&message);
        loop {
            let message = read_message(&mut self.stdout).expect("the server's answer");
            if message.get("method").is_some() {
                if let Some(request) = message.get("id") {
                    let answer = json!({ "jsonrpc": "2.0", "id": request, "result": null });
                    self.send(&answer);
                }
            } else if message.get("id") == Some(&json!(id)) {
                return message;
            }
        }
    }

    /// The process's status once it ended, within `limit`, its standard input still open.
    fn ended_within(&mut self, limit: Duration) -> Option<ExitStatus> {
        let started = Instant::now();
        while started.elapsed() < limit {
            if let Some(status) = self.child.try_wait().expect("the process's state") {
                return Some(status);
            }
            thread::sleep(Duration::from_millis(50));
        }
        None
    }
}

impl Drop for LanguageServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse().ok();
        }
    }
    let mut body = vec![0; length?];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

#[test]
fn exit_after_shutdown_ends_the_process_with_status_0_while_its_input_stays_open() {
    // LSP 3.17: after `shutdown` and `exit` the server ends with 0. The client here keeps the pipe
    // open, as a client may. Catches a server that waits for its standard input to close, which
    // tokio's uncancellable read of it made the server do, so that it never ended.
    let mut server = LanguageServer::start();
    let answer = server.request(2, "shutdown", Value::Null);
    assert!(answer.get("error").is_none(), "{answer}");
    server.send(&json!({ "jsonrpc": "2.0", "method": "exit" }));
    let status = server
        .ended_within(ENDS_WITHIN)
        .expect("the server ends after exit");
    assert_eq!(status.code(), Some(0));
}

#[test]
fn exit_without_shutdown_ends_the_process_with_status_1() {
    // LSP 3.17: `exit` without `shutdown` ends the server with 1. Catches 0 for an exit the client
    // did not prepare, and a server that ignores such an exit.
    let mut server = LanguageServer::start();
    server.send(&json!({ "jsonrpc": "2.0", "method": "exit" }));
    let status = server
        .ended_within(ENDS_WITHIN)
        .expect("the server ends after exit");
    assert_eq!(status.code(), Some(1));
}
