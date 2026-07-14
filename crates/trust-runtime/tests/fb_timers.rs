use trust_runtime::stdlib::fbs::{Tof, Ton, Tp};
use trust_runtime::value::Duration;

#[test]
fn ton_tof_tp() {
    let mut ton = Ton::new();
    let mut tof = Tof::new();
    let mut tp = Tp::new();

    let pt = Duration::from_millis(10);
    let delta = Duration::from_millis(5);

    let out = ton.step(false, pt, delta);
    assert!(!out.q);
    assert_eq!(out.et, Duration::ZERO);

    let out = ton.step(true, pt, delta);
    assert!(!out.q);
    assert_eq!(out.et, Duration::from_millis(5));

    let out = ton.step(true, pt, delta);
    assert!(out.q);
    assert_eq!(out.et, Duration::from_millis(10));

    let out = ton.step(false, pt, delta);
    assert!(!out.q);
    assert_eq!(out.et, Duration::ZERO);

    let out = tof.step(true, pt, delta);
    assert!(out.q);
    let out = tof.step(false, pt, delta);
    assert!(out.q);
    assert_eq!(out.et, Duration::from_millis(5));
    let out = tof.step(false, pt, delta);
    assert!(!out.q);
    assert_eq!(out.et, Duration::from_millis(10));
    let out = tof.step(false, pt, delta);
    assert!(!out.q);
    assert_eq!(out.et, Duration::from_millis(10));
    let out = tof.step(true, pt, delta);
    assert!(out.q);
    assert_eq!(out.et, Duration::ZERO);

    let out = tp.step(false, pt, delta);
    assert!(!out.q);
    let out = tp.step(true, pt, delta);
    assert!(out.q);
    assert_eq!(out.et, Duration::from_millis(5));
    let out = tp.step(true, pt, delta);
    assert!(!out.q);
    assert_eq!(out.et, Duration::from_millis(10));
    let out = tp.step(true, pt, delta);
    assert!(!out.q);
    assert_eq!(out.et, Duration::from_millis(10));
    let out = tp.step(false, pt, delta);
    assert!(!out.q);
    assert_eq!(out.et, Duration::ZERO);
    let out = tp.step(true, pt, delta);
    assert!(out.q);
    assert_eq!(out.et, Duration::from_millis(5));
}

#[test]
fn ton_clamps_elapsed_time_at_duration_max() {
    let mut ton = Ton::new();
    let pt = Duration::from_nanos(i64::MAX);

    let out = ton.step(
        true,
        pt,
        Duration::from_nanos(i64::MAX - 1),
    );
    assert!(!out.q);
    assert_eq!(out.et, Duration::from_nanos(i64::MAX - 1));

    let out = ton.step(true, pt, Duration::from_nanos(2));
    assert!(out.q);
    assert_eq!(out.et, pt);
}

#[test]
fn tof_clamps_elapsed_time_at_duration_max() {
    let mut tof = Tof::new();
    let pt = Duration::from_nanos(i64::MAX);

    let out = tof.step(true, pt, Duration::ZERO);
    assert!(out.q);
    assert_eq!(out.et, Duration::ZERO);

    let out = tof.step(
        false,
        pt,
        Duration::from_nanos(i64::MAX - 1),
    );
    assert!(out.q);
    assert_eq!(out.et, Duration::from_nanos(i64::MAX - 1));

    let out = tof.step(false, pt, Duration::from_nanos(2));
    assert!(!out.q);
    assert_eq!(out.et, pt);

    let out = tof.step(false, pt, Duration::from_nanos(1));
    assert!(!out.q);
    assert_eq!(out.et, pt);
}

#[test]
fn tp_clamps_elapsed_time_at_duration_max() {
    let mut tp = Tp::new();
    let pt = Duration::from_nanos(i64::MAX);

    let out = tp.step(
        true,
        pt,
        Duration::from_nanos(i64::MAX - 1),
    );
    assert!(out.q);
    assert_eq!(out.et, Duration::from_nanos(i64::MAX - 1));

    let out = tp.step(true, pt, Duration::from_nanos(2));
    assert!(!out.q);
    assert_eq!(out.et, pt);
}
