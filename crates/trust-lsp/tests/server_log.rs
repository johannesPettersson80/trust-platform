//! The language server's log (specification 14 §7.10): its own events at INFO, Salsa's query
//! events only when `RUST_LOG` names `salsa`.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

fn send(stdin: &mut impl Write, message: &Value) {
    let body = message.to_string();
    write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).expect("a write");
    stdin.flush().expect("a flush");
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

/// The log of a session with `RUST_LOG` set to `rust_log`, or unset: the server initializes,
/// lists the symbols of an open document, which runs its analysis, and is shut down.
fn log_of_a_session(rust_log: Option<&str>) -> String {
    let mut command = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-lsp")
            .expect("Cargo must provide trust-lsp binary while executing integration tests"),
    );
    command.env_remove("RUST_LOG");
    if let Some(directives) = rust_log {
        command.env("RUST_LOG", directives);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the trust-lsp binary");
    let mut stdin = child.stdin.take().expect("its stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("its stdout"));
    let uri = "file:///g275b/main.st";
    let text = "PROGRAM Main\nVAR\n    count : INT;\nEND_VAR\ncount := count + 1;\nEND_PROGRAM\n";
    let requests = [
        (
            1,
            "initialize",
            json!({ "processId": null, "rootUri": null, "capabilities": {} }),
        ),
        (
            2,
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": uri } }),
        ),
        (3, "shutdown", Value::Null),
    ];
    for (id, method, params) in requests {
        if id == 2 {
            send(
                &mut stdin,
                &json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }),
            );
            let document =
                json!({ "uri": uri, "languageId": "structured-text", "version": 1, "text": text });
            send(
                &mut stdin,
                &json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": { "textDocument": document } }),
            );
        }
        let mut request = json!({ "jsonrpc": "2.0", "id": id, "method": method });
        if !params.is_null() {
            request["params"] = params;
        }
        send(&mut stdin, &request);
        loop {
            let message = read_message(&mut stdout).expect("the server's answer");
            if message.get("method").is_some() {
                if let Some(asked) = message.get("id") {
                    send(
                        &mut stdin,
                        &json!({ "jsonrpc": "2.0", "id": asked, "result": null }),
                    );
                }
            } else if message.get("id") == Some(&json!(id)) {
                break;
            }
        }
    }
    send(&mut stdin, &json!({ "jsonrpc": "2.0", "method": "exit" }));
    let mut log = String::new();
    child
        .stderr
        .take()
        .expect("its stderr")
        .read_to_string(&mut log)
        .expect("the log");
    let _ = child.wait();
    log
}

#[test]
fn salsa_query_events_are_logged_only_when_rust_log_names_salsa() {
    // A document's analysis executes Salsa queries, which Salsa logs at INFO. Without `RUST_LOG`
    // the log holds the server's own INFO events and none of Salsa's; with `RUST_LOG=salsa=info`
    // it holds Salsa's too. Catches every query logged by default (thousands of lines per edit in
    // an editor's output) and a default that `RUST_LOG` cannot open again.
    let quiet = log_of_a_session(None);
    assert!(quiet.contains("Starting ST Language Server"), "{quiet}");
    assert!(!quiet.contains("salsa::"), "{quiet}");
    let traced = log_of_a_session(Some("salsa=info"));
    assert!(traced.contains("Starting ST Language Server"), "{traced}");
    assert!(traced.contains("salsa::"), "{traced}");
}
