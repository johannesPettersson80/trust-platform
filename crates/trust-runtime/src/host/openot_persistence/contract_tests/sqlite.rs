use super::*;
#[test]
fn sqlite_sink_opens_empty_database_with_initial_schema_generation() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");

    let sink = open_test_sqlite(&path)
        .unwrap_or_else(|error| panic!("SQLite initialization failed: {error:?}"));

    assert!(path.is_file(), "SQLite database was not created");
    assert_eq!(sink.schema_version().expect("schema generation"), 1);
    drop(sink);
    let connection = rusqlite::Connection::open(&path).expect("inspect SQLite schema marker");
    connection
        .execute("UPDATE logging_schema SET version=2 WHERE singleton=1", [])
        .expect_err("generation-1 marker constraint must reject another value");
    let marker: u32 = connection
        .query_row(
            "SELECT version FROM logging_schema WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .expect("read unchanged schema marker");
    assert_eq!(marker, 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sqlite_sink_exposes_heartbeat_through_descriptive_event_log() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-logging-event-log-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");
    let mut sink = open_test_sqlite(&path).expect("open SQLite logging database");
    sink.commit(&PersistenceBatch {
        documents: vec![heartbeat_document()],
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: u64::from(std::process::id()),
            cursor_abs: 64,
        },
    })
    .expect("commit heartbeat");
    drop(sink);

    let connection = rusqlite::Connection::open(&path).expect("inspect logging database");
    let public_event_log_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='event_log')",
            [],
            |row| row.get(0),
        )
        .expect("inspect public event log");
    assert!(
        public_event_log_exists,
        "schema generation 1 must expose the descriptive event_log table"
    );
    let internal_record_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='logging_records')",
            [],
            |row| row.get(0),
        )
        .expect("inspect internal logging records");
    assert!(
        internal_record_exists,
        "schema generation 1 must use the descriptive internal logging_records name"
    );
    let stored: (String, i64, i64, String) = connection
        .query_row(
            "SELECT event_name,event_type_id,source_id,sequence FROM event_log",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("query heartbeat without JSON extraction");
    assert_eq!(
        stored,
        ("Heartbeat".to_string(), 0x0100, 66, "1".to_string())
    );
    drop(connection);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_projects_named_bool_and_full_ulint_without_json() {
    let definition = open_ot_definition::sample_definition();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-logging-typed-values-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");
    let mut sink = SqliteDocumentSink::open_with_definitions(&path, vec![definition.clone()])
        .expect("open SQLite with logging definition");
    sink.commit(&PersistenceBatch {
        documents: vec![
            value_changed_document(&definition, 2, 2003, "Bool", serde_json::json!(true)),
            value_changed_document(&definition, 3, 2009, "ULInt", serde_json::json!(u64::MAX)),
        ],
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: u64::from(std::process::id()),
            cursor_abs: 128,
        },
    })
    .expect("commit typed values");
    drop(sink);

    let connection = rusqlite::Connection::open(&path).expect("inspect typed logging values");
    let mut statement = connection
        .prepare(
            "SELECT value_name,value_type,boolean_value,unsigned_value,exact_value \
             FROM logged_values ORDER BY sequence",
        )
        .expect("query typed values without JSON");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<bool>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .expect("read typed values")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect typed values");
    assert_eq!(
        rows,
        vec![
            (
                "Enabled".to_string(),
                "BOOL".to_string(),
                Some(true),
                None,
                "true".to_string(),
            ),
            (
                "UnsignedLong".to_string(),
                "ULINT".to_string(),
                None,
                Some(u64::MAX.to_string()),
                u64::MAX.to_string(),
            ),
        ]
    );
    drop(statement);
    drop(connection);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_public_read_model_exposes_common_columns_on_every_object() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-logging-common-columns-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");
    drop(open_test_sqlite(&path).expect("create SQLite public read model"));
    let connection = rusqlite::Connection::open(&path).expect("inspect SQLite public read model");
    let required = [
        "record_id",
        "event_time",
        "event_time_ns",
        "received_time",
        "received_time_ns",
        "source",
        "source_id",
        "source_path",
        "source_hierarchy",
        "buffer_id",
        "run_id",
        "epoch_id",
        "sequence",
        "definition_hash",
        "time_unsynced",
        "synthetic_record",
        "partial_payload",
    ];
    for object in [
        "event_log",
        "logged_values",
        "alarm_history",
        "message_log",
        "state_history",
        "batch_history",
        "recipe_history",
        "material_additions",
        "operator_activity",
        "audit_log",
        "electronic_signatures",
        "system_events",
        "data_loss",
        "unresolved_records",
    ] {
        let mut statement = connection
            .prepare(&format!("PRAGMA table_info({object})"))
            .expect("inspect public object columns");
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query public object columns")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect public object columns");
        for column in required {
            assert!(
                columns.iter().filter(|candidate| *candidate == column).count() == 1,
                "public object {object} must expose common column {column} exactly once: {columns:?}"
            );
        }
        assert!(
            columns.iter().all(|column| !column.contains(':')),
            "public object {object} must not expose SQLite-renamed duplicate columns: {columns:?}"
        );
    }
    drop(connection);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_rejects_incompatible_pre_release_schema_without_mutating_it() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-logging-incompatible-schema-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");
    std::fs::create_dir_all(&root).expect("schema root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure schema root");
    }
    let connection = rusqlite::Connection::open(&path).expect("create pre-release database");
    connection
        .execute_batch(
            "CREATE TABLE openot_documents(identity_key TEXT PRIMARY KEY); \
             INSERT INTO openot_documents VALUES('sentinel'); \
             PRAGMA user_version=3;",
        )
        .expect("seed incompatible pre-release schema");
    drop(connection);

    let error = open_test_sqlite(&path).expect_err("legacy schema must fail closed");
    assert!(format!("{error:?}").contains("incompatible pre-release schema"));

    let connection = rusqlite::Connection::open(&path).expect("inspect untouched database");
    let sentinel: String = connection
        .query_row("SELECT identity_key FROM openot_documents", [], |row| {
            row.get(0)
        })
        .expect("legacy row remains");
    assert_eq!(sentinel, "sentinel");
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .expect("schema marker"),
        3
    );
    drop(connection);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_rejects_markerless_logging_object_without_mutating_it() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-markerless-schema-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("schema root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure schema root");
    }
    let path = root.join("trust-logging.sqlite3");
    let connection = rusqlite::Connection::open(&path).expect("create markerless database");
    connection
        .execute_batch(
            "CREATE TABLE logging_records(identity_key TEXT PRIMARY KEY); \
             INSERT INTO logging_records VALUES('sentinel');",
        )
        .expect("seed markerless schema");
    drop(connection);

    let error = open_test_sqlite(&path).expect_err("markerless logging schema must fail");
    assert!(format!("{error:?}").contains("incompatible pre-release schema"));
    let connection = rusqlite::Connection::open(&path).expect("inspect untouched database");
    let sentinel: String = connection
        .query_row("SELECT identity_key FROM logging_records", [], |row| {
            row.get(0)
        })
        .expect("stored row remains");
    assert_eq!(sentinel, "sentinel");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_rejects_incomplete_generation_1_without_repairing_it() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-logging-incomplete-schema-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("schema root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure schema root");
    }
    let path = root.join("trust-logging.sqlite3");
    let connection = rusqlite::Connection::open(&path).expect("create incomplete database");
    connection
        .execute_batch(
            "CREATE TABLE logging_schema(singleton INTEGER PRIMARY KEY,version INTEGER NOT NULL); \
             INSERT INTO logging_schema VALUES(1,1); \
             PRAGMA user_version=1;",
        )
        .expect("seed incomplete generation 1");
    drop(connection);

    let error = open_test_sqlite(&path).expect_err("incomplete schema must fail closed");
    assert!(format!("{error:?}").contains("required object logging_records is missing"));
    let connection = rusqlite::Connection::open(&path).expect("inspect untouched database");
    let table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            [],
            |row| row.get(0),
        )
        .expect("count tables");
    assert_eq!(table_count, 1, "rejection must not add missing tables");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_rejects_newer_schema_without_mutating_it() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-newer-schema-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("schema root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure schema root");
    }
    let path = root.join("trust-logging.sqlite3");
    let connection = rusqlite::Connection::open(&path).expect("create newer database");
    connection
        .execute_batch("CREATE TABLE sentinel(value TEXT); PRAGMA user_version=5;")
        .expect("seed newer schema");
    drop(connection);

    let error = open_test_sqlite(&path).expect_err("newer schema must fail closed");

    assert!(format!("{error:?}").contains("incompatible pre-release schema"));
    let connection = rusqlite::Connection::open(&path).expect("reopen untouched database");
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .expect("schema version"),
        5
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_rejects_malformed_checkpoint_run_identity() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-corrupt-checkpoint-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");
    drop(open_test_sqlite(&path).expect("create schema"));
    let connection = rusqlite::Connection::open(&path).expect("open database for corruption");
    connection
        .execute_batch(
            "PRAGMA ignore_check_constraints=ON; \
             INSERT INTO logging_checkpoint(singleton,buffer_id,run_id,cursor_abs) \
             VALUES(1,7,X'01',X'000000000000007B');",
        )
        .expect("inject malformed durable bytes");
    drop(connection);
    let mut sink = open_test_sqlite(&path).expect("schema itself remains readable");

    let error = sink
        .load_checkpoint(7, 1)
        .expect_err("malformed checkpoint must fail closed");

    assert!(format!("{error:?}").contains("checkpoint run is not an 8-byte unsigned value"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_rejects_malformed_stored_canonical_document_on_reopen() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-corrupt-document-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");
    let mut sink = open_test_sqlite(&path).expect("create schema");
    sink.commit(&PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 24_576,
        },
    })
    .expect("commit valid canonical documents");
    drop(sink);
    let connection = rusqlite::Connection::open(&path).expect("open database for corruption");
    connection
        .execute(
            "UPDATE logging_records SET canonical_json='{not-json' WHERE identity_key=(SELECT MIN(identity_key) FROM logging_records)",
            [],
        )
        .expect("inject malformed canonical JSON");
    drop(connection);

    let error = open_test_sqlite(&path)
        .expect_err("malformed stored canonical document must fail closed at startup");

    assert!(format!("{error:?}").contains("malformed canonical document"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_rejects_corrupt_database_bytes() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-corrupt-database-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("database root");
    let path = root.join("trust-logging.sqlite3");
    std::fs::write(&path, b"not a sqlite database").expect("seed corrupt database");

    let _error = open_test_sqlite(&path).expect_err("corrupt database must fail closed");
    std::fs::remove_dir_all(root).ok();
}

#[cfg(unix)]
#[test]
fn sqlite_sink_rejects_read_only_database_before_accepting_work() {
    use std::os::unix::fs::PermissionsExt;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-read-only-database-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");
    drop(open_test_sqlite(&path).expect("create valid database"));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400))
        .expect("make database read-only");

    let error = open_test_sqlite(&path).expect_err("read-only database must fail closed");

    assert!(format!("{error:?}").contains("read-only"));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_uncommitted_child_transaction_body() {
    let Ok(path) = std::env::var("TRUST_OPENOT_SQLITE_CRASH_CHILD_PATH") else {
        return;
    };
    let connection = rusqlite::Connection::open(path).expect("open child database");
    connection
        .execute_batch(
            "BEGIN IMMEDIATE; \
             UPDATE logging_records SET canonical_json='partial-child-write'; \
             UPDATE logging_checkpoint SET cursor_abs=X'000000000000270F';",
        )
        .expect("stage uncommitted child transaction");
    std::process::exit(86);
}

