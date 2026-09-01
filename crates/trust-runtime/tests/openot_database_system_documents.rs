#![cfg(all(unix, feature = "openot-real-database-tests"))]

use open_ot_carriage::loss::records_dropped_record;
use open_ot_carriage::registry::{
    EVENT_BUFFER_CLEARED, EVENT_DEFINITION_CHANGED, EVENT_HEARTBEAT, EVENT_LOGGER_STARTED,
    EVENT_LOGGER_STOPPED, EVENT_MESSAGE, EVENT_SOURCE_HIGH_WATER, EVENT_SOURCE_REGISTERED,
    EVENT_TIME_SYNC_CHANGED, KEY_CLOCK_QUALITY, KEY_COLD_START, KEY_DEF_HASH_NEW, KEY_DEF_HASH_OLD,
    KEY_EPOCH_ID, KEY_MESSAGE_TEMPLATE_ID, KEY_REGISTERED_SOURCE_ID, KEY_SOURCE_HIGH_WATER,
    SYSTEM_SOURCE_ID, TY_BOOL, TY_BYTES, TY_UDINT, TY_UINT, TY_ULINT,
};
use open_ot_carriage::ring::LossRange;
use open_ot_carriage::wire::{Record, Slot};
use open_ot_document::{to_json, Document};
use open_ot_shm::SharedRecordPublisher;
use trust_runtime::openot_persistence::{
    DocumentSink, InfluxDb3DocumentSink, MySqlDocumentSink, OpenOtDocumentSource,
    OpenOtPersistenceConsumer, PostgreSqlDocumentSink, SharedMemoryOpenOtSource,
    SqlServerDocumentSink, SqliteDocumentSink, TimescaleDbDocumentSink,
};

const RECEIVE_TIME_NS: u64 = 1_781_000_000_000_000_000;

fn private_root(label: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-openot-system-documents-{label}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create private fixture root");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("secure fixture root");
    root
}

fn system_record(seq: u64, event_type: u32, slots: Vec<Slot>) -> Record {
    let mut record = Record::new(RECEIVE_TIME_NS + seq, 1, seq, SYSTEM_SOURCE_ID, event_type);
    record.slots = slots;
    record
}

fn resolved_system_loss_placeholder_batch(
    ring_path: &std::path::Path,
) -> trust_runtime::openot_persistence::PersistenceBatch {
    let mut publisher = SharedRecordPublisher::create(ring_path, 65_536).expect("publisher");
    publisher
        .append_record(&system_record(0, EVENT_HEARTBEAT, Vec::new()))
        .expect("heartbeat");
    publisher
        .append_record(&system_record(
            1,
            EVENT_LOGGER_STARTED,
            vec![Slot::new(KEY_COLD_START, TY_BOOL, [1])],
        ))
        .expect("logger started");
    publisher
        .append_record(&system_record(2, EVENT_LOGGER_STOPPED, Vec::new()))
        .expect("logger stopped");
    publisher
        .append_record(&system_record(3, EVENT_BUFFER_CLEARED, Vec::new()))
        .expect("buffer cleared");
    publisher
        .append_record(&records_dropped_record(
            4,
            &LossRange {
                run_id: 1,
                source_id: 12,
                first_seq: 5,
                last_seq: 7,
            },
        ))
        .expect("records dropped");
    publisher
        .append_record(&system_record(
            5,
            EVENT_SOURCE_REGISTERED,
            vec![Slot::new(
                KEY_REGISTERED_SOURCE_ID,
                TY_UDINT,
                11u32.to_le_bytes(),
            )],
        ))
        .expect("source registered");
    publisher
        .append_record(&system_record(
            6,
            EVENT_DEFINITION_CHANGED,
            vec![
                Slot::new(KEY_DEF_HASH_OLD, TY_BYTES, [0u8; 8]),
                Slot::new(KEY_DEF_HASH_NEW, TY_BYTES, [1u8; 8]),
                Slot::new(KEY_EPOCH_ID, TY_ULINT, 1u64.to_le_bytes()),
            ],
        ))
        .expect("definition changed");
    publisher
        .append_record(&system_record(
            7,
            EVENT_TIME_SYNC_CHANGED,
            vec![Slot::new(KEY_CLOCK_QUALITY, TY_UINT, 1u16.to_le_bytes())],
        ))
        .expect("time synchronization changed");

    publisher
        .append_record(&Record::new(RECEIVE_TIME_NS + 8, 1, 0, 11, EVENT_HEARTBEAT))
        .expect("source sequence zero");
    publisher
        .append_record(&Record::new(RECEIVE_TIME_NS + 9, 1, 2, 11, EVENT_HEARTBEAT))
        .expect("source sequence gap");
    let mut high_water = Record::new(RECEIVE_TIME_NS + 10, 1, 3, 11, EVENT_SOURCE_HIGH_WATER);
    high_water.slots.push(Slot::new(
        KEY_SOURCE_HIGH_WATER,
        TY_ULINT,
        3u64.to_le_bytes(),
    ));
    publisher
        .append_record(&high_water)
        .expect("source high water");

    let mut unresolved = Record::new(RECEIVE_TIME_NS + 11, 1, 0, 999, EVENT_MESSAGE);
    unresolved.slots.push(Slot::new(
        KEY_MESSAGE_TEMPLATE_ID,
        TY_UINT,
        1u16.to_le_bytes(),
    ));
    publisher
        .append_record(&unresolved)
        .expect("unresolved source record");

    let mut source = SharedMemoryOpenOtSource::open(ring_path).expect("shared-memory source");
    let poll = source.poll().expect("poll runtime ring");
    let mut consumer =
        OpenOtPersistenceConsumer::new(open_ot_definition::sample_definition(), None)
            .expect("OpenOT resolver");
    consumer
        .prepare_batch(&poll.batch, &poll.snapshot, RECEIVE_TIME_NS, 0)
        .expect("resolve system/loss/placeholder batch")
}

