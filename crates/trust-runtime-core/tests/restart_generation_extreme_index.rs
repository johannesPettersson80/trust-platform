use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{array_offset_i64, checked_array_offset_i64};

/// Spec 12 §7.8: an invalid index faults, and arrays with extreme signed lower bounds are covered; one step past the upper bound of an array starting at i64::MIN is an index fault.
#[test]
fn one_step_past_the_upper_bound_at_an_extreme_lower_bound_is_an_index_fault() {
    let dimensions = [(i64::MIN, i64::MIN + 1)];
    let index = i64::MIN + 2;

    assert_eq!(array_offset_i64(&dimensions, &[index]), None);
    assert_eq!(
        checked_array_offset_i64(&dimensions, &[index]),
        Err(RuntimeError::IndexOutOfBounds {
            index,
            lower: i64::MIN,
            upper: i64::MIN + 1,
        })
    );
}

/// Spec 12 §7.8: the last valid element of an array starting at i64::MIN resolves to its exact offset.
#[test]
fn last_valid_element_at_an_extreme_lower_bound_resolves_exactly() {
    assert_eq!(
        checked_array_offset_i64(&[(i64::MIN, i64::MIN + 1)], &[i64::MIN + 1]),
        Ok(1)
    );
}

/// Spec 12 §7.8: index arithmetic is overflow-checked with extreme signed lower bounds. The spec names no error variant for this case, so the test requires a refusal and never a wrapped offset.
#[test]
fn linearised_offset_overflow_with_extreme_lower_bounds_is_refused_not_wrapped() {
    let dimensions = [(i64::MIN, i64::MAX), (0, 1)];

    assert!(checked_array_offset_i64(&dimensions, &[i64::MAX, 0]).is_err());
    assert_eq!(checked_array_offset_i64(&dimensions, &[i64::MIN, 1]), Ok(1));
}
