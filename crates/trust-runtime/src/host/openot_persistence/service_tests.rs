use std::time::{Duration, SystemTime, UNIX_EPOCH};

use open_ot_carriage::registry::{
    EVENT_HEARTBEAT, EVENT_VALUE_CHANGED, KEY_NEW_VALUE, KEY_VALUE_ID, TY_BOOL, TY_UDINT,
};
use open_ot_carriage::wire::{Record, Slot};
use open_ot_definition::sample_definition;
use open_ot_shm::SharedRecordPublisher;

use super::*;
use crate::config::{
    OpenOtPersistenceBackend, OpenOtPersistenceConfig, OpenOtPersistenceTlsMode,
    OpenOtPostgreSqlPersistenceConfig, OpenOtSqlitePersistenceConfig,
};
#[test]
fn operator_status_redacts_backend_secrets_and_sensitive_paths() {
    let secret = "password=plant-secret token=operator-token /private/customer/history.db";
    for error in [
        PersistenceError::InvalidConfig(secret.to_string()),
        PersistenceError::Commit(secret.to_string()),
        PersistenceError::Connection(secret.to_string()),
        PersistenceError::CapacityExhausted(secret.to_string()),
    ] {
        let projected = redacted_error(&error);
        assert!(!projected.contains("plant-secret"));
        assert!(!projected.contains("operator-token"));
        assert!(!projected.contains("/private/customer"));
    }
}

#[cfg(feature = "openot-database-postgresql")]
#[test]
fn service_rejects_missing_database_ca_before_worker_start() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-missing-ca-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("root");
    std::fs::write(
        root.join("openot-definition.json"),
        serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
    )
    .expect("write definition");
    SharedRecordPublisher::create(root.join("openot.shm"), 4096).expect("publisher");
    let database_environment = format!(
        "TRUST_TEST_OPENOT_MISSING_CA_DATABASE_URL_{}_{}",
        std::process::id(),
        stamp
    );
    std::env::set_var(
        &database_environment,
        "postgresql://unused.invalid/trust_logging",
    );
    let config = OpenOtTelemetryConfig {
        enabled: true,
        path: "openot.shm".into(),
        persistence: OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::PostgreSql),
            postgresql: Some(OpenOtPostgreSqlPersistenceConfig {
                connection_url_env: database_environment.clone().into(),
                schema: "trust_logging".into(),
                tls: OpenOtPersistenceTlsMode::Require,
                ca_cert_path: Some("missing-ca.pem".into()),
            }),
            ..OpenOtPersistenceConfig::default()
        },
        ..OpenOtTelemetryConfig::default()
    };

    let error = match OpenOtPersistenceService::start(&config, &root) {
        Err(error) => error,
        Ok(_) => panic!("missing local CA must reject startup synchronously"),
    };
    std::env::remove_var(database_environment);
    assert!(
        error.to_string().contains("missing-ca.pem"),
        "unexpected error: {error}"
    );
    std::fs::remove_dir_all(root).ok();
}

