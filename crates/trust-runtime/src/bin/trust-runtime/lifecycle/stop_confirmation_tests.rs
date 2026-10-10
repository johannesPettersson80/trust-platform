use super::*;
use crate::cli::FleetRuntimeTemplateArg;
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
    stream
}

fn acknowledge_shutdown(listener: &TcpListener) {
    for operation in ["status", "shutdown"] {
        let mut stream = accept_request(listener, operation);
        writeln!(stream, "{STATUS_OK}").expect("acknowledge request");
    }
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

#[test]
fn only_transport_boundary_errors_are_inconclusive_stop_probes() {
    for kind in [
        std::io::ErrorKind::ConnectionReset,
        std::io::ErrorKind::ConnectionAborted,
        std::io::ErrorKind::BrokenPipe,
        std::io::ErrorKind::UnexpectedEof,
        std::io::ErrorKind::TimedOut,
        std::io::ErrorKind::WouldBlock,
    ] {
        let transport = control_transport_error(std::io::Error::from(kind));
        assert!(interrupted_stop_probe(
            &transport.context("control exchange")
        ));
        let configuration = anyhow::Error::from(std::io::Error::from(kind));
        assert!(!interrupted_stop_probe(&configuration));
    }
    let other = control_transport_error(std::io::Error::from(std::io::ErrorKind::InvalidData));
    assert!(!interrupted_stop_probe(&other));
}
