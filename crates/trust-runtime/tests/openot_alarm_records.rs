//! Specification 40, §5.2: the runtime publishes each alarm transition to the OpenOT ring as a
//! condition record whose identity is the alarm's instance path.

#![cfg(unix)]

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use open_ot_carriage::concurrent::ConcurrentRawConsumer;
use open_ot_carriage::registry::{
    EVENT_CONDITION_ACKNOWLEDGED, EVENT_CONDITION_ACTIVE, EVENT_CONDITION_CLEARED,
    EVENT_CONDITION_COMMENTED, EVENT_CONDITION_SHELVED, KEY_ACK_BY, KEY_COMMENT, KEY_CONDITION_ID,
    KEY_CORRELATION_ID, KEY_SEVERITY, KEY_SHELVE_SECS, KEY_SOURCE_PATH, TY_STRING, TY_UDINT,
    TY_UINT,
};
use open_ot_carriage::wire::{Record, FLAG_PARTIAL_PAYLOAD};
use open_ot_shm::SharedConcurrentStore;
use serde_json::json;
use trust_runtime::config::{
    OpenOtTelemetryConfig, OpenOtTelemetryFenceMode, OpenOtTelemetrySource,
};
use trust_runtime::harness::TestHarness;
use trust_runtime::hmi::alarms::{read_alarms, EventKind, OperatorAction, RuntimeAlarms};
use trust_runtime::hmi::commands::write::{write_point, WriteAction};
use trust_runtime::hmi::member_paths::MemberPath;
use trust_runtime::scheduler::{ResourceRunner, StdClock};
use trust_runtime::CompiledRuntime;

const ALARM: &str = include_str!("../../../libraries/alarm/src/alarm.st");
const HMI: &str = include_str!("../../../libraries/hmi/src/hmi.st");

const PROGRAM: &str = r#"
USING Alarm;

PROGRAM Tank
VAR
    Trip : BOOL;
    Note : STRING := 'x';
    Overflow : DigitalAlarm;
END_VAR
    Overflow(Condition := Trip, Message := 'Tank overflow', Priority := AlarmPriority#High);
END_PROGRAM
"#;

/// The alarm service's OpenOT source (specification 40, §5.2).
const ALARM_SOURCE: u32 = 0xF000;

fn ring_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("trust-runtime-openot-alarms-{name}-{stamp}.shm"))
}

fn telemetry(path: PathBuf) -> OpenOtTelemetryConfig {
    OpenOtTelemetryConfig {
        enabled: true,
        path,
        capacity: 1 << 16,
        fence_mode: OpenOtTelemetryFenceMode::Fenced,
        allow_unfenced_for_proof: false,
        source: OpenOtTelemetrySource::Heartbeat,
        producer_instance: None,
        producer_instances: Vec::new(),
        persistence: Default::default(),
    }
}

/// A runtime with the program, publishing to a fresh ring, and its alarm service.
struct Plant {
    runtime: CompiledRuntime,
    alarms: RuntimeAlarms,
    path: PathBuf,
}

impl Plant {
    fn new(name: &str, program: &str) -> Self {
        let path = ring_path(name);
        let mut runtime = TestHarness::from_sources(&[ALARM, program])
            .expect("compiles")
            .into_runtime();
        runtime
            .configure_openot_telemetry(&telemetry(path.clone()), None)
            .expect("telemetry");
        let mut plant = Self {
            runtime,
            alarms: RuntimeAlarms::default(),
            path,
        };
        plant.scan();
        plant
    }

    /// One scan and one read of the alarms, as the call of `Hmi.Display` does; the read's
    /// records are published.
    fn scan(&mut self) {
        self.runtime.execute_cycle().expect("a scan");
        let records = self
            .alarms
            .apply(self.runtime.with_program_view(read_alarms));
        self.runtime.publish_alarm_records(&records);
    }

    fn write(&mut self, path: &str, value: serde_json::Value) {
        let path = MemberPath::parse(path).expect("a path");
        self.runtime
            .with_program(|program| write_point(program, &path, &WriteAction::Set(value)))
            .expect("written");
    }

