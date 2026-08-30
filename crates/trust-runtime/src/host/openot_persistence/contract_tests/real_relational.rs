use super::*;
#[cfg(feature = "openot-real-database-tests")]
#[test]
fn postgresql_sink_connects_to_real_tls_server_and_applies_schema_v3_read_model() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_POSTGRES_URL")
        .expect("TRUST_TEST_OPENOT_POSTGRES_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA")
        .expect("TRUST_TEST_OPENOT_POSTGRES_CA must identify its CA certificate");

    let mut sink = PostgreSqlDocumentSink::open(
        &connection_url,
        "openot",
        std::path::Path::new(&ca_cert_path),
    )
    .expect("connect and migrate real PostgreSQL");

    assert_eq!(sink.schema_version().expect("PostgreSQL schema version"), 3);
    let public_objects: i64 = sink
        .client
        .query_one(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema=$1 AND table_name IN ('event_log','logged_values','alarm_history')",
            &[&sink.schema],
        )
        .expect("inspect PostgreSQL public read model")
        .get(0);
    assert_eq!(public_objects, 3);
    let internal_objects: i64 = sink
        .client
        .query_one(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema=$1 AND table_name IN ('logging_schema','logging_records','logging_checkpoint')",
            &[&sink.schema],
        )
        .expect("inspect PostgreSQL internal logging objects")
        .get(0);
    assert_eq!(internal_objects, 3);
    let legacy_objects: i64 = sink
        .client
        .query_one(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema=$1 AND table_name LIKE 'openot_%'",
            &[&sink.schema],
        )
        .expect("inspect PostgreSQL legacy OpenOT names")
        .get(0);
    assert_eq!(legacy_objects, 0);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn postgresql_sink_commits_documents_and_checkpoint_on_real_server() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_POSTGRES_URL")
        .expect("TRUST_TEST_OPENOT_POSTGRES_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA")
        .expect("TRUST_TEST_OPENOT_POSTGRES_CA must identify its CA certificate");
    let schema = format!("openot_commit_{}", std::process::id());
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 12288,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = PostgreSqlDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca_cert_path),
        vec![open_ot_definition::sample_definition()],
    )
    .expect("connect and migrate real PostgreSQL");

    let outcome = sink.commit(&batch).expect("commit PostgreSQL batch");

    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.checkpoint, checkpoint);
    assert_eq!(
        sink.document_count().expect("document count"),
        CANONICAL_DOCUMENT_COUNT as i64
    );
    assert_eq!(
        sink.checkpoint().expect("checkpoint"),
        Some((
            checkpoint.buffer_id,
            checkpoint.run_id.to_be_bytes().to_vec(),
            checkpoint.cursor_abs.to_be_bytes().to_vec()
        ))
    );
    for (table, expected) in [
        ("event_log", 35_i64),
        ("logged_values", 2),
        ("alarm_history", 13),
        ("message_log", 1),
        ("state_history", 1),
        ("batch_history", 1),
        ("recipe_history", 2),
        ("material_additions", 1),
        ("operator_activity", 4),
        ("audit_log", 1),
        ("electronic_signatures", 1),
        ("system_events", 9),
        ("data_loss", 1),
        ("unresolved_records", 1),
    ] {
        let count: i64 = sink
            .client
            .query_one(
                &format!("SELECT COUNT(*) FROM \"{}\".{table}", sink.schema),
                &[],
            )
            .unwrap_or_else(|error| panic!("query PostgreSQL {table}: {error}"))
            .get(0);
        assert_eq!(count, expected, "PostgreSQL {table} projection count");
    }
    assert_canonical_jsons(
        sink.canonical_jsons()
            .expect("canonical PostgreSQL documents"),
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sink_factory_opens_only_toml_selected_postgresql() {
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA")
        .expect("TRUST_TEST_OPENOT_POSTGRES_CA must identify its CA certificate");
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::PostgreSql),
        postgresql: Some(crate::config::OpenOtPostgreSqlPersistenceConfig {
            connection_url_env: "TRUST_TEST_OPENOT_POSTGRES_URL".into(),
            schema: format!("openot_factory_{}", std::process::id()).into(),
            tls: crate::config::OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some(ca_cert_path.into()),
        }),
        ..Default::default()
    };

    let sink = OpenOtDocumentSink::open(&config, std::path::Path::new("/"))
        .expect("open selected PostgreSQL sink");

    assert!(matches!(sink, OpenOtDocumentSink::PostgreSql(_)));
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn timescaledb_sink_requires_real_extension_and_creates_hypertable() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_TIMESCALE_URL")
        .expect("TRUST_TEST_OPENOT_TIMESCALE_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_TIMESCALE_CA")
        .expect("TRUST_TEST_OPENOT_TIMESCALE_CA must identify its CA certificate");
    let schema = format!("openot_timescale_{}", std::process::id());

    let mut sink = TimescaleDbDocumentSink::open(
        &connection_url,
        &schema,
        std::path::Path::new(&ca_cert_path),
    )
    .expect("connect and migrate real TimescaleDB");

    assert_eq!(
        sink.extension_version().expect("extension version"),
        "2.29.2"
    );
    assert!(sink.hypertable_exists().expect("hypertable query"));
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sink_factory_selects_timescaledb_and_commits_to_real_hypertable() {
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_TIMESCALE_CA")
        .expect("TRUST_TEST_OPENOT_TIMESCALE_CA must identify its CA certificate");
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::TimescaleDb),
        timescaledb: Some(crate::config::OpenOtTimescaleDbPersistenceConfig {
            connection_url_env: "TRUST_TEST_OPENOT_TIMESCALE_URL".into(),
            schema: format!("openot_ts_factory_{}", std::process::id()).into(),
            tls: crate::config::OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some(ca_cert_path.into()),
        }),
        ..Default::default()
    };
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 16384,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = OpenOtDocumentSink::open_with_definitions(
        &config,
        std::path::Path::new("/"),
        &[open_ot_definition::sample_definition()],
    )
    .expect("open selected TimescaleDB sink");

    let outcome = sink.commit(&batch).expect("commit TimescaleDB batch");
    let OpenOtDocumentSink::TimescaleDb(timescale) = &mut sink else {
        panic!("TOML selection did not construct TimescaleDB");
    };
    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.checkpoint, checkpoint);
    assert_eq!(timescale.time_index_count().expect("time index count"), 35);
    assert_canonical_jsons(
        timescale
            .canonical_jsons()
            .expect("canonical TimescaleDB documents"),
    );
}

