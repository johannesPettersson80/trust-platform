use super::*;

pub(super) struct ValidationBudget {
    limits: ValidationLimits,
    storage: usize,
    pub(super) stats: ValidationStats,
}

impl ValidationBudget {
    pub(super) fn new(limits: ValidationLimits) -> Self {
        Self {
            limits,
            storage: 0,
            stats: ValidationStats::default(),
        }
    }

    pub(super) fn work(&mut self, units: usize) -> Result<(), BytecodeError> {
        let next = self
            .stats
            .work
            .checked_add(units)
            .ok_or(RejectionReason::ValidationWorkLimit)?;
        if next > self.limits.max_work {
            return Err(BytecodeError::from(RejectionReason::ValidationWorkLimit));
        }
        self.stats.work = next;
        Ok(())
    }

    pub(super) fn storage(&mut self, bytes: usize) -> Result<(), BytecodeError> {
        let next = self
            .storage
            .checked_add(bytes)
            .ok_or(RejectionReason::ValidationStorageLimit)?;
        if next > self.limits.max_scratch_bytes {
            return Err(BytecodeError::from(RejectionReason::ValidationStorageLimit));
        }
        self.storage = next;
        self.stats.peak_scratch_bytes = self.stats.peak_scratch_bytes.max(next);
        Ok(())
    }

    pub(super) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        required: usize,
    ) -> Result<(), BytecodeError> {
        if required > values.capacity() {
            self.work(values.len())?; // reserve may move every existing element
            let extra = required - values.capacity();
            self.storage(
                extra
                    .checked_mul(core::mem::size_of::<T>())
                    .ok_or(RejectionReason::ValidationStorageLimit)?,
            )?;
            values
                .try_reserve_exact(required - values.len())
                .map_err(|_| RejectionReason::ValidationStorageLimit)?;
        }
        Ok(())
    }

    pub(super) fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), BytecodeError> {
        if values.len() == values.capacity() {
            let next = values
                .capacity()
                .max(16)
                .checked_mul(2)
                .ok_or(RejectionReason::ValidationStorageLimit)?;
            self.reserve(values, next)?;
        }
        values.push(value);
        Ok(())
    }

    pub(super) fn push_stack<T>(
        &mut self,
        values: &mut Vec<T>,
        value: T,
    ) -> Result<(), BytecodeError> {
        validate_operand_stack_depth(
            values
                .len()
                .checked_add(1)
                .ok_or(RejectionReason::ValidationStorageLimit)?,
        )?;
        self.push(values, value)
    }

    pub(super) fn copy<T: Copy>(
        &mut self,
        target: &mut Vec<T>,
        source: &[T],
    ) -> Result<(), BytecodeError> {
        self.work(source.len())?;
        target.clear();
        self.reserve(target, source.len())?;
        target.extend_from_slice(source);
        Ok(())
    }

    /// Temporary analysis results do not escape; only work/peak accounting survives.
    pub(super) fn temporary(
        &mut self,
        run: impl FnOnce(&mut Self) -> Result<(), BytecodeError>,
    ) -> Result<(), BytecodeError> {
        let retained = self.storage;
        let result = run(self);
        self.storage = retained;
        result
    }
}

impl ValidationBudget {
    /// Fallible binary search permits charging comparisons before doing them.
    pub(super) fn search_by<T>(
        &mut self,
        values: &[T],
        mut compare: impl FnMut(&T, &mut Self) -> Result<core::cmp::Ordering, BytecodeError>,
    ) -> Result<Result<usize, usize>, BytecodeError> {
        let (mut low, mut high) = (0, values.len());
        while low < high {
            self.work(1)?;
            let middle = low + (high - low) / 2;
            match compare(&values[middle], self)? {
                core::cmp::Ordering::Less => low = middle + 1,
                core::cmp::Ordering::Greater => high = middle,
                core::cmp::Ordering::Equal => return Ok(Ok(middle)),
            }
        }
        Ok(Err(low))
    }

