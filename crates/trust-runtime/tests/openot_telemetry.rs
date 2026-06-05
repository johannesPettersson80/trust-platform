#![cfg(unix)]

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use open_ot_carriage::concurrent::ConcurrentRawConsumer;
use open_ot_carriage::registry::{EVENT_HEARTBEAT, SYSTEM_SOURCE_ID};
use open_ot_shm::SharedConcurrentStore;
use trust_runtime::config::{OpenOtTelemetryConfig, OpenOtTelemetryFenceMode};
use trust_runtime::Runtime;

fn temp_shm_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("trust-runtime-openot-{name}-{stamp}.shm"))
}

fn telemetry_config(path: PathBuf, capacity: usize) -> OpenOtTelemetryConfig {
    OpenOtTelemetryConfig {
        enabled: true,
        path,
        capacity,
        fence_mode: OpenOtTelemetryFenceMode::Fenced,
        allow_unfenced_for_proof: false,
    }
}

#[test]
fn openot_telemetry_publishes_crc_valid_heartbeats() {
    let path = temp_shm_path("heartbeat");
    let mut runtime = Runtime::new();
    runtime
        .configure_openot_telemetry(&telemetry_config(path.clone(), 4096), None)
        .expect("configure OpenOT telemetry");

    for _ in 0..3 {
        runtime.execute_cycle().expect("cycle publishes heartbeat");
    }

    let store = SharedConcurrentStore::open_existing(&path).expect("open telemetry shm");
    let mut consumer = ConcurrentRawConsumer::with_store(store);
    let batch = consumer.poll().expect("poll telemetry records");
    assert_eq!(batch.records.len(), 3);

    for (expected_seq, read) in batch.records.iter().enumerate() {
        let expected_seq = u64::try_from(expected_seq).expect("small test sequence");
        let record = &read.record;
        assert_eq!(record.event_type_id, EVENT_HEARTBEAT);
        assert_eq!(record.source_id, SYSTEM_SOURCE_ID);
        assert_eq!(record.run_id, 1);
        assert_eq!(record.seq, expected_seq);
        assert_eq!(record.source_time, expected_seq);
        assert!(record.slots.is_empty());
    }

    drop(std::fs::remove_file(path));
}

#[test]
fn openot_telemetry_publish_failure_is_fail_closed() {
    let path = temp_shm_path("too-small");
    let mut runtime = Runtime::new();
    runtime
        .configure_openot_telemetry(&telemetry_config(path.clone(), 1), None)
        .expect("configure tiny OpenOT telemetry ring");

    let err = runtime
        .execute_cycle()
        .expect_err("telemetry append failure must fail the cycle");
    assert!(err
        .to_string()
        .contains("OpenOT telemetry publish heartbeat"));

    drop(std::fs::remove_file(path));
}
