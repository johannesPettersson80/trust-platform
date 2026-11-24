//! `ctl record`'s own rules (specification 11 §6.8.1, "Cycle traces"): its goal, its default file
//! name, and the checks of a trace's state and parts. The expected values were worked out with
//! Python's `datetime`, `zlib` and `base64`, not with this crate.

use std::path::PathBuf;

use serde_json::json;

use super::{default_file, duration_ns, ended, goal, part, Ended, Goal};

#[test]
fn a_duration_is_an_iec_duration_with_or_without_its_prefix() {
    // Specification 11: `<t>` is a duration as IEC 61131-3 writes one (Table 8), with or without
    // `T#`, of at least 1 ns. Catches a prefix demanded or doubled, a unit read wrong, and a zero,
    // negative or unitless duration that would record nothing or forever.
    assert_eq!(duration_ns("500ms").unwrap(), 500_000_000);
    assert_eq!(duration_ns("2s").unwrap(), 2_000_000_000);
    assert_eq!(duration_ns("1m30s").unwrap(), 90_000_000_000);
    assert_eq!(duration_ns("T#1h").unwrap(), 3_600_000_000_000);
    assert_eq!(duration_ns(" TIME#2s ").unwrap(), 2_000_000_000);
    for refused in ["0ms", "T#-2s", "5", "two seconds", "INT#5", ""] {
        assert!(duration_ns(refused).is_err(), "{refused:?} is no duration");
    }
}

#[test]
fn a_goal_is_cycles_or_a_duration() {
    // Catches a goal of no cycles, which `trace.record` refuses, and a goal of both kinds.
    assert_eq!(goal(Some(500), None).unwrap(), Goal::Cycles(500));
    assert_eq!(
        goal(None, Some("2s")).unwrap(),
        Goal::DurationNs(2_000_000_000)
    );
    assert!(goal(Some(0), None).is_err());
    assert!(goal(Some(5), Some("2s")).is_err());
    assert!(goal(None, None).is_err());
}

#[test]
fn the_default_file_names_the_utc_date_and_time_in_iso_8601_basic_form() {
    // Specification 11: `trace-<UTC date and time>.trusttrace`, as `trace-20261124T013000Z`.
    // 1795483800 is 2026-11-24T01:30:00Z and 1835481599 is 2028-02-29T23:59:59Z, a leap day
    // (Python's datetime). Catches local time, a 0-based month, and separators a Windows file
    // name cannot hold.
    assert_eq!(
        default_file(1_795_483_800),
        PathBuf::from("trace-20261124T013000Z.trusttrace")
    );
    assert_eq!(
        default_file(1_835_481_599),
        PathBuf::from("trace-20280229T235959Z.trusttrace")
    );
}

#[test]
fn a_state_is_recording_or_ended_with_all_its_figures() {
    // Specification 11: `trace.read` answers `recording` with its cycles, or `ended` with the
    // reason, cycles, bytes and SHA-256. Catches an ended trace taken without its figures, which
    // the file could not be checked against, and a state of no recording taken as recording, which
    // would ask forever.
    assert_eq!(
        ended(&json!({"state": "recording", "cycles": 3})).unwrap(),
        None
    );
    let sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    assert_eq!(
        ended(&json!({
            "state": "ended", "reason": "stopped", "cycles": 2, "bytes": 900, "sha256": sha256
        }))
        .unwrap(),
        Some(Ended {
            reason: "stopped".to_owned(),
            cycles: 2,
            bytes: 900,
            sha256: sha256.to_owned(),
        })
    );
    assert!(
        ended(&json!({"state": "ended", "reason": "complete", "cycles": 2, "bytes": 900})).is_err()
    );
    assert!(ended(&json!({"state": "none"})).is_err());
}

#[test]
fn a_part_is_taken_only_whole_and_at_its_offset() {
    // Specification 34 PORT-05: a part carries its offset, its bytes in base64, their CRC-32 and the
    // next offset. The bytes 01 to 05 are "AQIDBAU=" with the CRC-32 470b99f4 (Python's base64 and
    // zlib). Catches a damaged part kept, a part from another offset spliced in, and an empty part,
    // on which the reading would never end.
    let good =
        json!({"offset": 8192, "data": "AQIDBAU=", "crc32": "470b99f4", "next_offset": 8197});
    assert_eq!(part(&good, 8192).unwrap(), vec![1, 2, 3, 4, 5]);
    assert!(part(&good, 4096).is_err());
    let damaged =
        json!({"offset": 8192, "data": "AQIDBAU=", "crc32": "470b99f5", "next_offset": 8197});
    assert!(part(&damaged, 8192).is_err());
    let next =
        json!({"offset": 8192, "data": "AQIDBAU=", "crc32": "470b99f4", "next_offset": 8196});
    assert!(part(&next, 8192).is_err());
    let empty = json!({"offset": 8192, "data": "", "crc32": "00000000", "next_offset": 8192});
    assert!(part(&empty, 8192).is_err());
    let bare = json!({"offset": 8192, "data": "AQIDBAU="});
    assert!(part(&bare, 8192).is_err());
}