#[cfg(feature = "openot-database-postgresql")]
#[test]
fn service_rejects_missing_database_environment_before_worker_start() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-missing-database-environment-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("root");
    std::fs::write(
        root.join("openot-definition.json"),
        serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
    )
    .expect("write definition");
    std::fs::write(root.join("database-ca.pem"), b"local-startup-artifact")
        .expect("write CA artifact");
    SharedRecordPublisher::create(root.join("openot.shm"), 4096).expect("publisher");
    let missing_environment = format!(
        "TRUST_TEST_OPENOT_MISSING_DATABASE_URL_{}_{}",
        std::process::id(),
        stamp
    );
    assert!(std::env::var_os(&missing_environment).is_none());
    let config = OpenOtTelemetryConfig {
        enabled: true,
        path: "openot.shm".into(),
        persistence: OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::PostgreSql),
            postgresql: Some(OpenOtPostgreSqlPersistenceConfig {
                connection_url_env: missing_environment.clone().into(),
                schema: "trust_logging".into(),
                tls: OpenOtPersistenceTlsMode::Require,
                ca_cert_path: Some("database-ca.pem".into()),
            }),
            ..OpenOtPersistenceConfig::default()
        },
        ..OpenOtTelemetryConfig::default()
    };

    let error = match OpenOtPersistenceService::start(&config, &root) {
        Err(error) => error,
        Ok(_) => panic!("missing database environment must reject startup synchronously"),
    };
    let diagnostic = error.to_string();
    assert!(
        diagnostic.contains(&missing_environment),
        "unexpected error: {error}"
    );
    assert!(!diagnostic.contains("postgresql://"));

    std::env::set_var(&missing_environment, "");
    let empty_error = match OpenOtPersistenceService::start(&config, &root) {
        Err(error) => error,
        Ok(_) => panic!("empty database environment must reject startup synchronously"),
    };
    std::env::remove_var(&missing_environment);
    assert!(empty_error.to_string().contains(&missing_environment));
    std::fs::remove_dir_all(root).ok();
}