    fn operator(&mut self, kind: EventKind, detail: Option<&str>, seconds: Option<u32>) {
        let records = self.alarms.operator_action(OperatorAction {
            path: "Tank.Overflow".to_string(),
            kind,
            user: "anna".to_string(),
            detail: detail.map(str::to_string),
            seconds,
        });
        self.runtime.publish_alarm_records(&records);
    }

    /// The alarm service's records in the ring, in order.
    fn records(&self) -> Vec<Record> {
        let store = SharedConcurrentStore::open_existing(&self.path).expect("the ring");
        let mut consumer = ConcurrentRawConsumer::with_store(store);
        let batch = consumer.poll().expect("records");
        batch
            .records
            .into_iter()
            .map(|read| read.record)
            .filter(|record| record.source_id == ALARM_SOURCE)
            .collect()
    }
}

impl Drop for Plant {
    fn drop(&mut self) {
        drop(std::fs::remove_file(&self.path));
    }
}

fn slot(record: &Record, key: u16) -> Option<(u8, Vec<u8>)> {
    record
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .map(|slot| (slot.ty, slot.payload.clone()))
}

fn text(record: &Record, key: u16) -> Option<String> {
    slot(record, key).map(|(ty, bytes)| {
        assert_eq!(ty, TY_STRING, "key {key:#06x} is a string");
        String::from_utf8(bytes).expect("UTF-8")
    })
}

fn number(record: &Record, key: u16) -> Option<u64> {
    slot(record, key).map(|(ty, bytes)| match ty {
        TY_UDINT => u64::from(u32::from_le_bytes(bytes.try_into().expect("four bytes"))),
        TY_UINT => u64::from(u16::from_le_bytes(bytes.try_into().expect("two bytes"))),
        other => panic!("key {key:#06x} has type {other}"),
    })
}

/// The 32-bit FNV-1a hash of the lower-case path (specification 40, §5.2).
fn condition_id(path: &str) -> u64 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in path.to_ascii_lowercase().bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    u64::from(hash.max(1))
}

#[test]
fn a_raised_and_cleared_alarm_is_published_as_condition_records_of_its_instance_path() {
    let mut plant = Plant::new("raise", PROGRAM);
    plant.write("Tank.Trip", json!(true));
    plant.scan();
    plant.write("Tank.Trip", json!(false));
    plant.scan();
    let records = plant.records();
    let kinds: Vec<u32> = records.iter().map(|record| record.event_type_id).collect();
    assert_eq!(
        kinds,
        vec![EVENT_CONDITION_ACTIVE, EVENT_CONDITION_CLEARED],
        "{records:?}"
    );
    let active = &records[0];
    assert_eq!(
        text(active, KEY_SOURCE_PATH).as_deref(),
        Some("Tank.Overflow")
    );
    assert_eq!(
        number(active, KEY_CONDITION_ID),
        Some(condition_id("Tank.Overflow"))
    );
    assert_eq!(
        number(active, KEY_CORRELATION_ID),
        Some(1),
        "the raise count of this activation"
    );
    assert_eq!(number(active, KEY_SEVERITY), Some(900), "High");
    assert_eq!(
        number(&records[1], KEY_CORRELATION_ID),
        Some(1),
        "the clearing of the same activation"
    );
    assert_eq!(
        records.iter().map(|record| record.seq).collect::<Vec<_>>(),
        vec![1, 2]
    );
    let now_ns = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos(),
    )
    .expect("ns");
    assert!(
        now_ns - active.source_time < 60_000_000_000,
        "the time in ns since the epoch: {}",
        active.source_time
    );
}

