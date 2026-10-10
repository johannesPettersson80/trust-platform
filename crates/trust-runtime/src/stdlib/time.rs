//! Host wall-clock adapter; deterministic date/time semantics are shared.

use crate::error::RuntimeError;
use crate::value::{DateTimeValue, Duration, Value};
use std::time::{SystemTime, UNIX_EPOCH};
pub use trust_runtime_core::stdlib::time::*;

/// Read logical elapsed time or the current host UTC date/time.
pub fn runtime_clock_value(name: &str, elapsed: Duration) -> Result<Value, RuntimeError> {
    runtime_clock_value_at(name, elapsed, SystemTime::now())
}

fn runtime_clock_value_at(
    name: &str,
    elapsed: Duration,
    host_now: SystemTime,
) -> Result<Value, RuntimeError> {
    trust_runtime_core::stdlib::time::runtime_clock_value(name, elapsed, || current_dt(host_now))
}

fn current_dt(now: SystemTime) -> Result<DateTimeValue, RuntimeError> {
    current_dt_from_epoch_elapsed(now.duration_since(UNIX_EPOCH).ok())
}

#[cfg(test)]
mod tests {
    use super::{
        current_dt, current_dt_elapsed, current_dt_from_epoch_elapsed, is_runtime_clock_name,
        runtime_clock_value, runtime_clock_value_at, DateTimeValue, Duration, RuntimeError,
        SystemTime, Value, UNIX_EPOCH,
    };
    use std::time::Duration as StdDuration;

    #[test]
    fn current_dt_reads_unix_host_time_at_millisecond_resolution() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test host clock is after the Unix epoch")
            .as_millis();

        let value = runtime_clock_value("CURRENT_DT", Duration::from_secs(86_400))
            .expect("CURRENT_DT reads the host clock");

        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test host clock is after the Unix epoch")
            .as_millis();
        let ticks = match value {
            Value::Dt(value) => u128::try_from(value.ticks())
                .expect("the current Unix host timestamp is nonnegative"),
            other => panic!("CURRENT_DT returned {other:?}"),
        };

        assert!((before..=after).contains(&ticks));
    }

    #[test]
    fn current_dt_preserves_the_full_nonnegative_dt_tick_range() {
        assert_eq!(
            current_dt_elapsed(StdDuration::from_millis(i64::MAX as u64)),
            Ok(DateTimeValue::new(i64::MAX))
        );
    }

    #[test]
    fn current_dt_uses_utc_unix_epoch_and_truncates_to_milliseconds() {
        let host_time = UNIX_EPOCH + StdDuration::from_nanos(1_234_999);

        assert_eq!(current_dt(host_time), Ok(DateTimeValue::new(1)));
    }

    #[test]
    fn current_dt_rejects_pre_epoch_and_out_of_range_host_time() {
        assert_eq!(
            current_dt_from_epoch_elapsed(None),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            current_dt_elapsed(StdDuration::from_millis(i64::MAX as u64 + 1)),
            Err(RuntimeError::Overflow)
        );
    }

    #[test]
    fn current_dt_ignores_logical_time_and_does_not_clamp_host_clock_rollback() {
        let later_host_time = UNIX_EPOCH + StdDuration::from_millis(2_000);
        let earlier_host_time = UNIX_EPOCH + StdDuration::from_millis(1_000);

        assert_eq!(
            runtime_clock_value_at("CURRENT_DT", Duration::ZERO, later_host_time),
            Ok(Value::Dt(DateTimeValue::new(2_000)))
        );
        assert_eq!(
            runtime_clock_value_at(
                "CURRENT_DT",
                Duration::from_secs(i64::MAX / 1_000_000_000),
                earlier_host_time,
            ),
            Ok(Value::Dt(DateTimeValue::new(1_000)))
        );
        assert_eq!(
            runtime_clock_value_at("TIME", Duration::from_millis(42), later_host_time,),
            Ok(Value::Time(Duration::from_millis(42)))
        );
    }

    #[test]
    fn runtime_clock_value_preserves_time_and_rejects_unknown_name() {
        let elapsed = Duration::from_nanos(-123);

        assert_eq!(
            runtime_clock_value("TIME", elapsed),
            Ok(Value::Time(elapsed))
        );
        assert_eq!(
            runtime_clock_value("NOT_A_CLOCK", elapsed),
            Err(RuntimeError::UndefinedFunction("NOT_A_CLOCK".into()))
        );
    }

    #[test]
    fn is_runtime_clock_name_recognizes_only_canonical_names() {
        assert!(is_runtime_clock_name("TIME"));
        assert!(is_runtime_clock_name("CURRENT_DT"));
        assert!(!is_runtime_clock_name("time"));
        assert!(!is_runtime_clock_name("current_dt"));
        assert!(!is_runtime_clock_name("NOT_A_CLOCK"));
    }
}
