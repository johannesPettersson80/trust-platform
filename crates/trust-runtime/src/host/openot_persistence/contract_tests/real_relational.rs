use super::*;
#[cfg(feature = "openot-real-database-tests")]
#[test]
fn postgresql_sink_connects_to_real_tls_server_and_initializes_generation_1() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_POSTGRES_URL")
        .expect("TRUST_TEST_OPENOT_POSTGRES_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA")
        .expect("TRUST_TEST_OPENOT_POSTGRES_CA must identify its CA certificate");

    let mut sink = PostgreSqlDocumentSink::open(
        &connection_url,
        "trust_logging",
        std::path::Path::new(&ca_cert_path),
    )
    .expect("connect and initialize real PostgreSQL");

    assert_eq!(
        sink.schema_version().expect("PostgreSQL schema generation"),
        1
    );
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
    let schema = format!("logging_commit_{}", std::process::id());
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
    .expect("connect and initialize real PostgreSQL");

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
    let audited = sink
        .client
        .query_one(
            &format!(
                "SELECT previous_boolean_value,is_audited,actor,reason,authorization_result \
                 FROM \"{}\".logged_values WHERE is_audited LIMIT 1",
                sink.schema
            ),
            &[],
        )
        .expect("query PostgreSQL audited value projection");
    assert_eq!(audited.get::<_, Option<bool>>(0), Some(false));
    assert!(audited.get::<_, bool>(1));
    assert_eq!(
        audited.get::<_, Option<String>>(2).as_deref(),
        Some("operator-a")
    );
    assert_eq!(
        audited.get::<_, Option<String>>(3).as_deref(),
        Some("approved change")
    );
    assert_eq!(
        audited.get::<_, Option<String>>(4).as_deref(),
        Some("authorized")
    );
    assert_canonical_jsons(
        sink.canonical_jsons()
            .expect("canonical PostgreSQL documents"),
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn postgresql_sink_rejects_changed_generation_1_catalog() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_POSTGRES_URL").expect("PostgreSQL URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA").expect("PostgreSQL CA");
    let schema = format!("logging_catalog_{}", std::process::id());
    let mut sink =
        PostgreSqlDocumentSink::open(&connection_url, &schema, std::path::Path::new(&ca))
            .expect("initialize PostgreSQL catalog fixture");
    sink.client
        .batch_execute(&format!(
            "DROP INDEX \"{schema}\".logging_records_receive_time"
        ))
        .expect("damage required PostgreSQL index");
    let reopened =
        PostgreSqlDocumentSink::open(&connection_url, &schema, std::path::Path::new(&ca));
    sink.client
        .batch_execute(&format!(
            "CREATE INDEX logging_records_receive_time ON \"{schema}\".logging_records(receive_time_ns)"
        ))
        .expect("restore required PostgreSQL index");
    let error = reopened.expect_err("changed PostgreSQL catalog must fail closed");
    assert!(format!("{error:?}").contains("incompatible pre-release schema"));
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn postgresql_sink_allows_unrelated_objects_in_shared_schema() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_POSTGRES_URL").expect("PostgreSQL URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA").expect("PostgreSQL CA");
    let schema = format!("logging_unrelated_{}", std::process::id());
    let mut sink =
        PostgreSqlDocumentSink::open(&connection_url, &schema, std::path::Path::new(&ca))
            .expect("initialize shared PostgreSQL schema");
    sink.client
        .batch_execute(&format!(
            "CREATE TABLE \"{schema}\".operator_owned_notes(id BIGINT PRIMARY KEY)"
        ))
        .expect("add unrelated PostgreSQL table");

    PostgreSqlDocumentSink::open(&connection_url, &schema, std::path::Path::new(&ca))
        .expect("unrelated PostgreSQL table must not change the logging contract");
    sink.client
        .batch_execute(&format!("DROP TABLE \"{schema}\".operator_owned_notes"))
        .expect("remove unrelated PostgreSQL table");
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
            schema: format!("logging_factory_{}", std::process::id()).into(),
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
    let schema = format!("logging_timescale_{}", std::process::id());

    let mut sink = TimescaleDbDocumentSink::open(
        &connection_url,
        &schema,
        std::path::Path::new(&ca_cert_path),
    )
    .expect("connect and initialize real TimescaleDB");

    assert_eq!(
        sink.extension_version().expect("extension version"),
        "2.29.2"
    );
    assert_eq!(sink.schema_version().expect("schema generation"), 1);
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
            schema: format!("logging_ts_factory_{}", std::process::id()).into(),
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
    assert_eq!(
        timescale
            .audited_value_projection()
            .expect("TimescaleDB audited value projection"),
        (
            Some(false),
            true,
            Some("operator-a".into()),
            Some("approved change".into()),
            Some("authorized".into()),
        )
    );
    assert_canonical_jsons(
        timescale
            .canonical_jsons()
            .expect("canonical TimescaleDB documents"),
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn timescaledb_sink_rejects_changed_generation_1_catalog() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_TIMESCALE_URL").expect("TimescaleDB URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_TIMESCALE_CA").expect("TimescaleDB CA");
    let schema = format!("logging_catalog_{}", std::process::id());
    let mut sink =
        TimescaleDbDocumentSink::open(&connection_url, &schema, std::path::Path::new(&ca))
            .expect("initialize TimescaleDB catalog fixture");
    sink.set_required_index_present_for_test(false)
        .expect("damage required TimescaleDB index");
    let reopened =
        TimescaleDbDocumentSink::open(&connection_url, &schema, std::path::Path::new(&ca));
    sink.set_required_index_present_for_test(true)
        .expect("restore required TimescaleDB index");
    let error = reopened.expect_err("changed TimescaleDB catalog must fail closed");
    assert!(format!("{error:?}").contains("incompatible pre-release schema"));
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
        "trust_logging",
        std::path::Path::new(&ca_cert_path),
        vec![open_ot_definition::sample_definition()],
    )
    .expect("connect and initialize real MySQL-protocol server");

    sink.reset_test_state()
        .expect("reset reviewed test database");
    assert_eq!(sink.schema_version().expect("schema generation"), 1);
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
    assert_eq!(
        sink.audited_value_projection()
            .expect("MySQL-protocol audited value projection"),
        (
            Some(false),
            true,
            Some("operator-a".into()),
            Some("approved change".into()),
            Some("authorized".into()),
        )
    );
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
fn mysql_sink_initializes_and_commits_on_real_mysql_8_4_lts() {
    assert_mysql_protocol_product(
        "TRUST_TEST_OPENOT_MYSQL_URL",
        "TRUST_TEST_OPENOT_MYSQL_CA",
        "8.4.11",
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_sink_initializes_and_commits_on_real_mariadb_11_8_lts() {
    assert_mysql_protocol_product(
        "TRUST_TEST_OPENOT_MARIADB_URL",
        "TRUST_TEST_OPENOT_MARIADB_CA",
        "11.8.8-MariaDB",
    );
}

#[cfg(feature = "openot-real-database-tests")]
fn assert_mysql_protocol_schema_marker_rejects_non_singleton(url_env: &str, ca_env: &str) {
    use mysql::{prelude::Queryable, Conn, Opts, OptsBuilder, SslOpts};

    let connection_url = std::env::var(url_env).unwrap_or_else(|_| panic!("{url_env} is required"));
    let ca = std::env::var(ca_env).unwrap_or_else(|_| panic!("{ca_env} is required"));
    MySqlDocumentSink::open(&connection_url, "trust_logging", std::path::Path::new(&ca))
        .expect("initialize MySQL-protocol schema marker");
    let options = Opts::from_url(&connection_url).expect("parse MySQL-protocol test URL");
    let ssl = SslOpts::default().with_root_cert_path(Some(std::path::PathBuf::from(ca)));
    let options = OptsBuilder::from_opts(options).ssl_opts(Some(ssl));
    let mut connection = Conn::new(options).expect("connect to MySQL-protocol test server");
    connection
        .query_drop("USE `trust_logging`")
        .expect("select MySQL-protocol test database");
    let result = connection.query_drop(
        "INSERT INTO logging_schema(singleton,version,catalog_fingerprint) \
         VALUES(2,1,REPEAT('0',64))",
    );
    let _ = connection.query_drop("DELETE FROM logging_schema WHERE singleton=2");

    assert!(
        result.is_err(),
        "MySQL/MariaDB logging_schema must reject singleton=2"
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_schema_marker_rejects_non_singleton_on_real_server() {
    assert_mysql_protocol_schema_marker_rejects_non_singleton(
        "TRUST_TEST_OPENOT_MYSQL_URL",
        "TRUST_TEST_OPENOT_MYSQL_CA",
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mariadb_schema_marker_rejects_non_singleton_on_real_server() {
    assert_mysql_protocol_schema_marker_rejects_non_singleton(
        "TRUST_TEST_OPENOT_MARIADB_URL",
        "TRUST_TEST_OPENOT_MARIADB_CA",
    );
}

#[cfg(feature = "openot-real-database-tests")]
fn assert_mysql_protocol_rejects_changed_catalog(url_env: &str, ca_env: &str) {
    let connection_url = std::env::var(url_env).unwrap_or_else(|_| panic!("{url_env} is required"));
    let ca = std::env::var(ca_env).unwrap_or_else(|_| panic!("{ca_env} is required"));
    let mut sink =
        MySqlDocumentSink::open(&connection_url, "trust_logging", std::path::Path::new(&ca))
            .expect("initialize MySQL-protocol catalog fixture");
    sink.set_required_index_present_for_test(false)
        .expect("damage required MySQL-protocol index");
    let reopened =
        MySqlDocumentSink::open(&connection_url, "trust_logging", std::path::Path::new(&ca));
    sink.set_required_index_present_for_test(true)
        .expect("restore required MySQL-protocol index");
    let error = reopened.expect_err("changed MySQL-protocol catalog must fail closed");
    assert!(format!("{error:?}").contains("incompatible pre-release schema"));
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_sink_rejects_changed_generation_1_catalog() {
    assert_mysql_protocol_rejects_changed_catalog(
        "TRUST_TEST_OPENOT_MYSQL_URL",
        "TRUST_TEST_OPENOT_MYSQL_CA",
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mariadb_sink_rejects_changed_generation_1_catalog() {
    assert_mysql_protocol_rejects_changed_catalog(
        "TRUST_TEST_OPENOT_MARIADB_URL",
        "TRUST_TEST_OPENOT_MARIADB_CA",
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mysql_sink_rejects_populated_incompatible_pre_release_schema() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_MYSQL_URL").expect("MySQL URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_MYSQL_CA").expect("MySQL CA");
    let definition = open_ot_definition::sample_definition();
    let mut sink = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "trust_logging",
        std::path::Path::new(&ca),
        vec![definition.clone()],
    )
    .expect("open MySQL generation-1 seed");
    sink.reset_test_state().expect("reset MySQL seed");
    sink.commit(&PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 4096,
        },
    })
    .expect("seed populated MySQL history");
    sink.seed_incompatible_generation_for_test()
        .expect("seed incompatible MySQL generation");
    let error = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "trust_logging",
        std::path::Path::new(&ca),
        vec![definition],
    )
    .expect_err("populated incompatible MySQL schema must fail closed");
    assert!(format!("{error:?}").contains("incompatible pre-release"));
    sink.set_schema_version_for_test(1)
        .expect("restore generation 1");
    sink.reset_test_state().expect("clear rejected fixture");
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn mariadb_sink_rejects_populated_incompatible_pre_release_schema() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_MARIADB_URL").expect("MariaDB URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_MARIADB_CA").expect("MariaDB CA");
    let definition = open_ot_definition::sample_definition();
    let mut sink = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "trust_logging",
        std::path::Path::new(&ca),
        vec![definition.clone()],
    )
    .expect("open MariaDB generation-1 seed");
    sink.reset_test_state().expect("reset MariaDB seed");
    sink.commit(&PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 4096,
        },
    })
    .expect("seed populated MariaDB history");
    sink.seed_incompatible_generation_for_test()
        .expect("seed incompatible MariaDB generation");
    let error = MySqlDocumentSink::open_with_definitions(
        &connection_url,
        "trust_logging",
        std::path::Path::new(&ca),
        vec![definition],
    )
    .expect_err("populated incompatible MariaDB schema must fail closed");
    assert!(format!("{error:?}").contains("incompatible pre-release"));
    sink.set_schema_version_for_test(1)
        .expect("restore generation 1");
    sink.reset_test_state().expect("clear rejected fixture");
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
            database: "trust_logging".into(),
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
fn sqlserver_full_projection_batch_stays_below_real_parameter_limit() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_CA must identify its CA certificate");
    let schema = format!("logging_parameter_limit_{}", std::process::id());
    let definition = open_ot_definition::sample_definition();
    let documents = (1_u64..=2_101)
        .map(|sequence| {
            value_changed_document(
                &definition,
                sequence,
                2003,
                "Bool",
                serde_json::json!(sequence % 2 == 0),
            )
        })
        .collect::<Vec<_>>();
    let batch = PersistenceBatch {
        documents,
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 2_101 * 4096,
        },
    };
    let mut sink = SqlServerDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca_cert_path),
        vec![definition],
    )
    .expect("connect and initialize SQL Server parameter-limit fixture");

    let outcome = sink
        .commit(&batch)
        .expect("commit 2,101 projected documents without exceeding 2,100 parameters");

    assert_eq!(outcome.inserted, 2_101);
    assert_eq!(
        sink.public_count("logged_values")
            .expect("SQL Server logged-value count"),
        2_101
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sqlserver_projects_every_loss_and_unresolved_document_in_one_batch() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_CA must identify its CA certificate");
    let schema = format!("logging_special_batch_{}", std::process::id());
    let mut documents = canonical_documents_for_run(10_001);
    documents.extend(canonical_documents_for_run(10_002));
    let mut sink = SqlServerDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca_cert_path),
        vec![open_ot_definition::sample_definition()],
    )
    .expect("connect and initialize SQL Server special-document fixture");

    let outcome = sink
        .commit(&PersistenceBatch {
            documents,
            checkpoint: PersistenceCheckpoint {
                buffer_id: 7,
                run_id: 10_002,
                cursor_abs: 74 * 4096,
            },
        })
        .expect("commit every loss and unresolved projection");

    assert_eq!(outcome.inserted, 2 * CANONICAL_DOCUMENT_COUNT);
    assert_eq!(sink.public_count("data_loss").expect("loss rows"), 2);
    assert_eq!(
        sink.public_count("unresolved_records")
            .expect("unresolved rows"),
        2
    );
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sqlserver_sink_initializes_and_commits_on_real_sql_server_2025() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_URL must identify the reviewed real server");
    let ca_cert_path = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA")
        .expect("TRUST_TEST_OPENOT_SQLSERVER_CA must identify its CA certificate");
    let schema = format!("logging_{}", std::process::id());
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 24576,
    };
    let mut documents = canonical_documents();
    for document in &mut documents {
        let provenance = match document {
            Document::Loss(document) => &mut document.provenance,
            Document::Placeholder(document) => &mut document.provenance,
            _ => continue,
        };
        provenance.source.path = vec!["Area1".into(), "Line2".into()];
        provenance.source.hierarchy = vec!["area".into(), "line".into()];
        provenance.flags.time_unsynced = true;
        provenance.flags.synthetic_record = true;
        provenance.flags.partial_payload = true;
    }
    let mut expected_jsons = documents
        .iter()
        .map(|document| open_ot_document::to_json(document).expect("serialize SQL fixture"))
        .collect::<Vec<_>>();
    expected_jsons.sort();
    let batch = PersistenceBatch {
        documents,
        checkpoint,
    };
    let mut sink = SqlServerDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca_cert_path),
        vec![open_ot_definition::sample_definition()],
    )
    .expect("connect and initialize real SQL Server");

    assert_eq!(sink.schema_version().expect("schema generation"), 1);
    sink.set_schema_version_for_test(99)
        .expect_err("SQL Server generation-1 marker must reject another value");
    assert_eq!(
        sink.schema_version().expect("unchanged schema generation"),
        1
    );
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
    for table in ["data_loss", "unresolved_records"] {
        assert_eq!(
            sink.public_provenance(table)
                .unwrap_or_else(|error| panic!("query SQL Server {table} provenance: {error}")),
            (
                "Area1/Line2".to_string(),
                "area/line".to_string(),
                true,
                true,
                true,
            ),
            "SQL Server {table} must preserve canonical provenance"
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
    let mut actual_jsons = sink
        .canonical_jsons()
        .expect("canonical SQL Server documents");
    actual_jsons.sort();
    assert_eq!(actual_jsons, expected_jsons);
    assert_eq!(
        sink.audited_value_projection()
            .expect("SQL Server audited value projection"),
        (
            Some(false),
            true,
            Some("operator-a".into()),
            Some("approved change".into()),
            Some("authorized".into()),
        )
    );
    let retried = sink.commit(&batch).expect("idempotent retry");
    assert_eq!(retried.inserted, 0);
    assert_eq!(retried.duplicated, CANONICAL_DOCUMENT_COUNT);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sqlserver_sink_rejects_populated_incompatible_pre_release_schema() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL").expect("SQL Server URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA").expect("SQL Server CA");
    let schema = format!("logging_incompatible_{}", std::process::id());
    let definition = open_ot_definition::sample_definition();
    let mut sink = SqlServerDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca),
        vec![definition.clone()],
    )
    .expect("open SQL Server generation-1 seed");
    sink.commit(&PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 4096,
        },
    })
    .expect("seed populated SQL Server history");
    sink.seed_incompatible_generation_for_test()
        .expect("seed incompatible SQL Server generation");
    let error = SqlServerDocumentSink::open_with_definitions(
        &connection_url,
        &schema,
        std::path::Path::new(&ca),
        vec![definition],
    )
    .expect_err("populated incompatible SQL Server schema must fail closed");
    assert!(format!("{error:?}").contains("incompatible pre-release"));
    sink.set_schema_version_for_test(1)
        .expect("restore generation 1");
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sqlserver_sink_rejects_changed_generation_1_catalog() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL").expect("SQL Server URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA").expect("SQL Server CA");
    let schema = format!("logging_catalog_{}", std::process::id());
    let mut sink = SqlServerDocumentSink::open(&connection_url, &schema, std::path::Path::new(&ca))
        .expect("initialize SQL Server catalog fixture");
    sink.set_required_index_present_for_test(false)
        .expect("damage required SQL Server index");
    let reopened = SqlServerDocumentSink::open(&connection_url, &schema, std::path::Path::new(&ca));
    sink.set_required_index_present_for_test(true)
        .expect("restore required SQL Server index");
    let error = reopened.expect_err("changed SQL Server catalog must fail closed");
    assert!(format!("{error:?}").contains("incompatible pre-release schema"));
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sqlserver_sink_allows_unrelated_objects_in_shared_schema() {
    let connection_url = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL").expect("SQL Server URL");
    let ca = std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA").expect("SQL Server CA");
    let schema = format!("logging_unrelated_{}", std::process::id());
    let mut sink = SqlServerDocumentSink::open(&connection_url, &schema, std::path::Path::new(&ca))
        .expect("initialize shared SQL Server schema");
    sink.set_unrelated_table_present_for_test(true)
        .expect("add unrelated SQL Server table");

    SqlServerDocumentSink::open(&connection_url, &schema, std::path::Path::new(&ca))
        .expect("unrelated SQL Server table must not change the logging contract");
    sink.set_unrelated_table_present_for_test(false)
        .expect("remove unrelated SQL Server table");
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
            schema: format!("logging_factory_{}", std::process::id()).into(),
            tls: crate::config::OpenOtPersistenceTlsMode::Require,
            ca_cert_path: Some(ca_cert_path.into()),
        }),
        ..Default::default()
    };

    let sink = OpenOtDocumentSink::open(&config, std::path::Path::new("/"))
        .expect("open selected SQL Server sink");

    assert!(matches!(sink, OpenOtDocumentSink::SqlServer(_)));
}
