//! The `clock.set` operation (specification 11, the runtime clock contract; specification 34
//! TARGET-06): the computer at the other end of the control connection gives a device without a
//! battery-held clock its time. A computer's operating system holds its clock, so trust-runtime
//! refuses the request and changes nothing.

use alloc::format;
use alloc::string::ToString;

use serde_json::{json, Value};
use trust_runtime_core::ports::WallClock;

use crate::protocol::ControlResponse;

/// The code of the refusal of a runtime whose operating system holds its clock.
pub const CLOCK_NOT_SETTABLE: &str = "clock_not_settable";

/// `clock.set`: sets the clock to the `utc_ms` of `params` and answers the clock after the set and
/// the one before it, `null` while it was not set; or refuses parameters that are not one whole
/// number of milliseconds, or a runtime that does not hold its clock.
#[must_use]
pub fn set<H: WallClock>(id: u64, params: Option<&Value>, host: &H) -> ControlResponse {
    let utc_ms = match utc_ms(params) {
        Ok(utc_ms) => utc_ms,
        Err(reason) => return ControlResponse::error(id, format!("invalid params: {reason}")),
    };
    let was = host.utc_ms();
    if !host.set_utc_ms(utc_ms) {
        return ControlResponse::error_with_code(
            id,
            "the operating system holds this runtime's clock".to_string(),
            CLOCK_NOT_SETTABLE,
        );
    }
    ControlResponse::ok(id, json!({ "utc_ms": host.utc_ms(), "was_utc_ms": was }))
}

/// The milliseconds since the Unix epoch that `params` give, or why they are refused.
fn utc_ms(params: Option<&Value>) -> Result<u64, &'static str> {
    let Some(Value::Object(params)) = params else {
        return Err("utc_ms is required");
    };
    if params.keys().any(|key| key != "utc_ms") {
        return Err("only utc_ms is taken");
    }
    params
        .get("utc_ms")
        .ok_or("utc_ms is required")?
        .as_u64()
        .ok_or("utc_ms must be a whole number of milliseconds since 1970-01-01T00:00:00Z")
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;

    use serde_json::json;

    use super::*;

    /// A device's clock: not set until `set_utc_ms`.
    struct DeviceClock(Cell<Option<u64>>);

    impl WallClock for DeviceClock {
        fn utc_ms(&self) -> Option<u64> {
            self.0.get()
        }

        fn set_utc_ms(&self, utc_ms: u64) -> bool {
            self.0.set(Some(utc_ms));
            true
        }
    }

    /// A computer's clock, which its operating system holds.
    struct ComputerClock;

    impl WallClock for ComputerClock {
        fn utc_ms(&self) -> Option<u64> {
            Some(1_761_400_000_000)
        }
    }

    /// Specification 11, the runtime clock contract: a device takes the computer's time and
    /// answers its clock after the set and the one before it, `null` the first time. Catches a
    /// device that answers without setting its clock, answers the time it had as the new one, or
    /// invents a time before the first set.
    #[test]
    fn a_device_takes_the_time_it_is_given_and_answers_its_clock_before_and_after() {
        let device = DeviceClock(Cell::new(None));
        let first = set(7, Some(&json!({"utc_ms": 1_761_400_000_123_u64})), &device);
        assert!(first.ok, "{:?}", first.error);
        assert_eq!(
            first.result,
            Some(json!({"utc_ms": 1_761_400_000_123_u64, "was_utc_ms": null}))
        );
        assert_eq!(device.utc_ms(), Some(1_761_400_000_123));
        let second = set(8, Some(&json!({"utc_ms": 1_761_400_060_000_u64})), &device);
        assert_eq!(
            second.result,
            Some(json!({"utc_ms": 1_761_400_060_000_u64, "was_utc_ms": 1_761_400_000_123_u64}))
        );
    }

    /// A computer's operating system holds its clock: the request is refused with the stable code
    /// and changes nothing. Catches a computer that reports success for a clock it never set.
    #[test]
    fn a_computer_refuses_with_its_code() {
        let reply = set(3, Some(&json!({"utc_ms": 5_u64})), &ComputerClock);
        assert!(!reply.ok);
        assert_eq!(reply.error_code.as_deref(), Some(CLOCK_NOT_SETTABLE));
        assert_eq!(
            reply.error.as_deref(),
            Some("the operating system holds this runtime's clock")
        );
    }

    /// Parameters that are not one whole number of milliseconds are refused before the clock is
    /// touched. Catches a negative, fractional or missing time, or an unknown parameter, set as
    /// some time.
    #[test]
    fn parameters_that_are_not_one_whole_number_of_milliseconds_are_refused() {
        for params in [
            None,
            Some(json!({})),
            Some(json!({"utc_ms": -1})),
            Some(json!({"utc_ms": 1.5})),
            Some(json!({"utc_ms": "1"})),
            Some(json!({"utc_ms": 1, "zone": "UTC"})),
            Some(json!([1])),
        ] {
            let device = DeviceClock(Cell::new(None));
            let reply = set(1, params.as_ref(), &device);
            assert!(!reply.ok, "{params:?}");
            assert!(
                reply
                    .error
                    .as_deref()
                    .is_some_and(|error| error.starts_with("invalid params: ")),
                "{params:?}: {:?}",
                reply.error
            );
            assert_eq!(device.utc_ms(), None, "{params:?}");
        }
    }
}
