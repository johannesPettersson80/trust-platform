//! A `variables.read` reply as a snapshot, and its values as Rust values (specification 11 §6.8.1,
//! "Reading variables"; specification 48 RU-62). Every expected value is worked out by hand.

use std::time::Duration;

use serde_json::json;

use super::{Quality, Snapshot, Variable};
use crate::ServiceError;

/// The reply of one completed scan, number 7 at 70 ms, of a running resource.
fn snapshot() -> Snapshot {
    Snapshot::of_reply(&json!({
        "scan": 7,
        "time_ns": 70_000_000,
        "state": "running",
        "quality": "good",
        "values": [
            {"path": "P1.level", "type": "REAL", "value": 1.5},
            {"path": "P1.count", "type": "INT", "value": -3},
            {"path": "P1.on", "type": "BOOL", "value": true},
            {"path": "P1.name", "type": "STRING[20]", "value": "pump"},
            {"path": "P1.delay", "type": "TIME", "value": 1_500_000_000_u64},
            {"path": "P1.big", "type": "LREAL", "value": 1e300},
            {"path": "P1.bad", "type": "REAL", "value": "NaN"},
        ],
    }))
    .expect("a variables.read reply")
}

#[test]
fn a_reply_reads_as_its_scan_and_its_values() {
    let read = snapshot();
    assert_eq!(
        (read.scan, read.time_ns, read.state.as_str()),
        (7, 70_000_000, "running")
    );
    assert_eq!(read.quality, Quality::Good);
    assert_eq!(
        read.values[0],
        Variable {
            path: "P1.level".into(),
            type_name: "REAL".into(),
            value: json!(1.5),
        }
    );
}

#[test]
fn each_value_reads_as_the_rust_type_of_its_declared_type() {
    // A REAL as f32 and f64, an INT as i16 and i64, a BOOL, a STRING, a TIME of 1.5 s as a
    // Duration, an LREAL as f64, and NaN as an f64 that is NaN. Catches a TIME read in the wrong
    // unit and a REAL that only an f64 could read.
    let read = snapshot();
    assert_eq!(read.get::<f32>("P1.level"), Ok(1.5));
    assert_eq!(read.get::<f64>("P1.level"), Ok(1.5));
    assert_eq!(read.get::<i16>("P1.count"), Ok(-3));
    assert_eq!(read.get::<i64>("P1.count"), Ok(-3));
    assert_eq!(read.get::<bool>("P1.on"), Ok(true));
    assert_eq!(read.get::<String>("P1.name"), Ok("pump".to_string()));
    assert_eq!(
        read.get::<Duration>("P1.delay"),
        Ok(Duration::from_millis(1500))
    );
    assert_eq!(read.get::<f64>("P1.big"), Ok(1e300));
    assert!(read.get::<f64>("P1.bad").expect("NaN").is_nan());
}

#[test]
fn a_value_of_another_type_or_range_is_refused() {
    // -3 is no u8; an LREAL is no f32, which cannot hold every LREAL; a text is no bool; a path
    // the read did not name. Catches a value wrapped into a narrower type.
    let read = snapshot();
    assert_eq!(
        read.get::<u8>("P1.count"),
        Err(ServiceError::Type {
            path: "P1.count".into(),
            type_name: "INT".into(),
            wanted: "u8",
        })
    );
    assert_eq!(
        read.get::<f32>("P1.big"),
        Err(ServiceError::Type {
            path: "P1.big".into(),
            type_name: "LREAL".into(),
            wanted: "f32",
        })
    );
    assert!(matches!(
        read.get::<bool>("P1.name"),
        Err(ServiceError::Type { .. })
    ));
    assert_eq!(
        read.get::<bool>("P1.other"),
        Err(ServiceError::NotRead("P1.other".into()))
    );
}

#[test]
fn a_reply_without_its_fields_is_no_snapshot() {
    // Catches a reply of another request taken for a read.
    assert_eq!(
        Snapshot::of_reply(&json!({"service": "recipe", "count": 1})),
        None
    );
    assert_eq!(
        Snapshot::of_reply(
            &json!({"scan": 1, "time_ns": 0, "state": "running", "quality": "fine", "values": []})
        ),
        None
    );
}