fn expected_json(batch: &trust_runtime::openot_persistence::PersistenceBatch) -> Vec<String> {
    let mut expected = batch
        .documents
        .iter()
        .map(|document| to_json(document).expect("canonical JSON"))
        .collect::<Vec<_>>();
    expected.sort();
    expected
}

fn assert_fixture_contract(batch: &trust_runtime::openot_persistence::PersistenceBatch) {
    let event_names = batch
        .documents
        .iter()
        .filter_map(|document| match document {
            Document::Event(event) => Some(event.event_name.as_str()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    for expected in [
        "Heartbeat",
        "LoggerStarted",
        "LoggerStopped",
        "BufferCleared",
        "RecordsDropped",
        "SourceRegistered",
        "DefinitionChanged",
        "TimeSyncChanged",
        "SourceHighWater",
    ] {
        assert!(
            event_names.contains(expected),
            "missing system event {expected}"
        );
    }
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../examples/openot_database/workload/openot-coverage-manifest.json"
    ))
    .expect("coverage manifest");
    let manifested_system_events = manifest["runtimeSystemEvents"]
        .as_array()
        .expect("manifest runtime system events")
        .iter()
        .map(|event| event.as_str().expect("manifest system event name"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        event_names, manifested_system_events,
        "runtime system-event manifest drift"
    );
    let loss_bases = batch
        .documents
        .iter()
        .filter_map(|document| match document {
            Document::Loss(loss) => Some(format!("{:?}", loss.basis).to_lowercase()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        loss_bases,
        ["authoritative".to_string(), "inferred".to_string()]
            .into_iter()
            .collect()
    );
    let placeholder = batch
        .documents
        .iter()
        .find_map(|document| match document {
            Document::Placeholder(placeholder) => Some(placeholder),
            _ => None,
        })
        .expect("resolver placeholder");
    assert!(!placeholder.raw_slots.is_empty(), "placeholder raw slots");
}

fn commit_and_compare<D: DocumentSink>(
    sink: &mut D,
    batch: &trust_runtime::openot_persistence::PersistenceBatch,
    actual: impl FnOnce(&mut D) -> Vec<String>,
    expected: &[String],
    product: &str,
) {
    let outcome = sink
        .commit(batch)
        .unwrap_or_else(|error| panic!("commit {product}: {error:?}"));
    assert_eq!(
        outcome.inserted,
        batch.documents.len(),
        "{product} inserted"
    );
    while sink
        .maintenance()
        .unwrap_or_else(|error| panic!("maintain {product}: {error:?}"))
        > 0
    {}
    let mut actual = actual(sink);
    actual.sort();
    assert_eq!(actual, expected, "{product} canonical document drift");
}

#[test]
fn runtime_system_loss_and_placeholder_documents_round_trip_through_every_real_product() {
    let root = private_root("matrix");
    let ring_path = root.join("openot.shm");
    let batch = resolved_system_loss_placeholder_batch(&ring_path);
    assert_fixture_contract(&batch);
    let expected = expected_json(&batch);
    let pid = std::process::id();

    let sqlite_path = root.join("sqlite/trust-logging.sqlite3");
    let mut sqlite = SqliteDocumentSink::open(&sqlite_path).expect("open SQLite");
    commit_and_compare(
        &mut sqlite,
        &batch,
        |_| {
            let connection = rusqlite::Connection::open(&sqlite_path).expect("inspect SQLite");
            let mut statement = connection
                .prepare("SELECT canonical_json FROM logging_records ORDER BY identity_key")
                .expect("prepare SQLite query");
            statement
                .query_map([], |row| row.get(0))
                .expect("query SQLite")
                .collect::<Result<Vec<String>, _>>()
                .expect("collect SQLite JSON")
        },
        &expected,
        "SQLite",
    );

    let mut postgresql = PostgreSqlDocumentSink::open(
        &std::env::var("TRUST_TEST_OPENOT_POSTGRES_URL").expect("PostgreSQL URL"),
        &format!("logging_system_{pid}"),
        std::path::Path::new(
            &std::env::var("TRUST_TEST_OPENOT_POSTGRES_CA").expect("PostgreSQL CA"),
        ),
    )
    .expect("open PostgreSQL");
    commit_and_compare(
        &mut postgresql,
        &batch,
        |sink| sink.canonical_jsons().expect("query PostgreSQL"),
        &expected,
        "PostgreSQL",
    );

    let mut timescale = TimescaleDbDocumentSink::open(
        &std::env::var("TRUST_TEST_OPENOT_TIMESCALE_URL").expect("TimescaleDB URL"),
        &format!("logging_system_{pid}"),
        std::path::Path::new(
            &std::env::var("TRUST_TEST_OPENOT_TIMESCALE_CA").expect("TimescaleDB CA"),
        ),
    )
    .expect("open TimescaleDB");
    commit_and_compare(
        &mut timescale,
        &batch,
        |sink| sink.canonical_jsons().expect("query TimescaleDB"),
        &expected,
        "TimescaleDB",
    );

    for (product, url_env, ca_env) in [
        (
            "MySQL",
            "TRUST_TEST_OPENOT_MYSQL_URL",
            "TRUST_TEST_OPENOT_MYSQL_CA",
        ),
        (
            "MariaDB",
            "TRUST_TEST_OPENOT_MARIADB_URL",
            "TRUST_TEST_OPENOT_MARIADB_CA",
        ),
    ] {
        let mut sink = MySqlDocumentSink::open(
            &std::env::var(url_env).unwrap_or_else(|_| panic!("{product} URL")),
            "trust_logging",
            std::path::Path::new(&std::env::var(ca_env).unwrap_or_else(|_| panic!("{product} CA"))),
        )
        .unwrap_or_else(|error| panic!("open {product}: {error:?}"));
        sink.reset_test_state()
            .unwrap_or_else(|error| panic!("reset {product}: {error:?}"));
        commit_and_compare(
            &mut sink,
            &batch,
            |sink| {
                sink.canonical_jsons()
                    .unwrap_or_else(|error| panic!("query {product}: {error:?}"))
            },
            &expected,
            product,
        );
    }

    let mut sqlserver = SqlServerDocumentSink::open(
        &std::env::var("TRUST_TEST_OPENOT_SQLSERVER_URL").expect("SQL Server URL"),
        &format!("logging_system_{pid}"),
        std::path::Path::new(
            &std::env::var("TRUST_TEST_OPENOT_SQLSERVER_CA").expect("SQL Server CA"),
        ),
    )
    .expect("open SQL Server");
    commit_and_compare(
        &mut sqlserver,
        &batch,
        |sink| sink.canonical_jsons().expect("query SQL Server"),
        &expected,
        "SQL Server",
    );

    let spool = root.join("influx/spool.sqlite3");
    let mut influx = InfluxDb3DocumentSink::open(
        &std::env::var("TRUST_TEST_OPENOT_INFLUX_HOST").expect("InfluxDB host"),
        &std::env::var("TRUST_TEST_OPENOT_INFLUX_TOKEN").expect("InfluxDB token"),
        "trust_logging",
        &spool,
        std::path::Path::new(&std::env::var("TRUST_TEST_OPENOT_INFLUX_CA").expect("InfluxDB CA")),
    )
    .expect("open InfluxDB 3");
    commit_and_compare(
        &mut influx,
        &batch,
        |sink| {
            sink.remote_canonical_jsons_at(RECEIVE_TIME_NS)
                .expect("query InfluxDB 3")
        },
        &expected,
        "InfluxDB 3",
    );

    std::fs::remove_dir_all(root).ok();
}