#[cfg(feature = "openot-database-sqlite")]
#[test]
fn service_faults_incompatible_pre_release_schema_without_spending_reconnect_budget() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-logging-service-incompatible-schema-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("root");
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
    SharedRecordPublisher::create(root.join("openot.shm"), 4096).expect("publisher");
    let database_path = root.join("trust-logging.sqlite3");
    rusqlite::Connection::open(&database_path)
        .expect("create incompatible pre-release database")
        .execute_batch("CREATE TABLE sentinel(value TEXT); PRAGMA user_version=5;")
        .expect("seed incompatible pre-release schema");
    let config = OpenOtTelemetryConfig {
        enabled: true,
        path: "openot.shm".into(),
        persistence: OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::Sqlite),
            retry_initial_ms: 1_000,
            retry_max_ms: 1_000,
            retry_multiplier: 1,
            retry_max_attempts: 3,
            sqlite: Some(OpenOtSqlitePersistenceConfig {
                path: database_path.clone(),
            }),
            ..OpenOtPersistenceConfig::default()
        },
        ..OpenOtTelemetryConfig::default()
    };

    let mut service = OpenOtPersistenceService::start(&config, &root)
        .expect("local schema validation belongs to supervised worker")
        .expect("enabled service");
    let deadline = std::time::Instant::now() + Duration::from_millis(250);
    while service.status().state == OpenOtPersistenceState::Starting
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    let status = service.status();
    assert_eq!(status.state, OpenOtPersistenceState::Faulted);
    assert_eq!(status.documents_retried, 0);
    assert_eq!(
        status.last_error.as_deref(),
        Some("selected OpenOT persistence backend operation failed"),
        "operator status must redact storage details: {status:?}"
    );
    service.shutdown();
    let version: u32 = rusqlite::Connection::open(database_path)
        .expect("reopen database")
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("schema version");
    assert_eq!(version, 5);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn enabled_service_persists_shared_memory_record_off_thread_and_stops() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-service-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("root");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure root");
    }
    let shm_path = root.join("openot.shm");
    let mut publisher = SharedRecordPublisher::create(&shm_path, 4096).expect("publisher");
    publisher
        .append_record(&Record::new(11, 1, 0, 7, EVENT_HEARTBEAT))
        .expect("publish");
    std::fs::write(
        root.join("openot-definition.json"),
        serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
    )
    .expect("write definition");
    let mut config = OpenOtTelemetryConfig {
        enabled: true,
        path: std::path::PathBuf::from("openot.shm"),
        ..OpenOtTelemetryConfig::default()
    };
    config.persistence = OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(OpenOtPersistenceBackend::Sqlite),
        flush_interval_ms: 10,
        sqlite: Some(OpenOtSqlitePersistenceConfig {
            path: std::path::PathBuf::from("trust-logging.sqlite3"),
        }),
        ..OpenOtPersistenceConfig::default()
    };

    let mut service = OpenOtPersistenceService::start(&config, &root)
        .expect("start service")
        .expect("enabled service");
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while service.status().documents_committed < 1 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(service.status().documents_committed, 1);
    assert_eq!(service.status().state, OpenOtPersistenceState::Ready);
    assert_eq!(service.status().schema_version, Some(1));
    service.shutdown();
    assert_eq!(service.status().state, OpenOtPersistenceState::Stopped);
    assert!(root.join("trust-logging.sqlite3").is_file());
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn service_supplies_compiled_definition_to_typed_database_projection() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-logging-service-definition-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("root");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).expect("secure root");
    let definition = sample_definition();
    std::fs::write(
        root.join("openot-definition.json"),
        serde_json::to_vec_pretty(&definition).expect("serialize definition"),
    )
    .expect("write definition");
    let shm_path = root.join("openot.shm");
    let mut publisher = SharedRecordPublisher::create(&shm_path, 4096).expect("publisher");
    let mut value = Record::new(11, 1, 0, 66, EVENT_VALUE_CHANGED);
    value.slots = vec![
        Slot::new(KEY_VALUE_ID, TY_UDINT, 2003u32.to_le_bytes()),
        Slot::new(KEY_NEW_VALUE, TY_BOOL, [1]),
    ];
    publisher
        .append_record(&value)
        .expect("publish typed value");
    let config = OpenOtTelemetryConfig {
        enabled: true,
        path: "openot.shm".into(),
        persistence: OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::Sqlite),
            flush_interval_ms: 10,
            retry_max_attempts: 2,
            sqlite: Some(OpenOtSqlitePersistenceConfig {
                path: "trust-logging.sqlite3".into(),
            }),
            ..OpenOtPersistenceConfig::default()
        },
        ..OpenOtTelemetryConfig::default()
    };

    let mut service = OpenOtPersistenceService::start(&config, &root)
        .expect("start typed logging service")
        .expect("enabled typed logging service");
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while service.status().documents_committed < 1 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let status = service.status();
    service.shutdown();
    assert_eq!(
        status.documents_committed, 1,
        "typed persistence status: {status:?}"
    );
    let database = rusqlite::Connection::open(root.join("trust-logging.sqlite3"))
        .expect("inspect typed logging database");
    let stored: (String, bool) = database
        .query_row(
            "SELECT value_name,boolean_value FROM logged_values",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query service-projected value");
    assert_eq!(stored, ("Enabled".to_string(), true));
    let projection_rows: i64 = database
        .query_row(
            "SELECT (SELECT COUNT(*) FROM event_log) + (SELECT COUNT(*) FROM logged_values)",
            [],
            |row| row.get(0),
        )
        .expect("count event and value projections");
    assert_eq!(status.projection_rows_committed, projection_rows as u64);
    assert_eq!(status.projection_rows_committed, 2);
    assert_eq!(status.unclassified_event_count, 0);
    assert_eq!(status.reconciled_part_count, 0);
    assert_eq!(status.pending_part_count, 0);
    drop(database);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_service_restart_uses_durable_checkpoint_and_catches_up_once() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-sqlite-service-restart-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure root");
    }
    let shm_path = root.join("openot.shm");
    let mut publisher = SharedRecordPublisher::create(&shm_path, 4096).expect("publisher");
    publisher
        .append_record(&Record::new(11, 1, 0, 7, EVENT_HEARTBEAT))
        .expect("publish baseline");
    std::fs::write(
        root.join("openot-definition.json"),
        serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
    )
    .expect("write definition");
    let mut config = OpenOtTelemetryConfig {
        enabled: true,
        path: std::path::PathBuf::from("openot.shm"),
        ..OpenOtTelemetryConfig::default()
    };
    config.persistence = OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(OpenOtPersistenceBackend::Sqlite),
        flush_interval_ms: 10,
        sqlite: Some(OpenOtSqlitePersistenceConfig {
            path: "history/trust-logging.sqlite3".into(),
        }),
        ..OpenOtPersistenceConfig::default()
    };

    let mut first = OpenOtPersistenceService::start(&config, &root)
        .expect("start first service")
        .expect("enabled first service");
    let first_deadline = std::time::Instant::now() + Duration::from_secs(5);
    while first.status().documents_committed < 1 && std::time::Instant::now() < first_deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(first.status().documents_committed, 1);
    let baseline_cursor = first.status().cursor_abs;
    first.shutdown();

    publisher
        .append_record(&Record::new(12, 1, 1, 7, EVENT_HEARTBEAT))
        .expect("publish while stopped 1");
    publisher
        .append_record(&Record::new(13, 1, 2, 7, EVENT_HEARTBEAT))
        .expect("publish while stopped 2");
    let mut second = OpenOtPersistenceService::start(&config, &root)
        .expect("restart service")
        .expect("enabled restarted service");
    let second_deadline = std::time::Instant::now() + Duration::from_secs(5);
    while (second.status().state != OpenOtPersistenceState::Ready
        || second.status().cursor_abs <= baseline_cursor)
        && std::time::Instant::now() < second_deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    let second_status = second.status();
    assert_eq!(second_status.state, OpenOtPersistenceState::Ready);
    assert_eq!(second_status.cursor_abs, second_status.head_abs);
    assert_eq!(second_status.documents_committed, 2);
    second.shutdown();

    let database = rusqlite::Connection::open(root.join("history/trust-logging.sqlite3"))
        .expect("inspect restart database");
    let events: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM logging_records WHERE document_kind='event'",
            [],
            |row| row.get(0),
        )
        .expect("event count");
    assert_eq!(events, 3);
    drop(database);

    let mut third = OpenOtPersistenceService::start(&config, &root)
        .expect("start caught-up service")
        .expect("enabled caught-up service");
    let third_deadline = std::time::Instant::now() + Duration::from_secs(5);
    while third.status().state != OpenOtPersistenceState::Ready
        && std::time::Instant::now() < third_deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(third.status().documents_committed, 0);
    assert_eq!(third.status().cursor_abs, third.status().head_abs);
    third.shutdown();
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn shutdown_does_not_wait_past_the_configured_deadline_for_a_stuck_backend_call() {
    let status = Arc::new(Mutex::new(OpenOtPersistenceStatus {
        state: OpenOtPersistenceState::Ready,
        backend: Some("sqlite".to_string()),
        schema_version: Some(1),
        documents_read: 0,
        documents_committed: 0,
        documents_duplicated: 0,
        remote_pending: 0,
        projection_rows_committed: 0,
        unclassified_event_count: 0,
        reconciled_part_count: 0,
        pending_part_count: 0,
        documents_retried: 0,
        pending: 4,
        rejected: 0,
        unresolved: 0,
        loss_range_count: 0,
        lost_record_count: 0,
        cursor_abs: 0,
        head_abs: 4,
        last_success_time_ns: None,
        last_error: None,
    }));
    let mut service = OpenOtPersistenceService {
        stop: Arc::new(AtomicBool::new(false)),
        drain_expired: Arc::new(AtomicBool::new(false)),
        shutdown_deadline: Arc::new(Mutex::new(None)),
        status,
        thread: Some(std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(500));
        })),
        shutdown_timeout: Duration::from_millis(20),
    };
    let started = std::time::Instant::now();

    service.shutdown();

    assert!(started.elapsed() < Duration::from_millis(200));
    let status = service.status();
    assert_eq!(status.state, OpenOtPersistenceState::Faulted);
    assert_eq!(status.pending, 4);
    service.shutdown();
    assert_eq!(
        service.status().state,
        OpenOtPersistenceState::Faulted,
        "repeated shutdown must preserve the timeout fault"
    );
}

