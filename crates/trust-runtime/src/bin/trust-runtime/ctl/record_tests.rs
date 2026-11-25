//! `ctl record`'s own rules (specification 11 §6.8.1, "Cycle traces"): its goal and its default
//! file name. The checks of a trace's state and parts are the client's (`trust_runtime_client::trace`,
//! tested there). The expected values were worked out with Python's `datetime`, not with this crate.

use std::path::PathBuf;

use super::{default_file, duration_ns, goal, Goal};

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
        default_file("trace", 1_795_483_800),
        PathBuf::from("trace-20261124T013000Z.trusttrace")
    );
    assert_eq!(
        default_file("trace", 1_835_481_599),
        PathBuf::from("trace-20280229T235959Z.trusttrace")
    );
    // `ctl fault-trace` and the runtime's own copy of a fault's trace take `fault-` in its place.
    assert_eq!(
        default_file("fault", 1_795_483_800),
        PathBuf::from("fault-20261124T013000Z.trusttrace")
    );
}