#[test]
fn sqlite_process_termination_recovers_before_or_after_batch_never_partial() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-sqlite-crash-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("crash root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure crash root");
    }
    let path = root.join("trust-logging.sqlite3");
    let baseline = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 4096,
        },
    };
    let mut sink = open_test_sqlite(&path).expect("open baseline database");
    sink.commit(&baseline).expect("commit baseline");
    drop(sink);
    let child = std::process::Command::new(std::env::current_exe().expect("current test binary"))
        .args([
            "--exact",
            "openot_persistence::contract_tests::sqlite::sqlite_uncommitted_child_transaction_body",
            "--nocapture",
        ])
        .env("TRUST_OPENOT_SQLITE_CRASH_CHILD_PATH", &path)
        .status()
        .expect("run crash child");
    assert_eq!(child.code(), Some(86));

    let mut recovered = open_test_sqlite(&path).expect("recover after child exit");
    assert_eq!(
        recovered
            .load_checkpoint(7, 1)
            .expect("recovered checkpoint"),
        Some(baseline.checkpoint)
    );
    let connection = rusqlite::Connection::open(&path).expect("inspect recovered documents");
    let partial: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM logging_records WHERE canonical_json='partial-child-write'",
            [],
            |row| row.get(0),
        )
        .expect("partial row count");
    assert_eq!(partial, 0);
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM logging_records", [], |row| row.get(0))
        .expect("recovered document count");
    assert_eq!(count, CANONICAL_DOCUMENT_COUNT as i64);
    drop(connection);
    std::fs::remove_dir_all(root).ok();
}

