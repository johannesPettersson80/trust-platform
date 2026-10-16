//! Compiler-independent host sampling; IEC clock encoding is owned by the core.
use std::time::{SystemTime, UNIX_EPOCH};
use trust_runtime_core::{error::RuntimeError, value::DateTimeValue};

#[cfg(any(feature = "full", test))]
use trust_runtime_core::value::{Duration, Value};

/// Read logical elapsed time or the current host UTC date/time.
#[cfg(feature = "full")]
pub fn runtime_clock_value(name: &str, elapsed: Duration) -> Result<Value, RuntimeError> {
    runtime_clock_value_at(name, elapsed, SystemTime::now())
}

#[cfg(any(feature = "full", test))]
pub(crate) fn runtime_clock_value_at(
    name: &str,
    elapsed: Duration,
    host_now: SystemTime,
) -> Result<Value, RuntimeError> {
    trust_runtime_core::stdlib::time::runtime_clock_value(name, elapsed, || current_dt(host_now))
}

pub(crate) fn current_dt(now: SystemTime) -> Result<DateTimeValue, RuntimeError> {
    trust_runtime_core::stdlib::time::current_dt_from_epoch_elapsed(
        now.duration_since(UNIX_EPOCH).ok(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_host_composition_uses_the_shared_utc_clock_encoding() {
        let now = UNIX_EPOCH + std::time::Duration::from_nanos(1_999_999);
        assert_eq!(current_dt(now), Ok(DateTimeValue::new(1)));
        assert_eq!(
            runtime_clock_value_at("CURRENT_DT", Duration::from_secs(99), now),
            Ok(Value::Dt(DateTimeValue::new(1)))
        );
        assert_eq!(
            runtime_clock_value_at("TIME", Duration::from_millis(37), now),
            Ok(Value::Time(Duration::from_millis(37)))
        );
        // One microsecond, not one nanosecond: Windows clocks have 100 ns
        // resolution, so a 1 ns step before the epoch rounds to the epoch.
        assert_eq!(
            current_dt(UNIX_EPOCH - std::time::Duration::from_micros(1)),
            Err(RuntimeError::Overflow)
        );
    }
}
