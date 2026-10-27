use super::*;
use crate::cli::FleetRuntimeTemplateArg;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);
const STATUS_OK: &str = r#"{"id":1,"ok":true,"result":{}}"#;

struct FleetFixture {
    root: PathBuf,
    listener: TcpListener,
}

impl FleetFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fleet-stop-confirmation-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create unique fixture directory");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind control endpoint");
        let web = TcpListener::bind("127.0.0.1:0").expect("reserve distinct web endpoint");
        let control_port = listener.local_addr().expect("control address").port();
        let web_port = web.local_addr().expect("web address").port();
        // Lifecycle tests own an already-running peer. Scaffold its real config
        // directly rather than asking port selection to reserve occupied ports.
        super::super::write_runtime_project(
            &root.join("cell"),
            "cell",
            FleetRuntimeTemplateArg::Simulate,
            control_port,
            web_port,
        )
        .expect("write managed runtime project");
        super::super::write_manifest(
            &root.join(FLEET_MANIFEST_FILE),
            &super::super::FleetManifest {
                runtime: vec![FleetManifestRuntime {
                    name: "cell".to_string(),
                    path: "cell".to_string(),
                    control_endpoint: format!("tcp://127.0.0.1:{control_port}"),
                    web_port,
                }],
            },
        )
        .expect("write managed runtime manifest");
        let paths = lifecycle_paths(&root, "cell");
        fs::create_dir_all(&paths.dir).expect("create lifecycle directory");
        fs::write(paths.pid, "4242").expect("write advisory PID");
        listener.set_nonblocking(true).expect("bound server wait");
        Self { root, listener }
    }
}

fn accept_request(listener: &TcpListener, expected: &str) -> TcpStream {
    accept_request_value(listener, expected).0
}

/// Accepts one request of type `expected` and returns the peer with the request it sent.
fn accept_request_value(listener: &TcpListener, expected: &str) -> (TcpStream, serde_json::Value) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "missing {expected} request");
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("accept {expected}: {error}"),
        }
    };
    stream
        .set_nonblocking(false)
        .expect("blocking accepted stream");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("bound request read");
    let mut request = String::new();
    BufReader::new(stream.try_clone().expect("clone peer"))
        .read_line(&mut request)
        .expect("read complete request");
    let request: serde_json::Value = serde_json::from_str(&request).expect("request JSON");
    assert_eq!(request["type"], expected);
    (stream, request)
}

fn acknowledge_shutdown(listener: &TcpListener) {
    let mut probe = accept_request(listener, "status");
    writeln!(probe, "{STATUS_OK}").expect("acknowledge the status probe");
    let (mut stream, request) = accept_request_value(listener, "shutdown");
    // Specification 11 section 6.8.3 and 34 STATE-08: `fleet runtime stop` ends the managed
    // process, so it sends the explicit exit; a plain `shutdown` would leave the process running.
    assert_eq!(request["params"], serde_json::json!({"exit": true}));
    writeln!(stream, "{STATUS_OK}").expect("acknowledge the shutdown");
}

/// A `status` answer that reports the program in `state`.
fn status_in_state(state: &str) -> String {
    serde_json::json!({"id": 1, "ok": true, "result": {"state": state}}).to_string()
}

