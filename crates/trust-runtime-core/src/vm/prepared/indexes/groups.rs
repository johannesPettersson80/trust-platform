//! Build-once grouped indexes: append, bounded sort, then allocation-free lookup.
use super::*;
use core::ops::Range;

#[derive(Debug)]
pub(super) struct Groups<K, V> {
    pending: Vec<(K, usize, V)>,
    keys: Vec<(K, Range<usize>)>,
    values: Vec<V>,
}

impl<K, V> Default for Groups<K, V> {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            keys: Vec::new(),
            values: Vec::new(),
        }
    }
}

impl<K: Copy + Ord, V> Groups<K, V> {
    pub(super) fn push(
        &mut self,
        key: K,
        value: V,
        budget: &mut PreparationBudget,
    ) -> Result<(), RuntimeError> {
        reserve_one(&mut self.pending, budget)?;
        // The ordinal makes grouping stable without a scratch sorting buffer.
        self.pending.push((key, self.pending.len(), value));
        Ok(())
    }

    pub(super) fn finish(&mut self, budget: &mut PreparationBudget) -> Result<(), RuntimeError> {
        sort::sort_by(&mut self.pending, budget, &mut |a, b, _| {
            Ok(a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
        })?;
        let count = self.pending.len();
        budget.records::<V>(count)?;
        self.values
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::Overflow)?;
        for (key, _, value) in core::mem::take(&mut self.pending) {
            budget.charge(0, 1)?;
            if self.keys.last().is_none_or(|entry| entry.0 != key) {
                reserve_one(&mut self.keys, budget)?;
                self.keys.push((key, self.values.len()..self.values.len()));
            }
            self.values.push(value);
            if let Some((_, range)) = self.keys.last_mut() {
                range.end = self.values.len();
            }
        }
        Ok(())
    }

    pub(super) fn get(&self, key: &K) -> Option<&[V]> {
        let at = self.keys.binary_search_by_key(key, |entry| entry.0).ok()?;
        Some(&self.values[self.keys[at].1.clone()])
    }

    #[cfg(test)]
    pub(super) fn values(&self) -> impl Iterator<Item = &[V]> {
        self.keys
            .iter()
            .map(|(_, range)| &self.values[range.clone()])
    }

    pub(super) fn sort_values(
        &mut self,
        budget: &mut PreparationBudget,
        mut compare: impl FnMut(&V, &V) -> Ordering,
    ) -> Result<(), RuntimeError> {
        for (_, range) in &self.keys {
            budget.charge(0, 1)?;
            sort::sort_by(&mut self.values[range.clone()], budget, &mut |a, b, _| {
                Ok(compare(a, b))
            })?;
        }
        Ok(())
    }
}

fn reserve_one<T>(values: &mut Vec<T>, budget: &mut PreparationBudget) -> Result<(), RuntimeError> {
    budget.charge(0, 1)?;
    if values.len() == values.capacity() {
        let extra = values.capacity().max(1);
        budget.records::<T>(extra)?;
        budget.charge(0, values.len())?; // Growth may move all existing records.
        values
            .try_reserve_exact(extra)
            .map_err(|_| RuntimeError::Overflow)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_groups_preserve_tree_key_order_and_per_key_wire_order() {
        let mut expected = BTreeMap::<u32, Vec<u32>>::new();
        let mut actual = Groups::default();
        let mut budget = PreparationBudget::new(PreparationLimits::default());
        for ordinal in 0..513 {
            let key = (ordinal * 73) % 31;
            expected.entry(key).or_default().push(ordinal);
            actual.push(key, ordinal, &mut budget).unwrap();
        }
        actual.finish(&mut budget).unwrap();
        assert!(actual.pending.is_empty());
        assert_eq!(actual.pending.capacity(), 0);
        assert_eq!(
            actual.keys.iter().map(|entry| entry.0).collect::<Vec<_>>(),
            expected.keys().copied().collect::<Vec<_>>()
        );
        for (key, values) in expected {
            assert_eq!(actual.get(&key), Some(values.as_slice()));
        }
        assert!(actual.get(&u32::MAX).is_none());
    }

    #[test]
    fn reversed_unique_groups_have_bounded_n_log_n_preparation_work() {
        let count = 8192u32;
        let mut groups = Groups::default();
        let mut budget = PreparationBudget::new(PreparationLimits::default());
        for id in (0..count).rev() {
            groups.push(id, id, &mut budget).unwrap();
        }
        groups.finish(&mut budget).unwrap();
        assert!(budget.usage().work < 12 * count as usize * (count.ilog2() as usize + 1));
        for id in 0..count {
            assert_eq!(groups.get(&id), Some(core::slice::from_ref(&id)));
        }
    }

    #[test]
    fn construction_charges_before_allocating_and_sort_stops_on_exhaustion() {
        let mut groups = Groups::<u32, u32>::default();
        let mut budget = PreparationBudget::new(PreparationLimits {
            max_preparation_bytes: 0,
            ..Default::default()
        });
        assert_eq!(
            groups.push(1, 1, &mut budget),
            Err(RuntimeError::PreparationLimit)
        );
        assert_eq!(groups.pending.capacity(), 0);
        let mut budget = PreparationBudget::new(PreparationLimits::default());
        groups.push(2, 20, &mut budget).unwrap();
        groups.push(1, 10, &mut budget).unwrap();
        let mut exhausted = PreparationBudget::new(PreparationLimits {
            max_preparation_work: 0,
            ..Default::default()
        });
        assert_eq!(
            groups.finish(&mut exhausted),
            Err(RuntimeError::PreparationLimit)
        );
        assert_eq!(groups.pending[0].0, 2);
        assert!(groups.keys.is_empty());
        assert!(groups.values.is_empty());
    }
}
