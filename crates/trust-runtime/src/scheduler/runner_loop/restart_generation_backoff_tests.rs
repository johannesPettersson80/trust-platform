use super::*;

/// Spec 11 §6.6: the backoff base is clamped to the inclusive range from one millisecond through one second, so a zero or sub-millisecond cycle interval waits one millisecond and a long cycle interval waits one second.
#[test]
fn automatic_restart_backoff_base_is_clamped_to_one_millisecond_through_one_second() {
    assert_eq!(
        automatic_restart_backoff(Duration::ZERO, 1).as_nanos(),
        1_000_000
    );
    assert_eq!(
        automatic_restart_backoff(Duration::from_nanos(1), 1).as_nanos(),
        1_000_000
    );
    assert_eq!(
        automatic_restart_backoff(Duration::from_secs(60), 1).as_nanos(),
        1_000_000_000
    );
}

/// Spec 11 §6.6: later automatic attempts increase exponentially from the base, and the resulting delay never exceeds one second.
#[test]
fn automatic_restart_backoff_doubles_per_attempt_and_stops_at_one_second() {
    let delays: Vec<i64> = (1..=5)
        .map(|attempt| automatic_restart_backoff(Duration::from_millis(10), attempt).as_nanos())
        .collect();
    assert_eq!(
        delays,
        vec![10_000_000, 20_000_000, 40_000_000, 80_000_000, 160_000_000]
    );
    assert_eq!(
        automatic_restart_backoff(Duration::from_millis(600), 2).as_nanos(),
        1_000_000_000
    );
}

/// Spec 11 §6.6: every consecutive retry delay is nonzero, monotonic non-decreasing and within the one millisecond to one second bounds, for any configured cycle interval.
#[test]
fn automatic_restart_backoff_is_nonzero_monotonic_and_bounded_for_every_attempt() {
    for interval in [
        Duration::ZERO,
        Duration::from_nanos(1),
        Duration::from_millis(1),
        Duration::from_millis(250),
        Duration::from_secs(1),
        Duration::from_secs(60),
    ] {
        let interval_ns = interval.as_nanos();
        let mut previous = 0_i64;
        for attempt in 1..=12 {
            let delay = automatic_restart_backoff(interval, attempt).as_nanos();
            assert!(
                (1_000_000..=1_000_000_000).contains(&delay),
                "delay {delay} ns is outside 1 ms..=1 s for cycle interval {interval_ns} ns, attempt {attempt}"
            );
            assert!(
                delay >= previous,
                "delay {delay} ns fell below the previous attempt's {previous} ns for cycle interval {interval_ns} ns"
            );
            previous = delay;
        }
    }
}
