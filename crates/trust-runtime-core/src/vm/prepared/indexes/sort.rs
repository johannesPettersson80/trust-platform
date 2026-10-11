//! Shared bounded sorting, with every comparison and swap charged before work.
use super::*;

#[inline(never)]
pub(in crate::vm::prepared) fn sort_by<T>(
    values: &mut [T],
    budget: &mut PreparationBudget,
    compare: &mut dyn FnMut(&T, &T, &mut PreparationBudget) -> Result<Ordering, RuntimeError>,
) -> Result<(), RuntimeError> {
    crate::sort::heap_sort(values.len(), &mut |operation| {
        use crate::sort::Operation;
        match operation {
            Operation::Charge => {
                budget.charge(0, 1)?;
                Ok(false)
            }
            Operation::Less(left, right) => {
                Ok(compare(&values[left], &values[right], budget)?.is_lt())
            }
            Operation::Swap(left, right) => {
                values.swap(left, right);
                Ok(false)
            }
        }
    })
}

pub(super) fn compare_names_charged(
    left: &str,
    right: &str,
    budget: &mut PreparationBudget,
) -> Result<Ordering, RuntimeError> {
    for (left, right) in left.bytes().zip(right.bytes()) {
        budget.charge(0, 1)?;
        let order = left.to_ascii_uppercase().cmp(&right.to_ascii_uppercase());
        if !order.is_eq() {
            return Ok(order);
        }
    }
    budget.charge(0, 1)?;
    Ok(left.len().cmp(&right.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_sort_charges_bytes_and_uses_the_runtime_case_insensitive_order() {
        let mut values = ["beta", "ALPHA", "alpha", "Gamma", ""];
        let mut budget = PreparationBudget::new(PreparationLimits::default());
        sort_by(&mut values, &mut budget, &mut |left, right, budget| {
            compare_names_charged(left, right, budget)
        })
        .unwrap();
        assert!(values
            .windows(2)
            .all(|pair| !compare_names(pair[0], pair[1]).is_gt()));
        assert!(budget.usage().work > 0);
        assert_eq!(budget.usage().bytes, 0);

        let mut equal = ["aB", "Ab"];
        let mut exact = PreparationBudget::new(PreparationLimits {
            max_preparation_work: 6,
            ..Default::default()
        });
        sort_by(&mut equal, &mut exact, &mut |left, right, budget| {
            compare_names_charged(left, right, budget)
        })
        .unwrap();
        assert_eq!(exact.usage().work, 6); // Three sort charges, two bytes and length.
        assert_eq!(equal, ["Ab", "aB"]);

        let mut values = ["aB", "Ab"];
        let mut exhausted = PreparationBudget::new(PreparationLimits {
            max_preparation_work: 4,
            ..Default::default()
        });
        assert_eq!(
            sort_by(&mut values, &mut exhausted, &mut |left, right, budget| {
                compare_names_charged(left, right, budget)
            }),
            Err(RuntimeError::PreparationLimit)
        );
        assert_eq!(values, ["aB", "Ab"]);
        assert_eq!(exhausted.usage().work, 4);
    }
}
