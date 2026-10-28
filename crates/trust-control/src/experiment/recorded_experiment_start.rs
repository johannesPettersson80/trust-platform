//! Where a recording's experiment data begins, and where a connected identification's steady baseline window ends
//! (`recordedExperimentStart.ts`; specification 35, B3.4).
//!
//! The window belongs to the identification request, which names it (`baselineStartSeconds` and
//! `baselineEndSeconds`), not to the recorded run (annex E4.7). The two values are therefore passed in as the request
//! gives them, and a recording with no window begins with its data.

/// Where a recording's experiment data begins, in its own seconds (`experimentStartSeconds`). A connected
/// identification records the wait for a steady baseline too, and its request names where the steady baseline
/// window starts. The wait before it stays in the recording but is never used for fits or noise estimates. Other
/// recordings begin with their data, at 0.
#[must_use]
pub fn experiment_start_seconds(baseline_start_seconds: Option<f64>) -> f64 {
    match baseline_start_seconds {
        Some(start) if start.is_finite() && start > 0.0 => start,
        _ => 0.0,
    }
}

/// Where a connected identification's steady baseline window ends, in its own seconds (`baselineEndSeconds`), or
/// `None` for other recordings. The window must end after the experiment's data begins.
#[must_use]
pub fn baseline_end_seconds(
    baseline_start_seconds: Option<f64>,
    baseline_end_seconds: Option<f64>,
) -> Option<f64> {
    let start = experiment_start_seconds(baseline_start_seconds);
    baseline_end_seconds.filter(|end| end.is_finite() && *end > start)
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // The windows are exact values from the request.
mod tests {
    use super::{baseline_end_seconds, experiment_start_seconds};

    #[test]
    fn a_recording_without_a_window_begins_with_its_data() {
        assert_eq!(experiment_start_seconds(None), 0.0);
        assert_eq!(experiment_start_seconds(Some(0.0)), 0.0);
        assert_eq!(experiment_start_seconds(Some(-1.0)), 0.0);
        assert_eq!(experiment_start_seconds(Some(f64::NAN)), 0.0);
        assert_eq!(experiment_start_seconds(Some(5.0)), 5.0);
        assert_eq!(baseline_end_seconds(None, None), None);
    }

    #[test]
    fn the_steady_baseline_ends_after_the_data_begins() {
        assert_eq!(baseline_end_seconds(Some(5.0), Some(35.1)), Some(35.1));
        assert_eq!(
            baseline_end_seconds(Some(5.0), Some(5.0)),
            None,
            "an empty window is no window"
        );
        assert_eq!(
            baseline_end_seconds(None, Some(10.0)),
            Some(10.0),
            "the data begins at 0"
        );
        assert_eq!(baseline_end_seconds(Some(5.0), None), None);
        assert_eq!(baseline_end_seconds(Some(5.0), Some(f64::INFINITY)), None);
    }
}
