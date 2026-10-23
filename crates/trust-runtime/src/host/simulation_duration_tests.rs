//! Range refusal for millisecond values read by the simulation descriptor parser.

use super::duration_from_millis;
use crate::error::RuntimeError;
use crate::value::Duration;

fn invalid_config_text(result: Result<Duration, RuntimeError>) -> String {
    match result.expect_err("the duration must be refused") {
        RuntimeError::InvalidConfig(message) => message.to_string(),
        other => panic!("expected an invalid-config refusal, got {other}"),
    }
}

#[test]
fn simulation_milliseconds_beyond_the_signed_nanosecond_range_are_refused() {
    for value in [i64::MAX as u64, u64::MAX] {
        assert_eq!(
            invalid_config_text(duration_from_millis("physics.step_ms", value)),
            "physics.step_ms must be <= 9223372036854"
        );
    }
}

#[test]
fn simulation_largest_representable_millisecond_value_is_exact() {
    assert_eq!(
        duration_from_millis("physics.step_ms", 9_223_372_036_854)
            .expect("the largest signed duration must stay representable"),
        Duration::from_nanos(9_223_372_036_854_000_000)
    );
}
