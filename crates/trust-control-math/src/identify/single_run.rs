//! The step of a record (`largestInputChange` in `identification.ts`, made aware of averaged samples): the step that places
//! a held single-run or validated record and its operating point (N2.2, N1.3). The fit itself is `fit`'s, and the step's
//! response is the recorded-input simulation of `simulate` (N3.1).

/// The step of a record (N2.2): its largest plant input change, counted whole through a sample that straddles it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputStep {
    /// The step's first sample: the straddling sample when there is one, else the first sample at the new level.
    pub index: usize,
    /// The first sample wholly at the new level: `index + 1` after a straddling sample, else `index`.
    pub end: usize,
    /// The whole change, `u[end] − u[index − 1]`.
    pub change: f64,
}

/// The step of `inputs` (N2.2). A sample whose input lies strictly between those of its neighbours, on a change in one
/// direction, straddles the step: its record period (specification 35, A4.2) or its block (N1.4) averaged executions
/// before and after the step. The change through it counts whole, from the sample before it to the sample after it, and
/// the step starts at the straddling sample, so the samples before the step never hold it. The step is the first change
/// that is strictly larger than every earlier one and above zero, so the first of equal changes wins. `None` when no
/// change is above zero.
#[must_use]
pub fn input_step(inputs: &[f64]) -> Option<InputStep> {
    let mut found: Option<InputStep> = None;
    for index in 1..inputs.len() {
        let end = if straddles(inputs, index) {
            index + 1
        } else {
            index
        };
        let change = inputs[end] - inputs[index - 1];
        if change.abs() > found.map_or(0.0, |step| step.change.abs()) {
            found = Some(InputStep { index, end, change });
        }
    }
    found
}

/// Whether the sample at `index` lies strictly between its neighbours: the changes into it and out of it are both above
/// zero and in one direction.
#[must_use]
pub fn straddles(inputs: &[f64], index: usize) -> bool {
    let (Some(&before), Some(&value), Some(&after)) = (
        index.checked_sub(1).and_then(|before| inputs.get(before)),
        inputs.get(index),
        inputs.get(index + 1),
    ) else {
        return false;
    };
    (value - before) * (after - value) > 0.0
}

/// The first sample of the step of `inputs` ([`input_step`]); `None` when no change is above zero.
#[must_use]
pub fn largest_input_change(inputs: &[f64]) -> Option<usize> {
    input_step(inputs).map(|step| step.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_largest_input_change_is_the_first_of_the_largest_steps() {
        assert_eq!(
            largest_input_change(&[1.0, 1.0, 3.0, 3.0, 0.0, 0.0]),
            Some(4)
        );
        assert_eq!(
            largest_input_change(&[0.0, 2.0, 2.0, 0.0]),
            Some(1),
            "an equal later change does not replace the first"
        );
        assert_eq!(largest_input_change(&[5.0, 5.0, 5.0]), None, "no change");
        assert_eq!(largest_input_change(&[7.0]), None);
        assert_eq!(largest_input_change(&[]), None);
    }

    /// A step of 10 whose edge falls inside a sample of 10 averaged executions (N2.2): at 50 % the sample reads 5, at
    /// 75 % 2.5, at 25 % 7.5. Each counts whole, 10, and starts at the straddling sample. Wrong behaviour caught: the
    /// step placed at the larger of the two partial changes, one sample late at 75 %, so the straddling sample joins the
    /// samples before the step.
    #[test]
    fn a_step_counts_whole_through_the_sample_that_straddles_it_and_starts_there() {
        let whole = |index, end| InputStep {
            index,
            end,
            change: 10.0,
        };
        assert_eq!(input_step(&[0.0, 0.0, 5.0, 10.0, 10.0]), Some(whole(2, 3)));
        assert_eq!(input_step(&[0.0, 0.0, 2.5, 10.0, 10.0]), Some(whole(2, 3)));
        assert_eq!(input_step(&[0.0, 0.0, 7.5, 10.0, 10.0]), Some(whole(2, 3)));
        // On a sample boundary no sample straddles.
        assert_eq!(input_step(&[0.0, 0.0, 10.0, 10.0]), Some(whole(2, 2)));
        // Downwards the same: 10 to 0 through 4.
        assert_eq!(
            input_step(&[10.0, 10.0, 4.0, 0.0]),
            Some(InputStep {
                index: 2,
                end: 3,
                change: -10.0
            })
        );
    }

    /// The step and its restoration both straddle (N2.2): the step at 55 % of a sample (4.5 then 5.5), the restoration
    /// at 35 % (6.5 then 3.5). Whole, both are 10, so the first, the step, wins. Wrong behaviour caught: the
    /// restoration's partial 6.5 beats the step's partial 5.5 and is taken as the step, which left too few samples after
    /// it and refused the recording ("A bounded input step with pre- and post-step data is required.").
    #[test]
    fn a_straddled_step_beats_its_restoration_that_straddles_less_evenly() {
        let inputs = [0.0, 0.0, 4.5, 10.0, 10.0, 10.0, 3.5, 0.0, 0.0];
        assert_eq!(
            input_step(&inputs),
            Some(InputStep {
                index: 2,
                end: 3,
                change: 10.0
            })
        );
        assert_eq!(largest_input_change(&inputs), Some(2));
    }

    #[test]
    fn a_sample_straddles_only_between_two_changes_in_one_direction() {
        assert!(straddles(&[0.0, 5.0, 10.0], 1));
        assert!(!straddles(&[0.0, 10.0, 10.0], 1), "no change out of it");
        assert!(!straddles(&[0.0, 10.0, 5.0], 1), "a peak, not a straddle");
        assert!(!straddles(&[0.0, 5.0], 1), "no sample after it");
        assert!(!straddles(&[5.0, 10.0], 0), "no sample before it");
    }
}