#[cfg(all(feature = "openot-real-database-tests", target_os = "linux"))]
#[test]
fn sqlite_disk_full_on_isolated_bounded_filesystem_preserves_last_checkpoint() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    struct MountGuard(std::path::PathBuf);
    impl Drop for MountGuard {
        fn drop(&mut self) {
            let _ = std::process::Command::new("sudo")
                .args(["umount", self.0.to_string_lossy().as_ref()])
                .status();
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-bounded-fs-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("bounded mountpoint");
    let owner = std::fs::metadata(&root).expect("mountpoint metadata");
    let mounted = std::process::Command::new("sudo")
        .args([
            "mount",
            "-t",
            "tmpfs",
            "-o",
            "size=1m,nosuid,nodev,noexec",
            "tmpfs",
            root.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("mount bounded tmpfs");
    assert!(mounted.success());
    let _guard = MountGuard(root.clone());
    let owned = std::process::Command::new("sudo")
        .args([
            "chown",
            &format!("{}:{}", owner.uid(), owner.gid()),
            root.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("own bounded tmpfs");
    assert!(owned.success());
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("secure bounded tmpfs");
    let path = root.join("trust-logging.sqlite3");
    let mut sink = open_test_sqlite(&path).expect("schema fits bounded filesystem");
    let mut last_checkpoint = None;
    let mut full_error = None;
    for run_id in 1..=10_000u64 {
        let batch = PersistenceBatch {
            documents: canonical_documents_for_run(run_id),
            checkpoint: PersistenceCheckpoint {
                buffer_id: 7,
                run_id,
                cursor_abs: run_id.saturating_mul(4096),
            },
        };
        match sink.commit(&batch) {
            Ok(_) => last_checkpoint = Some(batch.checkpoint),
            Err(error) => {
                full_error = Some((batch.checkpoint, error));
                break;
            }
        }
    }
    let (failed_checkpoint, error) = full_error.expect("bounded filesystem must become full");
    assert!(
        format!("{error:?}").to_ascii_lowercase().contains("full"),
        "expected explicit full-disk error, got {error:?}"
    );
    assert_ne!(last_checkpoint, Some(failed_checkpoint));
    if let Some(last_checkpoint) = last_checkpoint {
        assert_eq!(
            sink.load_checkpoint(last_checkpoint.buffer_id, last_checkpoint.run_id)
                .expect("checkpoint remains readable after full disk"),
            Some(last_checkpoint)
        );
        assert_eq!(
            sink.load_checkpoint(failed_checkpoint.buffer_id, failed_checkpoint.run_id)
                .expect("failed checkpoint remains absent"),
            None
        );
    }
}

#[test]
fn sqlite_sink_creates_missing_parent_directory_for_configured_path() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-parent-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("history/trust-logging.sqlite3");

    let sink = open_test_sqlite(&path).expect("create SQLite parent and database");

    assert!(path.is_file());
    drop(sink);
    std::fs::remove_dir_all(root).ok();
}

#[cfg(unix)]
#[test]
fn sqlite_sink_rejects_group_or_world_writable_parent() {
    use std::os::unix::fs::PermissionsExt;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-insecure-parent-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create insecure parent");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777))
        .expect("set insecure permissions");

    let result = open_test_sqlite(&root.join("trust-logging.sqlite3"));

    assert!(matches!(
        result,
        Err(super::PersistenceError::InvalidConfig(_))
    ));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn sqlite_sink_commits_documents_and_checkpoint_in_one_real_transaction() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-commit-{}-{stamp}",
        std::process::id()
    ));
    let path = root.join("trust-logging.sqlite3");
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 8192,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = open_test_sqlite(&path).expect("open SQLite sink");

    let outcome = sink.commit(&batch).expect("commit SQLite batch");
    drop(sink);

    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.checkpoint, checkpoint);
    let connection = rusqlite::Connection::open(&path).expect("independent SQLite inspection");
    let document_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM logging_records", [], |row| row.get(0))
        .expect("document count");
    let (buffer_id, cursor_abs): (u32, Vec<u8>) = connection
        .query_row(
            "SELECT buffer_id, cursor_abs FROM logging_checkpoint WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("checkpoint");
    assert_eq!(document_count, CANONICAL_DOCUMENT_COUNT as i64);
    let canonical_jsons = connection
        .prepare("SELECT canonical_json FROM logging_records ORDER BY identity_key")
        .expect("prepare canonical SQLite query")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query canonical SQLite documents")
        .collect::<Result<Vec<_>, _>>()
        .expect("decode canonical SQLite documents");
    assert_canonical_jsons(canonical_jsons);
    assert_eq!(buffer_id, checkpoint.buffer_id);
    assert_eq!(cursor_abs, checkpoint.cursor_abs.to_be_bytes());
    drop(connection);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sink_factory_opens_only_toml_selected_sqlite_at_bundle_relative_path() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-persistence-factory-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("history")).expect("create test bundle");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.join("history"), std::fs::Permissions::from_mode(0o700))
            .expect("secure test database directory");
    }
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::Sqlite),
        sqlite: Some(crate::config::OpenOtSqlitePersistenceConfig {
            path: "history/trust-logging.sqlite3".into(),
        }),
        ..Default::default()
    };

    let sink = OpenOtDocumentSink::open(&config, &root).expect("open selected SQLite sink");

    assert!(matches!(sink, OpenOtDocumentSink::Sqlite(_)));
    assert!(root.join("history/trust-logging.sqlite3").is_file());
    drop(sink);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(feature = "openot-database-postgresql"))]
#[test]
fn sink_factory_rejects_recognized_backend_omitted_from_binary_without_fallback() {
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::PostgreSql),
        postgresql: Some(crate::config::OpenOtPostgreSqlPersistenceConfig {
            connection_url_env: "TRUST_TEST_MUST_NOT_BE_READ".into(),
            schema: "trust_logging".into(),
            tls: crate::config::OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some("unused-ca.pem".into()),
        }),
        ..Default::default()
    };

    let error = OpenOtDocumentSink::open(&config, std::path::Path::new("/"))
        .expect_err("an omitted recognized backend must fail before reading its settings");

    assert_eq!(
        error.to_string(),
        "backend_not_available: runtime.openot.persistence.backend 'postgresql' is not compiled into this binary"
    );
}
