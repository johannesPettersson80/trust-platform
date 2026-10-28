//! Steady record periods (specification 35, A4.2; runtime protocol annex, E4.4 and E4.5).
//!
//! A sample of a recording carries the sequence value of its last execution. Consecutive samples step
//! by one record period `k`; only a stopped recording may end with a shorter, partial period.

/// The largest integer JSON carries exactly: 2⁵³ − 1 (E0.3).
const MAX_SAFE_INTEGER: i128 = (1 << 53) - 1;

/// Whether consecutive sample sequence values step by one steady record period `k` (specification 35,
/// A4.2; annex E4.4).
///
/// Two full steps establish `k`, so it is read from the first step of three or more samples; two samples
/// must be consecutive, as at the runtime's default `k = 1`. With `final_may_be_partial`, the last step may
/// be shorter than `k`, because the final sample of a stopped recording may average fewer executions.
/// Any other change of step is a missed execution (annex E4.5).
#[must_use]
pub fn steady_record_period(sequences: &[u64], final_may_be_partial: bool) -> bool {
    if sequences.len() < 2 {
        return true;
    }
    let delta = |index: usize| i128::from(sequences[index]) - i128::from(sequences[index - 1]);
    let period = if sequences.len() >= 3 { delta(1) } else { 1 };
    if !(1..=MAX_SAFE_INTEGER).contains(&period) {
        return false;
    }
    let last = sequences.len() - 1;
    (1..=last).all(|index| {
        let step = delta(index);
        if final_may_be_partial && index == last {
            (1..=period).contains(&step)
        } else {
            step == period
        }
    })
}

#[cfg(test)]
mod tests {
    use super::steady_record_period;

    #[test]
    fn a_steady_period_is_read_from_the_first_two_steps() {
        assert!(steady_record_period(&[10, 11, 12], false));
        assert!(
            steady_record_period(&[10, 11], true),
            "two consecutive samples establish k = 1"
        );
    }

    #[test]
    fn only_a_stopped_recording_ends_with_a_partial_period() {
        assert!(
            steady_record_period(&[10, 14, 18, 20], true),
            "a stopped recording's final sample may cover fewer executions"
        );
        assert!(
            !steady_record_period(&[10, 14, 18, 20], false),
            "only a stopped recording ends with a partial period"
        );
    }

    #[test]
    fn a_missed_execution_is_refused() {
        assert!(
            !steady_record_period(&[10, 14, 19], true),
            "a final step longer than the period is a missed execution"
        );
        assert!(
            !steady_record_period(&[10, 14, 17, 21], false),
            "a period that changes is a missed execution"
        );
        assert!(
            !steady_record_period(&[10, 14], true),
            "two samples cannot establish a period, so they must be consecutive"
        );
    }

    #[test]
    fn a_period_must_advance() {
        assert!(
            !steady_record_period(&[5, 4, 3], false),
            "a decreasing sequence has no period"
        );
        assert!(
            !steady_record_period(&[0, 0, 0], false),
            "a repeated sequence value has no period"
        );
    }
}