#[test]
fn an_operators_actions_carry_the_user_the_seconds_and_the_comment() {
    let mut plant = Plant::new("operator", PROGRAM);
    plant.write("Tank.Trip", json!(true));
    plant.scan();
    plant.operator(EventKind::Acknowledged, None, None);
    plant.write("Tank.Overflow.Request.Acknowledge", json!(true));
    plant.scan();
    plant.operator(EventKind::Shelved, Some("10 min"), Some(600));
    plant.write("Tank.Overflow.Request.Shelve", json!(600));
    plant.scan();
    plant.operator(EventKind::Comment, Some("Level gauge checked"), None);
    let records = plant.records();
    let kinds: Vec<u32> = records.iter().map(|record| record.event_type_id).collect();
    assert_eq!(
        kinds,
        vec![
            EVENT_CONDITION_ACTIVE,
            EVENT_CONDITION_ACKNOWLEDGED,
            EVENT_CONDITION_SHELVED,
            EVENT_CONDITION_COMMENTED
        ],
        "{records:?}"
    );
    assert_eq!(text(&records[1], KEY_ACK_BY).as_deref(), Some("anna"));
    assert_eq!(number(&records[2], KEY_SHELVE_SECS), Some(600));
    assert_eq!(text(&records[2], KEY_ACK_BY).as_deref(), Some("anna"));
    assert_eq!(
        text(&records[3], KEY_COMMENT).as_deref(),
        Some("Level gauge checked")
    );
    assert!(records
        .iter()
        .all(|record| text(record, KEY_SOURCE_PATH).as_deref() == Some("Tank.Overflow")));
}

#[test]
fn a_text_longer_than_a_slot_is_cut_and_the_record_flagged_partial() {
    let mut plant = Plant::new("partial", PROGRAM);
    plant.write("Tank.Trip", json!(true));
    plant.scan();
    let long = "ä".repeat(200);
    plant.operator(EventKind::Comment, Some(&long), None);
    let records = plant.records();
    let comment = records.last().expect("the comment");
    let cut = text(comment, KEY_COMMENT).expect("the comment text");
    assert!(
        cut.len() <= 255 && long.starts_with(&cut) && cut.len() >= 254,
        "{} bytes",
        cut.len()
    );
    assert_ne!(comment.flags & FLAG_PARTIAL_PAYLOAD, 0);
}

#[test]
fn without_openot_nothing_is_published_and_nothing_fails() {
    let mut runtime = TestHarness::from_sources(&[ALARM, PROGRAM])
        .expect("compiles")
        .into_runtime();
    let mut alarms = RuntimeAlarms::default();
    runtime.execute_cycle().expect("a scan");
    let records = alarms.apply(runtime.with_program_view(read_alarms));
    runtime.publish_alarm_records(&records);
}

/// Specification 40, §5 item 2 and 39, §18 Phase 5: the alarms are read at the program's call of
/// `Hmi.Display`, so the records reach the history with no display open.
#[test]
fn the_runner_publishes_alarm_records_with_no_display_attached() {
    let program = r#"
USING Alarm;

PROGRAM Tank
VAR
    Count : INT;
    Overflow : DigitalAlarm;
    Screens : Hmi.Display;
END_VAR
    Count := Count + 1;
    Overflow(Condition := Count > 3, Message := 'Tank overflow', Priority := AlarmPriority#High);
    Screens();
END_PROGRAM
"#;
    let path = ring_path("runner");
    let mut runtime = TestHarness::from_sources(&[ALARM, HMI, program])
        .expect("compiles")
        .into_runtime();
    runtime
        .configure_openot_telemetry(&telemetry(path.clone()), None)
        .expect("telemetry");
    let mut handle = ResourceRunner::new(
        runtime,
        StdClock::new(),
        trust_runtime::value::Duration::from_millis(20),
    )
    .spawn("openot-alarm-records")
    .expect("the runner starts");
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut found = None;
    while Instant::now() < deadline && found.is_none() {
        std::thread::sleep(Duration::from_millis(100));
        let store = SharedConcurrentStore::open_existing(&path).expect("the ring");
        let mut consumer = ConcurrentRawConsumer::with_store(store);
        found = consumer
            .poll()
            .expect("records")
            .records
            .into_iter()
            .map(|read| read.record)
            .find(|record| {
                record.source_id == ALARM_SOURCE && record.event_type_id == EVENT_CONDITION_ACTIVE
            });
    }
    handle.stop();
    let _ = handle.join();
    drop(std::fs::remove_file(&path));
    let active = found.expect("the runner published the raise without any display asking");
    assert_eq!(
        text(&active, KEY_SOURCE_PATH).as_deref(),
        Some("Tank.Overflow")
    );
}
