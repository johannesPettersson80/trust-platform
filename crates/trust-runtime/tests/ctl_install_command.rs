//! `trust-runtime ctl install` and the `serial://` endpoint as a user runs them (specification 11
//! §6.8.1; specification 34 §11.2, STATE-08 and PORT-05): the real binary against a scripted
//! runtime on a loopback port, against the real `trust-runtime` of a computer, and against a port
//! that does not exist. No test opens a serial port of this computer.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Output;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{json, Value};
use trust_runtime_services::transfer::{wire, Complete, Received, TransferSession};

use runtime_process::{describe, trust_runtime_command};

static FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// A file in the temporary folder that holds `bytes`, for a test to hand to the command.
fn container_file(bytes: &[u8]) -> PathBuf {
    let sequence = FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "trust-ctl-install-{}-{sequence}.trsta",
        std::process::id()
    ));
    fs::write(&path, bytes).expect("write the container file");
    path
}

/// A container of 10,000 bytes: two parts of 4,096 bytes and a third of 1,808.
fn container() -> Vec<u8> {
    (0..10_000_u32)
        .map(|index| u8::try_from(index % 251).expect("a remainder below 251 is a byte"))
        .collect()
}

/// What a scripted runtime saw: its requests and the container it installed.
struct Seen {
    requests: Vec<Value>,
    installed: Option<Vec<u8>>,
}

/// A stopped runtime on a loopback port that serves one connection with the services' own
/// transfer session. It answers `program.install` with the identity sequence 3, an SHA-256 of
/// sixty-four ones and 777 bytes, `restart` with an empty result and `status` with `running`.
fn spawn_runtime(token: &'static str) -> (String, JoinHandle<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let endpoint = format!(
        "tcp://{}",
        listener.local_addr().expect("the port's address")
    );
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept the command");
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("bound the reads");
        let mut reader = BufReader::new(stream.try_clone().expect("clone the connection"));
        let mut writer = stream;
        let mut session = TransferSession::new(1, 1 << 20, 1 << 20);
        let mut seen = Seen {
            requests: Vec::new(),
            installed: None,
        };
        let mut line = String::new();
        while reader.read_line(&mut line).expect("read a request") > 0 {
            let request: Value = serde_json::from_str(&line).expect("a JSON request line");
            line.clear();
            seen.requests.push(request.clone());
            assert_eq!(
                request["auth"],
                json!(token),
                "every request carries the token"
            );
            let params = request.get("params").cloned().unwrap_or(Value::Null);
            let result = match request["type"].as_str().expect("a request type") {
                "transfer.begin" => {
                    let begin = wire::begin_request(&params).expect("a readable begin");
                    session
                        .begin(begin.bytes, begin.digest)
                        .map(wire::begin_reply)
                        .map_err(|error| error.to_string())
                }
                "transfer.part" => {
                    let part = wire::part_request(&params).expect("a readable part");
                    session
                        .part(part.transfer, part.offset, part.encoded, part.crc32)
                        .map(|received| wire::part_reply(next_offset(received)))
                        .map_err(|error| error.to_string())
                }
                "program.install" => {
                    let transfer = wire::transfer_request(&params).expect("a transfer id");
                    session
                        .take_complete(transfer)
                        .map(memory_bytes)
                        .map(|container| {
                            seen.installed = Some(container);
                            json!({
                                "status": "installed",
                                "sequence": 3,
                                "program_sha256": "1".repeat(64),
                                "bytes": 777,
                            })
                        })
                        .map_err(|error| error.to_string())
                }
                "restart" => Ok(json!({})),
                "status" => Ok(json!({"state": "running"})),
                other => panic!("the runtime was asked for {other}"),
            };
            let reply = match result {
                Ok(result) => json!({"id": request["id"], "ok": true, "result": result}),
                Err(error) => json!({"id": request["id"], "ok": false, "error": error}),
            };
            writeln!(writer, "{reply}").expect("write a reply");
        }
        seen
    });
    (endpoint, server)
}

/// Runs `trust-runtime ctl <args>` without a token from the environment.
fn run_ctl(args: &[&str]) -> Output {
    trust_runtime_command()
        .arg("ctl")
        .args(args)
        .env_remove("TRUST_CTL_TOKEN")
        .output()
        .expect("run trust-runtime ctl")
}

#[test]
fn ctl_install_over_loopback_tcp_prints_the_identity_and_starts_the_program() {
    // Specification 11 6.8.1: `ctl install <container> --restart cold` uploads the file in parts
    // of 4,096 bytes with the token on every request, installs it, prints the identity that the
    // reply gives (sequence 3, the SHA-256 of sixty-four ones and 777 bytes here, so none of it
    // is worked out by the command), sends `restart`, asks `status` and prints the state. The
    // runtime's session checks every part's CRC-32 and the SHA-256 of the file. Wrong behaviour:
    // an option after the action is refused, a part is lost or reordered, the identity is not the
    // reply's, or the restart is not waited for.
    let container = container();
    let file = container_file(&container);
    let (endpoint, server) = spawn_runtime("tcp-token");

    let output = run_ctl(&[
        "install",
        "--endpoint",
        &endpoint,
        "--token",
        "tcp-token",
        file.to_str().expect("the temporary path is text"),
        "--restart",
        "cold",
    ]);

    assert!(output.status.success(), "{}", describe(&output));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "installed sequence=3 program_sha256={} bytes=777\nrestarted mode=cold state=running\n",
            "1".repeat(64)
        )
    );
    let seen = server.join().expect("the scripted runtime ends cleanly");
    assert_eq!(seen.installed, Some(container));
    let types: Vec<&str> = seen
        .requests
        .iter()
        .map(|request| request["type"].as_str().expect("a request type"))
        .collect();
    assert_eq!(
        types,
        [
            "transfer.begin",
            "transfer.part",
            "transfer.part",
            "transfer.part",
            "program.install",
            "restart",
            "status"
        ]
    );
    assert_eq!(seen.requests[5]["params"], json!({"mode": "cold"}));
    let _ = fs::remove_file(file);
}