#[test]
fn shutdown_drains_records_published_after_the_last_worker_poll() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-shutdown-drain-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("root");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure root");
    }
    let shm_path = root.join("openot.shm");
    let mut publisher = SharedRecordPublisher::create(&shm_path, 4096).expect("publisher");
    std::fs::write(
        root.join("openot-definition.json"),
        serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
    )
    .expect("write definition");
    let config = OpenOtTelemetryConfig {
        enabled: true,
        path: "openot.shm".into(),
        persistence: OpenOtPersistenceConfig {
            enabled: true,
            backend: Some(OpenOtPersistenceBackend::Sqlite),
            flush_interval_ms: 60_000,
            shutdown_timeout_ms: 2_000,
            sqlite: Some(OpenOtSqlitePersistenceConfig {
                path: "history/logging.sqlite3".into(),
            }),
            ..OpenOtPersistenceConfig::default()
        },
        ..OpenOtTelemetryConfig::default()
    };
    let mut service = OpenOtPersistenceService::start(&config, &root)
        .expect("start service")
        .expect("enabled service");
    let ready_deadline = std::time::Instant::now() + Duration::from_secs(2);
    while service.status().state != OpenOtPersistenceState::Ready
        && std::time::Instant::now() < ready_deadline
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(service.status().state, OpenOtPersistenceState::Ready);
    assert_eq!(
        service.status().last_success_time_ns,
        None,
        "idle polls must not masquerade as durable commits"
    );
    publisher
        .append_record(&Record::new(11, 1, 0, 7, EVENT_HEARTBEAT))
        .expect("publish immediately before shutdown");

    service.shutdown();

    let connection = rusqlite::Connection::open(root.join("history/logging.sqlite3"))
        .expect("inspect drained database");
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM logging_records", [], |row| row.get(0))
        .expect("count drained records");
    assert_eq!(count, 1, "shutdown must commit pre-request records");
    assert_eq!(service.status().state, OpenOtPersistenceState::Stopped);
    std::fs::remove_dir_all(root).ok();
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn postgresql_service_reconnects_and_catches_up_after_real_server_restart() {
    struct RestartGuard(String);
    impl Drop for RestartGuard {
        fn drop(&mut self) {
            let _ = std::process::Command::new("docker")
                .args(["start", self.0.as_str()])
                .status();
        }
    }

    let container = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CONTAINER")
        .expect("real PostgreSQL container name");
    let ca = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA").expect("PostgreSQL CA");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-postgresql-reconnect-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("root");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure root");
    }
    let shm_path = root.join("openot.shm");
    let mut publisher = SharedRecordPublisher::create(&shm_path, 4096).expect("publisher");
    publisher
        .append_record(&Record::new(11, 1, 0, 7, EVENT_HEARTBEAT))
        .expect("publish baseline");
    std::fs::write(
        root.join("openot-definition.json"),
        serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
    )
    .expect("write definition");
    let mut config = OpenOtTelemetryConfig {
        enabled: true,
        path: std::path::PathBuf::from("openot.shm"),
        ..OpenOtTelemetryConfig::default()
    };
    config.persistence = OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(OpenOtPersistenceBackend::PostgreSql),
        flush_interval_ms: 10,
        retry_initial_ms: 20,
        retry_max_ms: 20,
        retry_multiplier: 1,
        retry_max_attempts: 200,
        postgresql: Some(OpenOtPostgreSqlPersistenceConfig {
            connection_url_env: "TRUST_TEST_OPENOT_POSTGRES_URL".into(),
            schema: format!("logging_reconnect_{}_{}", std::process::id(), stamp).into(),
            tls: OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some(ca.into()),
        }),
        ..OpenOtPersistenceConfig::default()
    };
    let stopped = std::process::Command::new("docker")
        .args(["stop", container.as_str()])
        .status()
        .expect("stop real PostgreSQL before persistence startup");
    assert!(stopped.success());
    let _restart_guard = RestartGuard(container.clone());
    let mut service = OpenOtPersistenceService::start(&config, &root)
        .expect("start PostgreSQL service")
        .expect("enabled service");
    wait_for_status(&service, Duration::from_secs(5), |status| {
        status.state == OpenOtPersistenceState::Retrying
    });
    assert_eq!(service.status().schema_version, None);
    let started = std::process::Command::new("docker")
        .args(["start", container.as_str()])
        .status()
        .expect("start real PostgreSQL after persistence startup");
    assert!(started.success());
    wait_for_status(&service, Duration::from_secs(5), |status| {
        status.state == OpenOtPersistenceState::Ready && status.cursor_abs > 0
    });
    assert_eq!(service.status().schema_version, Some(1));

    let stopped = std::process::Command::new("docker")
        .args(["stop", container.as_str()])
        .status()
        .expect("stop real PostgreSQL");
    assert!(stopped.success());
    publisher
        .append_record(&Record::new(11, 2, 0, 7, EVENT_HEARTBEAT))
        .expect("publish during outage");
    wait_for_status(&service, Duration::from_secs(5), |status| {
        status.state == OpenOtPersistenceState::Retrying
    });
    let started = std::process::Command::new("docker")
        .args(["start", container.as_str()])
        .status()
        .expect("restart real PostgreSQL");
    assert!(started.success());

    wait_for_status(&service, Duration::from_secs(15), |status| {
        status.state == OpenOtPersistenceState::Ready
            && status.cursor_abs == status.head_abs
            && status.cursor_abs > 64
    });
    assert_eq!(
        service.status().documents_committed,
        2,
        "reconnection must preserve cumulative service counters"
    );
    assert_eq!(
        service.status().documents_read,
        2,
        "reconnection must preserve cumulative source counters"
    );
    service.shutdown();
    std::fs::remove_dir_all(root).ok();
}