#[test]
fn start_of_a_runtime_whose_program_is_stopped_restarts_it_cold() {
    // Specification 11 section 6.8.3 and 34 STATE-08: a managed process that answers while its
    // program is stopped is not already running; `fleet runtime start` starts the program with
    // `restart` cold, the mode of a new process, and reports it started once it runs. Wrong
    // behaviour: it reports the runtime as already running and the program stays stopped.
    let FleetFixture { root, listener } = FleetFixture::new();
    let server = thread::spawn(move || {
        let mut status = accept_request(&listener, "status");
        writeln!(status, "{}", status_in_state("stopped")).expect("stopped status");
        let (mut restart, request) = accept_request_value(&listener, "restart");
        assert_eq!(request["params"], serde_json::json!({"mode": "cold"}));
        writeln!(restart, "{STATUS_OK}").expect("acknowledge the restart");
        let mut running = accept_request(&listener, "status");
        writeln!(running, "{}", status_in_state("running")).expect("running status");
    });
    let result = start_runtime(&root, "cell").expect("start the stopped program");
    server.join().expect("join scripted peer");
    assert_eq!(result.status, "running");
    assert_eq!(result.message, "Runtime program started.");
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn start_of_a_runtime_whose_program_runs_reports_it_already_running_and_sends_nothing() {
    // Specification 11 section 6.8.3: the start of a runtime that already runs its program keeps
    // its answer. Wrong behaviour: it restarts a running program. The peer answers one probe, so a
    // `restart` that follows is never answered and fails the start.
    let FleetFixture { root, listener } = FleetFixture::new();
    let server = thread::spawn(move || {
        let mut status = accept_request(&listener, "status");
        writeln!(status, "{}", status_in_state("running")).expect("running status");
    });
    let result = start_runtime(&root, "cell").expect("start finds the runtime running");
    server.join().expect("join scripted peer");
    assert_eq!(result.status, "running");
    assert_eq!(result.message, "Runtime is already running.");
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn start_of_a_stopped_program_that_faults_on_its_start_reports_the_fault() {
    // Specification 34 STATE-08: a program that cannot start leaves the resource faulted with the
    // reason, and the start reports that reason instead of a started program. Wrong behaviour:
    // the start waits out its budget or reports the runtime started.
    let FleetFixture { root, listener } = FleetFixture::new();
    let server = thread::spawn(move || {
        let mut status = accept_request(&listener, "status");
        writeln!(status, "{}", status_in_state("stopped")).expect("stopped status");
        let mut restart = accept_request(&listener, "restart");
        writeln!(restart, "{STATUS_OK}").expect("acknowledge the restart");
        let mut faulted = accept_request(&listener, "status");
        let answer = serde_json::json!({
            "id": 1,
            "ok": true,
            "result": {"state": "faulted", "fault": "invalid bundle: broken program"},
        });
        writeln!(faulted, "{answer}").expect("faulted status");
    });
    let error = start_runtime(&root, "cell").expect_err("the program did not start");
    server.join().expect("join scripted peer");
    assert!(
        error.to_string().contains("invalid bundle: broken program"),
        "the fault is the reason: {error}"
    );
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn acknowledged_stop_confirms_disconnect_after_an_accepted_probe_loses_its_response() {
    let FleetFixture { root, listener } = FleetFixture::new();
    let server = thread::spawn(move || {
        acknowledge_shutdown(&listener);
        let stream = accept_request(&listener, "status");
        // Close listening first: the accepted probe observes EOF, then the next
        // connection is refused. No scheduling delay chooses the race outcome.
        drop(listener);
        drop(stream);
    });
    let result = stop_runtime(&root, "cell");
    server.join().expect("join scripted peer");
    assert_eq!(result.expect("confirm acknowledged stop").status, "stopped");
    assert!(!lifecycle_paths(&root, "cell").pid.exists());
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn post_acknowledgement_protocol_errors_are_not_converted_to_stopped() {
    for response in [
        r#"{"id":1,"ok":false,"error":"unauthorized"}"#,
        r#"{"id":99,"ok":true}"#,
        "not-json",
        r#"{"id":1,"ok":"true"}"#,
    ] {
        let FleetFixture { root, listener } = FleetFixture::new();
        let server = thread::spawn(move || {
            acknowledge_shutdown(&listener);
            drop(accept_request(&listener, "status"));
            let mut healthy = accept_request(&listener, "status");
            writeln!(healthy, "{STATUS_OK}").expect("still reachable");
            drop(healthy);
            let mut rejected = accept_request(&listener, "status");
            writeln!(rejected, "{response}").expect("send invalid response");
        });
        let result = stop_runtime(&root, "cell");
        server.join().expect("join scripted peer");
        assert!(result.is_err(), "must reject {response}");
        assert!(lifecycle_paths(&root, "cell").pid.exists());
        fs::remove_dir_all(root).expect("remove fixture");
    }
}

#[test]
fn exhausted_interrupted_stop_confirmation_retains_pid_and_reports_stopping() {
    let FleetFixture { root, listener } = FleetFixture::new();
    let server = thread::spawn(move || {
        acknowledge_shutdown(&listener);
        for _ in 0..=30 {
            drop(accept_request(&listener, "status"));
        }
    });
    let result = stop_runtime(&root, "cell");
    server.join().expect("join scripted peer");
    assert_eq!(result.expect("bounded confirmation").status, "stopping");
    assert!(lifecycle_paths(&root, "cell").pid.exists());
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn unacknowledged_lifecycle_probes_do_not_treat_accepted_eof_as_stopped() {
    for stopping in [false, true] {
        let FleetFixture { root, listener } = FleetFixture::new();
        let server = thread::spawn(move || drop(accept_request(&listener, "status")));
        let result = if stopping {
            stop_runtime(&root, "cell")
        } else {
            status_runtime(&root, "cell")
        };
        server.join().expect("join scripted peer");
        assert!(result.is_err());
        assert!(lifecycle_paths(&root, "cell").pid.exists());
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
