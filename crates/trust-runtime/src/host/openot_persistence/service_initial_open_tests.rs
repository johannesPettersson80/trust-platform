use std::time::{Duration, SystemTime, UNIX_EPOCH};

use open_ot_carriage::registry::EVENT_HEARTBEAT;
use open_ot_carriage::wire::Record;
use open_ot_definition::sample_definition;
use open_ot_shm::SharedRecordPublisher;

use super::super::transport_failure_fixture::TransportFailureEndpoint;
use super::*;
use crate::config::{
    OpenOtInfluxDb3PersistenceConfig, OpenOtPersistenceBackend, OpenOtPersistenceConfig,
};

#[test]
fn service_retries_unreachable_influxdb_during_initial_open() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-influx-initial-retry-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure root");
    }
    std::fs::write(
        root.join("openot-definition.json"),
        serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
    )
    .expect("write definition");
    let mut publisher =
        SharedRecordPublisher::create(root.join("openot.shm"), 4096).expect("publisher");
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/tls/server-cert.pem"
        ),
        root.join("database-ca.pem"),
    )
    .expect("copy parseable CA certificate");
    let endpoint = TransportFailureEndpoint::new().expect("own unavailable local endpoint");
    let address = endpoint.address();
    let host_environment = format!(
        "TRUST_TEST_OPENOT_INFLUX_INITIAL_HOST_{}_{}",
        std::process::id(),
        stamp
    );
    let token_environment = format!(
        "TRUST_TEST_OPENOT_INFLUX_INITIAL_TOKEN_{}_{}",
        std::process::id(),
        stamp
    );
    std::env::set_var(&host_environment, format!("https://{address}"));
    std::env::set_var(&token_environment, "test-token");
    let config = OpenOtTelemetryConfig {
        enabled: true,
        path: "openot.shm".into(),
        persistence: OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::InfluxDb3),
            retry_initial_ms: 10,
            retry_max_ms: 10,
            retry_multiplier: 1,
            retry_max_attempts: 1_000,
            shutdown_timeout_ms: 50,
            influxdb3: Some(OpenOtInfluxDb3PersistenceConfig {
                host_env: host_environment.clone().into(),
                token_env: token_environment.clone().into(),
                database: "trust_logging".into(),
                spool_path: "trust-logging-spool.sqlite3".into(),
                max_bytes: 1_073_741_824,
                ca_cert_path: Some("database-ca.pem".into()),
            }),
            ..OpenOtPersistenceConfig::default()
        },
        ..OpenOtTelemetryConfig::default()
    };

    let mut service = OpenOtPersistenceService::start(&config, &root)
        .expect("unreachable remote endpoint must not reject PLC startup")
        .expect("enabled persistence service");
    publisher
        .append_record(&Record::new(11, 1, 0, 7, EVENT_HEARTBEAT))
        .expect("publish while the initial database connection is unavailable");
    let connection = endpoint.wait_for_request();
    let opening_status = service.status();
    assert_eq!(opening_status.state, OpenOtPersistenceState::Starting);
    assert_eq!(opening_status.documents_committed, 0);
    assert_eq!(opening_status.documents_retried, 0);
    assert_eq!(opening_status.cursor_abs, 0);
    // Start the existing behavior window only after an actual transport failure.
    // TLS/spool preparation is complete; the owned port cannot be reused.
    connection
        .shutdown(std::net::Shutdown::Both)
        .expect("close observed initial TLS request");
    drop(connection);
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while {
        let status = service.status();
        (status.state == OpenOtPersistenceState::Starting || status.head_abs == 0)
            && std::time::Instant::now() < deadline
    } {
        std::thread::sleep(Duration::from_millis(5));
    }
    let status = service.status();
    service.shutdown();
    let stopped_status = service.status();
    std::env::remove_var(host_environment);
    std::env::remove_var(token_environment);

    assert_eq!(status.state, OpenOtPersistenceState::Retrying, "{status:?}");
    assert!(status.documents_retried >= 1, "{status:?}");
    assert!(status.head_abs > 0, "{status:?}");
    assert_eq!(status.cursor_abs, 0, "{status:?}");
    assert_eq!(status.pending, status.head_abs, "{status:?}");
    assert_eq!(
        stopped_status.head_abs, status.head_abs,
        "{stopped_status:?}"
    );
    assert_eq!(stopped_status.pending, status.pending, "{stopped_status:?}");
    std::fs::remove_dir_all(root).ok();
}