    /// In-place heapsort with fallible comparisons: exhaustion stops work immediately.
    pub(super) fn sort_by<T>(
        &mut self,
        values: &mut [T],
        mut compare: impl FnMut(&T, &T, &mut Self) -> Result<core::cmp::Ordering, BytecodeError>,
    ) -> Result<(), BytecodeError> {
        fn sift<T>(
            values: &mut [T],
            mut root: usize,
            budget: &mut ValidationBudget,
            compare: &mut impl FnMut(
                &T,
                &T,
                &mut ValidationBudget,
            ) -> Result<core::cmp::Ordering, BytecodeError>,
        ) -> Result<(), BytecodeError> {
            while root < values.len() / 2 {
                let mut child = root * 2 + 1;
                budget.work(1)?;
                if child + 1 < values.len()
                    && compare(&values[child], &values[child + 1], budget)?.is_lt()
                {
                    child += 1;
                }
                budget.work(1)?;
                if !compare(&values[root], &values[child], budget)?.is_lt() {
                    break;
                }
                budget.work(1)?;
                values.swap(root, child);
                root = child;
            }
            Ok(())
        }
        for root in (0..values.len() / 2).rev() {
            sift(values, root, self, &mut compare)?;
        }
        for end in (1..values.len()).rev() {
            self.work(1)?;
            values.swap(0, end);
            sift(&mut values[..end], 0, self, &mut compare)?;
        }
        Ok(())
    }

    pub(super) fn lower_bound<T>(
        &mut self,
        values: &[T],
        mut less: impl FnMut(&T, &mut Self) -> Result<bool, BytecodeError>,
    ) -> Result<usize, BytecodeError> {
        let (mut low, mut high) = (0, values.len());
        while low < high {
            self.work(1)?;
            let middle = low + (high - low) / 2;
            if less(&values[middle], self)? {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Ok(low)
    }

    pub(super) fn compare_names(
        &mut self,
        left: &str,
        right: &str,
    ) -> Result<core::cmp::Ordering, BytecodeError> {
        for (left, right) in left.bytes().zip(right.bytes()) {
            self.work(1)?;
            let order = left.to_ascii_uppercase().cmp(&right.to_ascii_uppercase());
            if !order.is_eq() {
                return Ok(order);
            }
        }
        self.work(1)?;
        Ok(left.len().cmp(&right.len()))
    }

    pub(super) fn concat(&mut self, parts: &[&str]) -> Result<String, BytecodeError> {
        let len = parts.iter().try_fold(0usize, |sum, part| {
            sum.checked_add(part.len())
                .ok_or(RejectionReason::ValidationStorageLimit)
        })?;
        self.storage(len)?;
        self.work(len)?;
        let mut value = String::new();
        value
            .try_reserve_exact(len)
            .map_err(|_| RejectionReason::ValidationStorageLimit)?;
        for part in parts {
            value.push_str(part);
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn fallible_sort_handles_empty_singleton_even_odd_and_duplicates() {
        for mut values in [
            vec![],
            vec![4],
            vec![2, 1],
            vec![5, 2, 4, 1, 3],
            vec![3, 1, 3, 2, 1, 0],
        ] {
            let mut expected = values.clone();
            expected.sort_unstable();
            let mut budget = ValidationBudget::new(ValidationLimits::default());
            budget.sort_by(&mut values, |a, b, _| Ok(a.cmp(b))).unwrap();
            assert_eq!(values, expected);
        }
    }

    #[test]
    fn sort_and_lookup_stop_when_work_budget_is_exhausted() {
        let limits = ValidationLimits {
            max_work: 0,
            ..ValidationLimits::default()
        };
        let mut budget = ValidationBudget::new(limits);
        let mut values = [2, 1];
        assert_eq!(
            budget
                .sort_by(&mut values, |a, b, _| Ok(a.cmp(b)))
                .unwrap_err(),
            BytecodeError::from(RejectionReason::ValidationWorkLimit)
        );
        assert_eq!(values, [2, 1]);
        assert_eq!(
            budget.lower_bound(&values, |a, _| Ok(*a < 2)).unwrap_err(),
            BytecodeError::from(RejectionReason::ValidationWorkLimit)
        );
        assert_eq!(budget.stats.work, 0);
    }

    #[test]
    fn lower_and_upper_bounds_preserve_duplicate_wire_order() {
        let mut budget = ValidationBudget::new(ValidationLimits::default());
        let values = [1, 2, 2, 2, 4];
        assert_eq!(budget.lower_bound(&values, |a, _| Ok(*a < 2)).unwrap(), 1);
        assert_eq!(budget.lower_bound(&values, |a, _| Ok(*a <= 2)).unwrap(), 4);
        assert_eq!(budget.lower_bound(&values, |a, _| Ok(*a < 0)).unwrap(), 0);
        assert_eq!(budget.lower_bound(&values, |a, _| Ok(*a < 5)).unwrap(), 5);
    }
}
