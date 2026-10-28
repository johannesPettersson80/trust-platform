//! The largest input change of a record (`largestInputChange` in `identification.ts`): the step that places a held
//! single-run or validated record and its operating point (N2.2, N1.3). The fit itself is `fit`'s, and the step's
//! response is the recorded-input simulation of `simulate` (N3.1).

/// The index `i ≥ 1` of the largest input change `|uᵢ − uᵢ₋₁|` (`largestInputChange` in `identification.ts`): the
/// first index whose change is strictly larger than every earlier one and above zero, so the first of equal changes
/// wins. `None` when no change is above zero.
#[must_use]
pub fn largest_input_change(inputs: &[f64]) -> Option<usize> {
    let mut found = None;
    let mut largest_change = 0.0;
    for (index, pair) in inputs.windows(2).enumerate() {
        let change = (pair[1] - pair[0]).abs();
        if change > largest_change {
            largest_change = change;
            found = Some(index + 1);
        }
    }
    found
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
}
