//! The `serve` front end of the `trust-control-studio` program (specification 35, §0.1a) on a copy of
//! the PID commissioning example: framed JSON-RPC requests on standard input, framed answers on
//! standard output, and a clean end when standard input closes.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_control_studio::rpc;

const EXAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control-pid-commissioning"
);
const LOOP_ID: &str = "control-pid-commissioning";
/// How long the engine may take to end once its input is closed.
const END_DEADLINE: Duration = Duration::from_secs(10);

/// A folder of the test, removed when the test ends, whether or not its assertions held.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The engine process. Dropping it stops the process if it is still running.
struct Engine(Child);

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create the copy");
    for entry in fs::read_dir(from).expect("read the example") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("its type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy a file");
        }
    }
}

/// Writes one framed message to the engine.
fn send(input: &mut ChildStdin, body: &[u8]) {
    write!(input, "Content-Length: {}\r\n\r\n", body.len()).expect("the header is written");
    input.write_all(body).expect("the body is written");
    input.flush().expect("the message is sent");
}

/// Reads the engine's next framed answer.
fn read_answer(output: &mut impl BufRead) -> Value {
    let body = rpc::read_message(output)
        .expect("a framed answer")
        .expect("an answer before the end of the output");
    serde_json::from_slice(&body).expect("the answer is JSON")
}

#[test]
fn serve_answers_each_message_and_ends_when_its_input_closes() {
    let scratch =
        Scratch(std::env::temp_dir().join(format!("trust-engine-serve-{}", std::process::id())));
    let _ = fs::remove_dir_all(&scratch.0);
    let project = scratch.0.join("project");
    copy_dir(Path::new(EXAMPLE), &project);
    let journal = scratch.0.join("journal");

    let mut engine = Engine(
        Command::new(env!("CARGO_BIN_EXE_trust-control-studio"))
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the engine starts"),
    );
    let mut input = engine.0.stdin.take().expect("standard input");
    let mut output = BufReader::new(engine.0.stdout.take().expect("standard output"));

    let open = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "controlStudio/open",
        "params": {
            "projectRoot": project.display().to_string(),
            "runtimeId": "simulator",
            "storageFolder": journal.display().to_string(),
        },
    });
    send(&mut input, open.to_string().as_bytes());
    let opened = read_answer(&mut output);
    assert_eq!(opened["id"], json!(1), "{opened}");
    assert_eq!(
        opened["result"]["result"]["kind"],
        json!("loops"),
        "{opened}"
    );
    assert_eq!(
        opened["result"]["result"]["loops"][0]["loopId"],
        json!(LOOP_ID),
        "{opened}"
    );

    send(
        &mut input,
        br#"{"jsonrpc":"2.0","id":2,"method":"controlStudio/nothing"}"#,
    );
    let unknown = read_answer(&mut output);
    assert_eq!(unknown["id"], json!(2), "{unknown}");
    assert_eq!(unknown["error"]["code"], json!(-32601), "{unknown}");

    send(&mut input, b"{not json");
    let malformed = read_answer(&mut output);
    assert_eq!(malformed["id"], Value::Null, "{malformed}");
    assert_eq!(malformed["error"]["code"], json!(-32700), "{malformed}");

    send(
        &mut input,
        br#"{"id":4,"method":"controlStudio/status","params":{"operationId":"operation-1"}}"#,
    );
    let invalid = read_answer(&mut output);
    assert_eq!(invalid["id"], json!(4), "{invalid}");
    assert_eq!(invalid["error"]["code"], json!(-32600), "{invalid}");

    drop(input);
    let deadline = Instant::now() + END_DEADLINE;
    let status = loop {
        if let Some(status) = engine.0.try_wait().expect("the engine can be polled") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "the engine did not end when its input closed"
        );
        sleep(Duration::from_millis(20));
    };
    assert!(
        status.success(),
        "the engine ends with status 0, not {status}"
    );
}