#[test]
fn ctl_over_a_serial_endpoint_reports_a_port_that_cannot_be_opened() {
    // Specification 11 6.8.1: `serial://<port>` is an endpoint of `ctl`. The path below does not
    // exist, so the command reaches the opening of the port and says which port it could not open;
    // it neither hangs nor touches a device of this computer. Wrong behaviour: the scheme is
    // refused as an endpoint the command does not know, or the port is not named.
    let output = run_ctl(&[
        "--endpoint",
        "serial:///nonexistent/trust-test-port",
        "status",
    ]);

    assert!(!output.status.success(), "{}", describe(&output));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot open the serial port /nonexistent/trust-test-port"),
        "{stderr}"
    );
    assert!(!stderr.contains("unsupported endpoint"), "{stderr}");
}

/// The example the computer test installs a second program into.
#[cfg(unix)]
const EXAMPLE: &str = "memory_marker_counter";

#[cfg(unix)]
mod computer {
    use sha2::{Digest, Sha256};
    use trust_runtime_services::installation::encode_sequenced;

    use super::{container_file, describe, fs, EXAMPLE};
    use crate::runtime_process::{build_program, ExampleRuntime, Runtime, CLIMB, WAIT};

    /// The statement of the example that climbs `%MW0` by one in each scan, and the one that
    /// climbs it by two: the second program of the test.
    const STEP_ONE: &str = "Counter := Counter + 1;";
    const STEP_TWO: &str = "Counter := Counter + 2;";
    /// The scan count that the test waits for before it stops the program.
    const SCANS_BEFORE_STOP: u64 = 40;

    /// Proves that the running program climbs `%MW0` by `step` in each scan, by two snapshots of
    /// one run, after the scan counter has fallen below `stale`, which only a new run does.
    fn assert_climbs_by(runtime: &Runtime, stale: u64, step: i64) {
        let first =
            runtime.wait_for_snapshot(WAIT, "a snapshot of the new run", |s| s.scan() < stale);
        let second = runtime.wait_for_snapshot(WAIT, "later scans of the new run", |s| {
            s.scan() >= first.scan() + 3
        });
        let scans = i64::try_from(second.scan() - first.scan()).expect("a few scans");
        assert_eq!(
            second.int("%MW0") - first.int("%MW0"),
            step * scans,
            "the counter climbs by {step} in each scan"
        );
    }

    #[test]
    fn ctl_install_installs_a_program_into_a_stopped_computer_runtime() {
        // Specification 11 6.8.1 and specification 34 §11.2 and STATE-08: on a computer, as on a
        // chip, `ctl install` uploads the container to the stopped runtime and installs it, prints
        // the identity that `program.install` gives (sequence 1 for the first install into a
        // program store, the SHA-256 and the length of the program), and `--restart cold` starts
        // the new program, which then climbs the counter by two in each scan where the first
        // climbed by one. The real trust-runtime takes the upload and writes its store. Wrong
        // behaviour: the install is refused or lost, the identity is not the program's, or the
        // restart does not start the installed program.
        let runtime = ExampleRuntime::new(EXAMPLE).store().start();
        let before = runtime
            .wait_for_snapshot(CLIMB, "the counter past the stop mark", |s| {
                s.scan() >= SCANS_BEFORE_STOP
            })
            .scan();
        runtime.ctl_ok(&["shutdown"]);
        runtime.wait_until(WAIT, "the stopped state", || {
            runtime.status().state == "stopped"
        });
        let step_two = build_program(EXAMPLE, &[("src/Main.st", STEP_ONE, STEP_TWO)]);
        let container = encode_sequenced(1, &step_two.program, &step_two.record)
            .expect("encode the container of the second program");
        let file = container_file(&container);

        let output = runtime.ctl(&[
            "install",
            file.to_str().expect("the temporary path is text"),
            "--restart",
            "cold",
        ]);

        assert!(output.status.success(), "{}", describe(&output));
        let digest: [u8; 32] = Sha256::digest(&step_two.program).into();
        let program_sha256 = hex::encode(digest);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!(
                "installed sequence=1 program_sha256={program_sha256} bytes={}\n\
                 restarted mode=cold state=running\n",
                step_two.program.len()
            )
        );
        assert_climbs_by(&runtime, before, 2);
        let _ = fs::remove_file(file);
    }
}

/// The next offset of a part that an in-memory session took.
fn next_offset(received: Received) -> usize {
    let Received::Next(next) = received else {
        panic!("an upload held in memory takes each part at once");
    };
    next
}

/// The bytes of an upload that an in-memory session held.
fn memory_bytes(complete: Complete) -> Vec<u8> {
    let Complete::Memory(bytes) = complete else {
        panic!("an upload held in memory completes in memory");
    };
    bytes
}
