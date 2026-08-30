use super::*;
#[cfg(feature = "openot-real-database-tests")]
#[test]
fn influxdb3_sink_spools_and_delivers_to_real_influxdb_3_core() {
    let host = std::env::var("TRUST_TEST_OPENOT_INFLUX_HOST")
        .expect("TRUST_TEST_OPENOT_INFLUX_HOST must identify the reviewed real server");
    let token = std::env::var("TRUST_TEST_OPENOT_INFLUX_TOKEN")
        .expect("TRUST_TEST_OPENOT_INFLUX_TOKEN must hold the reviewed test token");
    let ca = std::env::var("TRUST_TEST_OPENOT_INFLUX_CA")
        .expect("TRUST_TEST_OPENOT_INFLUX_CA must identify its CA certificate");
    let spool_root =
        std::env::temp_dir().join(format!("trust-openot-influx-spool-{}", std::process::id()));
    let spool = spool_root.join("spool.sqlite3");
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 28672,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = InfluxDb3DocumentSink::open_bounded_with_definitions(
        &host,
        &token,
        "openot",
        &spool,
        std::path::Path::new(&ca),
        u64::MAX,
        vec![open_ot_definition::sample_definition()],
    )
    .expect("open real InfluxDB 3 sink and spool");

    assert_eq!(sink.server_version().expect("server version"), "3.11.2");
    assert_eq!(
        sink.internal_name_counts().expect("InfluxDB spool names"),
        (4, 0)
    );
    let outcome = sink
        .commit(&batch)
        .expect("durably accept batch into spool");
    assert_eq!(outcome.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(outcome.duplicated, 0);
    assert_eq!(outcome.checkpoint, checkpoint);
    assert_eq!(outcome.remote_pending, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(
        outcome.unclassified_events, 0,
        "the canonical matrix contains only registry-classified event types"
    );
    assert_eq!(
        outcome.pending_parts,
        outcome.inserted + outcome.projection_rows_committed
    );
    assert_eq!(
        sink.pending_part_count().expect("pending delivery parts"),
        110,
        "each canonical and typed InfluxDB point must have durable per-part state"
    );
    let reconciliation = sink
        .maintenance_status()
        .expect("deliver accepted spool batch");
    assert_eq!(reconciliation.remote_pending, 0);
    assert_eq!(reconciliation.reconciled_parts, outcome.pending_parts);
    assert_eq!(reconciliation.pending_parts, 0);
    assert_eq!(sink.pending_count().expect("pending spool count"), 0);
    assert_eq!(
        sink.remote_document_count_for_run(u64::from(std::process::id()))
            .expect("remote document count"),
        CANONICAL_DOCUMENT_COUNT as i64
    );
    assert_eq!(
        sink.remote_measurement_count_for_run("message_log", u64::from(std::process::id()))
            .expect("InfluxDB message count"),
        1
    );
    for (measurement, expected) in [
        ("event_log", 35),
        ("logged_values", 2),
        ("alarm_history", 13),
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
            sink.remote_measurement_count_for_run(measurement, u64::from(std::process::id()),)
                .unwrap_or_else(|error| panic!("query InfluxDB {measurement}: {error}")),
            expected,
            "InfluxDB {measurement} projection count"
        );
    }
    assert_canonical_jsons(
        sink.remote_canonical_jsons_for_run(u64::from(std::process::id()))
            .expect("canonical InfluxDB documents"),
    );
    let retried = sink.commit(&batch).expect("idempotent InfluxDB retry");
    assert_eq!(retried.inserted, 0);
    assert_eq!(retried.duplicated, CANONICAL_DOCUMENT_COUNT);
    let _ = std::fs::remove_dir_all(spool_root);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn influxdb3_sink_accepts_during_outage_then_catches_up_in_order() {
    let host = std::env::var("TRUST_TEST_OPENOT_INFLUX_HOST").expect("real InfluxDB host");
    let token = std::env::var("TRUST_TEST_OPENOT_INFLUX_TOKEN").expect("real InfluxDB token");
    let ca = std::env::var("TRUST_TEST_OPENOT_INFLUX_CA").expect("real InfluxDB CA");
    let spool_root =
        std::env::temp_dir().join(format!("trust-openot-influx-outage-{}", std::process::id()));
    let spool = spool_root.join("spool.sqlite3");
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: 1,
        cursor_abs: 32768,
    };
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint,
    };
    let mut sink = InfluxDb3DocumentSink::open_bounded_with_definitions(
        &host,
        &token,
        "openot",
        &spool,
        std::path::Path::new(&ca),
        u64::MAX,
        vec![open_ot_definition::sample_definition()],
    )
    .expect("open InfluxDB sink before outage");

    sink.set_host_for_test("https://127.0.0.1:1");
    let accepted = sink
        .commit(&batch)
        .expect("local spool remains acceptance authority");
    assert_eq!(accepted.inserted, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(accepted.remote_pending, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(
        sink.pending_count().expect("pending during outage"),
        CANONICAL_DOCUMENT_COUNT as i64
    );
    sink.set_host_for_test(&host);
    assert_eq!(sink.maintenance().expect("catch up after recovery"), 0);
    assert_eq!(sink.pending_count().expect("pending after recovery"), 0);
    let _ = std::fs::remove_dir_all(spool_root);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn influxdb3_sink_rejects_a_spool_limit_smaller_than_its_schema() {
    let host = std::env::var("TRUST_TEST_OPENOT_INFLUX_HOST").expect("real InfluxDB host");
    let token = std::env::var("TRUST_TEST_OPENOT_INFLUX_TOKEN").expect("real InfluxDB token");
    let ca = std::env::var("TRUST_TEST_OPENOT_INFLUX_CA").expect("real InfluxDB CA");
    let spool_root = std::env::temp_dir().join(format!(
        "trust-openot-influx-tiny-spool-{}",
        std::process::id()
    ));
    let result = InfluxDb3DocumentSink::open_bounded(
        &host,
        &token,
        "openot",
        &spool_root.join("spool.sqlite3"),
        std::path::Path::new(&ca),
        1,
    );

    assert!(matches!(
        result,
        Err(super::PersistenceError::InvalidConfig(_))
    ));
    let _ = std::fs::remove_dir_all(spool_root);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn influxdb3_spool_full_rolls_back_documents_and_checkpoint() {
    let host = std::env::var("TRUST_TEST_OPENOT_INFLUX_HOST").expect("real InfluxDB host");
    let token = std::env::var("TRUST_TEST_OPENOT_INFLUX_TOKEN").expect("real InfluxDB token");
    let ca = std::env::var("TRUST_TEST_OPENOT_INFLUX_CA").expect("real InfluxDB CA");
    let spool_root = std::env::temp_dir().join(format!(
        "trust-openot-influx-full-spool-{}",
        std::process::id()
    ));
    let spool = spool_root.join("spool.sqlite3");
    let initial = InfluxDb3DocumentSink::open_bounded_with_definitions(
        &host,
        &token,
        "openot",
        &spool,
        std::path::Path::new(&ca),
        u64::MAX,
        vec![open_ot_definition::sample_definition()],
    )
    .expect("create baseline spool");
    let schema_bytes = initial.spool_logical_bytes().expect("schema footprint");
    drop(initial);
    let mut bounded = InfluxDb3DocumentSink::open_bounded_with_definitions(
        &host,
        &token,
        "openot",
        &spool,
        std::path::Path::new(&ca),
        schema_bytes,
        vec![open_ot_definition::sample_definition()],
    )
    .expect("open exactly schema-sized spool");
    let batch = PersistenceBatch {
        documents: canonical_documents(),
        checkpoint: PersistenceCheckpoint {
            buffer_id: 7,
            run_id: 1,
            cursor_abs: 36_864,
        },
    };

    assert!(matches!(
        bounded.commit(&batch),
        Err(super::PersistenceError::CapacityExhausted(_))
    ));
    assert_eq!(bounded.pending_count().expect("pending after rollback"), 0);
    assert_eq!(
        bounded
            .load_checkpoint(7, 1)
            .expect("checkpoint after rollback"),
        None
    );
    let _ = std::fs::remove_dir_all(spool_root);
}

#[cfg(feature = "openot-real-database-tests")]
#[test]
fn sink_factory_opens_toml_selected_influxdb3_adapter() {
    let ca = std::env::var("TRUST_TEST_OPENOT_INFLUX_CA").expect("real InfluxDB CA");
    let root = std::env::temp_dir().join(format!(
        "trust-openot-influx-factory-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create factory bundle root");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("secure test spool directory");
    }
    let config = crate::config::OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(crate::config::OpenOtPersistenceBackend::InfluxDb3),
        influxdb3: Some(crate::config::OpenOtInfluxDb3PersistenceConfig {
            host_env: "TRUST_TEST_OPENOT_INFLUX_HOST".into(),
            token_env: "TRUST_TEST_OPENOT_INFLUX_TOKEN".into(),
            database: "openot".into(),
            spool_path: "spool.sqlite3".into(),
            max_bytes: 1_073_741_824,
            ca_cert_path: Some(ca.into()),
        }),
        ..Default::default()
    };

    let mut sink = OpenOtDocumentSink::open_with_definitions(
        &config,
        &root,
        &[open_ot_definition::sample_definition()],
    )
    .expect("open selected InfluxDB 3 sink");
    assert!(matches!(sink, OpenOtDocumentSink::InfluxDb3(_)));
    let checkpoint = PersistenceCheckpoint {
        buffer_id: 7,
        run_id: u64::from(std::process::id()),
        cursor_abs: 49_152,
    };
    let outcome = sink
        .commit(&PersistenceBatch {
            documents: canonical_documents(),
            checkpoint,
        })
        .expect("factory-selected Influx sink accepts the canonical batch into its spool");
    assert_eq!(outcome.remote_pending, CANONICAL_DOCUMENT_COUNT);
    assert_eq!(
        sink.maintenance()
            .expect("factory-selected maintenance delivers the spool"),
        0
    );
    let OpenOtDocumentSink::InfluxDb3(influx) = &mut sink else {
        unreachable!("selected InfluxDB 3 variant changed")
    };
    assert_canonical_jsons(
        influx
            .remote_canonical_jsons_for_run(checkpoint.run_id)
            .expect("query factory-delivered canonical documents"),
    );
    drop(sink);
    let _ = std::fs::remove_dir_all(root);
}