#[cfg(feature = "openot-real-database-tests")]
fn wait_for_status(
    service: &OpenOtPersistenceService,
    timeout: Duration,
    predicate: impl Fn(&OpenOtPersistenceStatus) -> bool,
) {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        let status = service.status();
        if predicate(&status) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "OpenOT persistence status deadline expired: {:#?}",
        service.status()
    );
}

#[test]
fn unresolved_document_keeps_caught_up_service_degraded() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-degraded-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("root");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure root");
    }
    let shm_path = root.join("openot.shm");
    let mut publisher = SharedRecordPublisher::create(&shm_path, 4096).expect("publisher");
    publisher
        .append_record(&Record::new(11, 1, 0, 7, 0xFFFF))
        .expect("publish unresolved event");
    std::fs::write(
        root.join("openot-definition.json"),
        serde_json::to_vec_pretty(&sample_definition()).expect("serialize definition"),
    )
    .expect("write definition");
    let mut config = OpenOtTelemetryConfig {
        enabled: true,
        path: std::path::PathBuf::from("openot.shm"),
        ..OpenOtTelemetryConfig::default()
    };
    config.persistence = OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(OpenOtPersistenceBackend::Sqlite),
        flush_interval_ms: 10,
        sqlite: Some(OpenOtSqlitePersistenceConfig {
            path: std::path::PathBuf::from("trust-logging.sqlite3"),
        }),
        ..OpenOtPersistenceConfig::default()
    };
    let mut service = OpenOtPersistenceService::start(&config, &root)
        .expect("start service")
        .expect("enabled service");
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while service.status().documents_committed < 1 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(service.status().unresolved, 1);
    assert_eq!(service.status().state, OpenOtPersistenceState::Degraded);
    service.shutdown();
    std::fs::remove_dir_all(root).ok();
}
