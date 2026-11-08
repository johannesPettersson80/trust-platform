use super::{Clock, ScaledClock, StdClock};
use crate::value::Duration;

#[test]
fn scaled_clock_clamps_zero_to_one() {
    let clock = ScaledClock::new(0);
    assert_eq!(clock.scale(), 1);
}

#[test]
fn scaled_clock_sleeps_faster_than_std_clock() {
    let std_clock = StdClock::new();
    let std_deadline = Duration::from_millis(200);
    let std_start = std::time::Instant::now();
    std_clock.sleep_until(std_deadline);
    let std_elapsed = std_start.elapsed();

    let scaled_clock = ScaledClock::new(20);
    let scaled_deadline = Duration::from_millis(20);
    let scaled_start = std::time::Instant::now();
    scaled_clock.sleep_until(scaled_deadline);
    let scaled_elapsed = scaled_start.elapsed();

    assert!(std_elapsed >= std::time::Duration::from_millis(10));
    assert!(scaled_elapsed < std_elapsed);
}

#[test]
fn scaled_clock_now_is_monotonic() {
    let clock = ScaledClock::new(10);
    let first = clock.now();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let second = clock.now();
    assert!(second.as_nanos() >= first.as_nanos());
}
