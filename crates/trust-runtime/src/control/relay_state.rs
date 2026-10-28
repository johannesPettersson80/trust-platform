//! Shared hysteresis transition for live and isolated feedback-relay acquisition.

/// Callers own measurement qualification, bounds, timing, writes and restoration.
pub(super) fn next_relay_state(
    measured: f64,
    center: f64,
    hysteresis: f64,
    positive_gain: bool,
    was_high: bool,
) -> bool {
    if measured >= center + hysteresis {
        !positive_gain
    } else if measured <= center - hysteresis {
        positive_gain
    } else {
        was_high
    }
}
