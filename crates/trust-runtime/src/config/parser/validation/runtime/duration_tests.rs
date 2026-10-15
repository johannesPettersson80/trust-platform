//! Range refusal for millisecond settings read from runtime TOML.

use super::parse_runtime_duration_millis;
use crate::error::RuntimeError;
use crate::value::Duration;

fn invalid_config_text(result: Result<Duration, RuntimeError>) -> String {
    match result.expect_err("the duration must be refused") {
        RuntimeError::InvalidConfig(message) => message.to_string(),
        other => panic!("expected an invalid-config refusal, got {other}"),
    }
}

#[test]
fn largest_representable_runtime_milliseconds_are_accepted_exactly() {
    let largest = (i64::MAX / 1_000_000) as u64;
    assert_eq!(
        parse_runtime_duration_millis(largest, 0, "resource.cycle_interval_ms")
            .expect("the largest signed duration must stay representable"),
        Duration::from_nanos(9_223_372_036_854_000_000)
    );
}

#[test]
fn runtime_i64_max_and_u64_max_milliseconds_are_refused_with_the_range_error() {
    for value in [i64::MAX as u64, u64::MAX] {
        assert_eq!(
            invalid_config_text(parse_runtime_duration_millis(
                value,
                0,
                "resource.cycle_interval_ms"
            )),
            "resource.cycle_interval_ms must be <= 9223372036854"
        );
    }
}
