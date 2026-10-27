//! Fault records (specification 34 MEM-05, STATE-04): the fixed-size `FaultRecord` stays `Copy`
//! and at most 56 bytes, and its logical time keeps the millisecond and does not wrap within a
//! century.
use trust_runtime_core::error::{RuntimeError, StableErrorCode};
use trust_runtime_core::fault::{FaultDetail, FaultRecord, FaultSite};
use trust_runtime_core::value::Duration;

/// The fault record is `Copy` and stays within its 56-byte bound on this target.
#[test]
fn the_fault_record_is_copy_and_at_most_56_bytes() {
    let record = FaultRecord {
        code: StableErrorCode::RuntimeIndexOutOfBounds,
        detail: FaultDetail::Index {
            index: 11,
            lower: 0,
            upper: 10,
        },
        site: FaultSite { pou: 7, pc: 9 },
        task: 2,
        logical_seconds: 30,
        logical_millis: 1,
    };
    let copied = record;
    assert_eq!(copied, record, "the record is copied, not moved");
    assert!(
        core::mem::size_of::<FaultRecord>() <= 56,
        "FaultRecord is {} bytes, above the 56-byte bound",
        core::mem::size_of::<FaultRecord>()
    );
}

/// Spec 34 MEM-05: the record's logical time is whole seconds beside the millisecond within
/// the second, at the millisecond precision the host's fault log reports. It would catch a
/// lost millisecond (a coarser unit or a wrong remainder) and a second that wraps before the
/// 32-bit field is full.
#[test]
fn the_time_field_keeps_the_millisecond_and_does_not_wrap() {
    let error = RuntimeError::DivisionByZero;
    let site = FaultSite { pou: 1, pc: 2 };
    let at_1_234_567_ms = FaultRecord::latch(&error, site, 0, Duration::from_millis(1_234_567));
    assert_eq!(
        (
            at_1_234_567_ms.logical_seconds,
            at_1_234_567_ms.logical_millis
        ),
        (1_234, 567),
        "1,234,567 ms is 1,234 s and 567 ms"
    );

    // u32::MAX seconds is 136 years of uptime; the last second and its millisecond read back
    // exactly.
    let at_the_last_second = FaultRecord::latch(
        &error,
        site,
        0,
        Duration::from_millis(i64::from(u32::MAX) * 1_000 + 999),
    );
    assert_eq!(
        (
            at_the_last_second.logical_seconds,
            at_the_last_second.logical_millis
        ),
        (u32::MAX, 999),
        "the last whole second and its millisecond read back exactly"
    );
}

/// Spec 34 MEM-05: the record's time stays distinct for a century of uptime in its 32-bit
/// second field, where a 32-bit millisecond count wraps after 49.7 days. It would catch a
/// record whose time field wraps within a century, or one that stores the raw millisecond
/// count beside its seconds.
#[test]
fn the_time_field_does_not_wrap_within_a_century() {
    // 100 years of 365 days: 3,153,600,000 seconds.
    const SECONDS_IN_A_CENTURY: u32 = 3_153_600_000;
    // The old 32-bit millisecond field holds 49 days; the record's 32-bit second field holds 136
    // years, so a century of uptime stays distinct in it.
    assert_eq!(
        u32::MAX / 86_400_000,
        49,
        "a 32-bit millisecond field wraps after 49 days"
    );
    assert_eq!(
        u32::MAX / 31_536_000,
        136,
        "a 32-bit second field lasts 136 years"
    );

    let error = RuntimeError::DivisionByZero;
    let site = FaultSite { pou: 1, pc: 2 };
    let at_a_century = FaultRecord::latch(
        &error,
        site,
        0,
        Duration::from_millis(i64::from(SECONDS_IN_A_CENTURY) * 1_000 + 36),
    );
    assert_eq!(
        (at_a_century.logical_seconds, at_a_century.logical_millis),
        (SECONDS_IN_A_CENTURY, 36),
        "a century of uptime and 36 ms is a distinct time in the record"
    );
    let one_second_later = FaultRecord::latch(
        &error,
        site,
        0,
        Duration::from_millis((i64::from(SECONDS_IN_A_CENTURY) + 1) * 1_000 + 36),
    );
    assert_eq!(
        one_second_later.logical_seconds,
        SECONDS_IN_A_CENTURY + 1,
        "the next second stays distinct, so the field has not wrapped"
    );
}