#[cfg(feature = "openot-real-database-tests")]
fn assert_mysql_protocol_product(url_env: &str, ca_env: &str, expected_version_fragment: &str) {
    let connection_url = std::env::var(url_env)
        .unwrap_or_else(|_| panic!("{url_env} must identify the reviewed real server"));
    let ca_cert_path = std::env::var(ca_env)
        .unwrap_or_else(|_| panic!("{ca_env} must identify its CA certificate"));
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 20480,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "openot",
        std::path::Path::new(&ca_cert_path),
        vec![open_ot_definition::sample_definition()],
    )
    .expect("connect and migrate real MySQL-protocol server");

    sink.reset_test_state()
        .expect("reset reviewed test database");
    assert_eq!(sink.schema_version().expect("schema version"), 3);
    assert_eq!(
        sink.internal_name_counts().expect("internal logging names"),
        (3, 0)
    );
    assert!(sink
        .server_version()
        .expect("server version")
        .contains(expected_version_fragment));
    assert_eq!(
        sink.identity_collation().expect("identity collation"),
        "ascii_bin",
        "document identity must use bytewise collation on both MySQL and MariaDB"
    );
    let outcome = sink.commit(&batch).expect("commit document batch");
    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.checkpoint, checkpoint);
    assert_eq!(
        sink.document_count().expect("document count"),
        CANONICAL_DOCUMENT_COUNT as u64
    );
    assert_eq!(sink.public_count("event_log").expect("event count"), 35);
    assert_eq!(sink.public_count("logged_values").expect("value count"), 2);
    assert_eq!(sink.public_count("alarm_history").expect("alarm count"), 13);
    for (table, expected) in [
        ("message_log", 1),
        ("state_history", 1),
        ("batch_history", 1),
        ("recipe_history", 2),
        ("material_additions", 1),
        ("operator_activity", 4),
        ("audit_log", 1),
        ("electronic_signatures", 1),
        ("system_events", 9),
        ("data_loss", 1),
        ("unresolved_records", 1),
    ] {
        assert_eq!(
            sink.public_count(table)
                .unwrap_or_else(|error| panic!("query MySQL-protocol {table}: {error}")),
            expected,
            "MySQL-protocol {table} projection count"
        );
    }
    assert_eq!(
        sink.checkpoint().expect("checkpoint"),
        Some((
            checkpoint.buffer_id,
            checkpoint.run_id.to_be_bytes().to_vec(),
            checkpoint.cursor_abs.to_be_bytes().to_vec()
        ))
    );
    assert_canonical_jsons(sink.canonical_jsons().expect("canonical MySQL documents"));
    let retried = sink.commit(&batch).expect("retry identical document batch");
    assert_eq!(retried.inserted, 0);
    assert_eq!(retried.duplicated, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(
        sink.document_count().expect("idempotent document count"),
        CANONICAL_DOCUMENT_COUNT as u64
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_sink_migrates_and_commits_on_real_mysql_8_4_lts() {
    assert_mysql_protocol_product(
        "TRUST_TEST_OPENOT_MYSQL_URL",
        "TRUST_TEST_OPENOT_MYSQL_CA",
        "8.4.11",
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_sink_migrates_and_commits_on_real_mariadb_11_8_lts() {
    assert_mysql_protocol_product(
        "TRUST_TEST_OPENOT_MARIADB_URL",
        "TRUST_TEST_OPENOT_MARIADB_CA",
        "11.8.8-MariaDB",
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_sink_backfills_populated_v2_with_shared_projector() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_MYSQL_URL").expect("MySQL URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_MYSQL_CA").expect("MySQL CA");
    let definition = open_ot_definition::sample_definition();
    let mut sink = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "openot",
        std::path::Path::new(&ca),
        vec![definition.clone()],
    )
    .expect("open MySQL v3 seed");
    sink.reset_test_state().expect("reset MySQL v2 seed");
    sink.commit(&PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 4096,
        },
    })
    .expect("seed populated MySQL history");
    sink.seed_v2_without_projections()
        .expect("seed MySQL schema v2 projection gap");
    drop(sink);

    let mut migrated = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "openot",
        std::path::Path::new(&ca),
        vec![definition],
    )
    .expect("migrate populated MySQL v2");
    assert_eq!(migrated.schema_version().expect("migrated version"), 3);
    assert_eq!(
        migrated
            .public_count("event_log")
            .expect("backfilled events"),
        35
    );
    assert_eq!(
        migrated
            .public_count("logged_values")
            .expect("backfilled values"),
        2
    );
    assert_eq!(
        migrated
            .public_count("alarm_history")
            .expect("backfilled alarms"),
        13
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mariadb_sink_backfills_populated_v2_with_shared_projector() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_MARIADB_URL").expect("MariaDB URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_MARIADB_CA").expect("MariaDB CA");
    let definition = open_ot_definition::sample_definition();
    let mut sink = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "openot",
        std::path::Path::new(&ca),
        vec![definition.clone()],
    )
    .expect("open MariaDB v3 seed");
    sink.reset_test_state().expect("reset MariaDB v2 seed");
    sink.commit(&PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 4096,
        },
    })
    .expect("seed populated MariaDB history");
    sink.seed_v2_without_projections()
        .expect("seed MariaDB schema v2 projection gap");
    drop(sink);
    let mut migrated = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "openot",
        std::path::Path::new(&ca),
        vec![definition],
    )
    .expect("migrate populated MariaDB v2");
    assert_eq!(migrated.schema_version().expect("migrated version"), 3);
    assert_eq!(
        migrated
            .public_count("event_log")
            .expect("backfilled events"),
        35
    );
    assert_eq!(
        migrated
            .public_count("logged_values")
            .expect("backfilled values"),
        2
    );
    assert_eq!(
        migrated
            .public_count("alarm_history")
            .expect("backfilled alarms"),
        13
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sink_factory_opens_toml_selected_mysql_adapter() {
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_MYSQL_CA")
        .expect("TRUST_TEST_OPENOT_MYSQL_CA must identify its CA certificate");
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::MySql),
        mysql: Some(crate::config::OpenOtMySqlPersistenceConfig {
            connection_url_env: "TRUST_TEST_OPENOT_MYSQL_URL".into(),
            database: "openot".into(),
            tls: crate::config::OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some(ca_cert_path.into()),
        }),
        ..Default::default()
    };

    let sink = OpenOtDocumentSink::open(&config, std::path::Path::new("/"))
        .expect("open selected MySQL sink");

    assert!(matches!(sink, OpenOtDocumentSink::MySql(_)));
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sqlserver_sink_migrates_and_commits_on_real_sql_server_2025() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_CA must identify its CA certificate");
    let schema = format!("openot_{}", std::process::id());
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 24576,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = SqlServerDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca_cert_path),
        vec![open_ot_definition::sample_definition()],
    )
    .expect("connect and migrate real SQL Server");

    assert_eq!(sink.schema_version().expect("schema version"), 3);
    assert_eq!(
        sink.internal_name_counts().expect("internal logging names"),
        (3, 0)
    );
    assert!(sink
        .product_version()
        .expect("product version")
        .starts_with("17.0.4075.5"));
    let outcome = sink.commit(&batch).expect("commit SQL Server batch");
    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.checkpoint, checkpoint);
    assert_eq!(
        sink.document_count().expect("document count"),
        CANONICAL_DOCUMENT_COUNT as i64
    );
    assert_eq!(
        sink.invalid_json_count().expect("canonical JSON validity"),
        0
    );
    assert_eq!(sink.public_count("event_log").expect("event count"), 35);
    assert_eq!(sink.public_count("logged_values").expect("value count"), 2);
    assert_eq!(sink.public_count("alarm_history").expect("alarm count"), 13);
    for (table, expected) in [
        ("message_log", 1),
        ("state_history", 1),
        ("batch_history", 1),
        ("recipe_history", 2),
        ("material_additions", 1),
        ("operator_activity", 4),
        ("audit_log", 1),
        ("electronic_signatures", 1),
        ("system_events", 9),
        ("data_loss", 1),
        ("unresolved_records", 1),
    ] {
        assert_eq!(
            sink.public_count(table)
                .unwrap_or_else(|error| panic!("query SQL Server {table}: {error}")),
            expected,
            "SQL Server {table} projection count"
        );
    }
    assert_eq!(
        sink.checkpoint().expect("checkpoint"),
        Some((
            checkpoint.buffer_id,
            checkpoint.run_id.to_be_bytes().to_vec(),
            checkpoint.cursor_abs.to_be_bytes().to_vec()
        ))
    );
    assert_canonical_jsons(
        sink.canonical_jsons()
            .expect("canonical SQL Server documents"),
    );
    let retried = sink.commit(&batch).expect("idempotent retry");
    assert_eq!(retried.inserted, 0);
    assert_eq!(retried.duplicated, CANONICAL_DOCUMENT_COUNT);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sqlserver_sink_backfills_populated_v2_with_shared_projector() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL").expect("SQL Server URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA").expect("SQL Server CA");
    let schema = format!("openot_v2_{}", std::process::id());
    let definition = open_ot_definition::sample_definition();
    let mut sink = SqlServerDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca),
        vec![definition.clone()],
    )
    .expect("open SQL Server v3 seed");
    sink.commit(&PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 4096,
        },
    })
    .expect("seed populated SQL Server history");
    sink.seed_v2_without_projections()
        .expect("seed SQL Server schema v2 projection gap");
    drop(sink);

    let mut migrated = SqlServerDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca),
        vec![definition],
    )
    .expect("migrate populated SQL Server v2");
    assert_eq!(migrated.schema_version().expect("migrated version"), 3);
    assert_eq!(
        migrated
            .public_count("event_log")
            .expect("backfilled events"),
        35
    );
    assert_eq!(
        migrated
            .public_count("logged_values")
            .expect("backfilled values"),
        2
    );
    assert_eq!(
        migrated
            .public_count("alarm_history")
            .expect("backfilled alarms"),
        13
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sink_factory_opens_toml_selected_sqlserver_adapter() {
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_CA must identify its CA certificate");
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::SqlServer),
        sqlserver: Some(crate::config::OpenOtSqlServerPersistenceConfig {
            connection_url_env: "TRUST_TEST_OPENOT_SQLSERVER_URL".into(),
            schema: format!("openot_factory_{}", std::process::id()).into(),
            tls: crate::config::OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some(ca_cert_path.into()),
        }),
        ..Default::default()
    };

    let sink = OpenOtDocumentSink::open(&config, std::path::Path::new("/"))
        .expect("open selected SQL Server sink");

    assert!(matches!(sink, OpenOtDocumentSink::SqlServer(_)));
}
