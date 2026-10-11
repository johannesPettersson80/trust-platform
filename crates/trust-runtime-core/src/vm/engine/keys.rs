//! Compact internal keys preserve admitted declaration and full instance identity.
use super::*;

// Source-free admission enforces this cap before EngineState exists. Checking at
// insertion also protects internal callers; growing the format cap must revisit
// this representation rather than silently truncate an identity.
const _: () = assert!(crate::bytecode::BYTECODE_MAX_CONSTRUCTION_RECORDS <= 65_536);
const OWNER_MASK: u64 = (1u64 << 33) - 1;

fn encode_mark(declaration: u32, owner: Option<InstanceId>) -> Result<u64, RuntimeError> {
    let declaration =
        u16::try_from(declaration).map_err(|_| RuntimeError::InvalidExecutionState)?;
    let owner = owner.map_or(0, |owner| u64::from(owner.0) + 1);
    Ok((u64::from(declaration) << 33) | owner)
}

fn decode_mark(key: u64) -> (u32, Option<InstanceId>) {
    let owner = key & OWNER_MASK;
    (
        (key >> 33) as u32,
        (owner != 0).then(|| InstanceId((owner - 1) as u32)),
    )
}

#[derive(Default)]
pub(super) struct LifecycleMarks(super::ordered::Entries<u64, ()>);

impl LifecycleMarks {
    pub(super) fn insertion_demand(
        &self,
        key: (u32, Option<InstanceId>),
    ) -> Result<(usize, usize), RuntimeError> {
        self.0.insertion_demand(&encode_mark(key.0, key.1)?)
    }
    pub(super) fn prepare_insert(
        &mut self,
        key: (u32, Option<InstanceId>),
    ) -> Result<(), RuntimeError> {
        self.0.prepare_insert(&encode_mark(key.0, key.1)?)
    }

    pub(super) fn insert(&mut self, key: (u32, Option<InstanceId>)) -> Result<bool, RuntimeError> {
        Ok(self.0.insert(encode_mark(key.0, key.1)?, ())?.is_none())
    }
    pub(super) fn contains(&self, key: &(u32, Option<InstanceId>)) -> Result<bool, RuntimeError> {
        Ok(self.0.get(&encode_mark(key.0, key.1)?).is_some())
    }
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (u32, Option<InstanceId>)> + '_ {
        self.0.iter().map(|(key, _)| decode_mark(*key))
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(&(u32, Option<InstanceId>)) -> bool) {
        self.0.retain(|key, _| keep(&decode_mark(*key)));
    }
}

pub(super) fn root_index(index: usize) -> Result<u32, RuntimeError> {
    u32::try_from(index).map_err(|_| RuntimeError::InvalidExecutionState)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_key_is_injective_ordered_and_keeps_all_instance_bits() {
        let owners = [
            None,
            Some(InstanceId(0)),
            Some(InstanceId(1)),
            Some(InstanceId(u32::MAX - 1)),
            Some(InstanceId(u32::MAX)),
        ];
        let mut expected = BTreeSet::new();
        let mut marks = LifecycleMarks::default();
        for declaration in [u16::MAX as u32, 1, 0] {
            for owner in owners.into_iter().rev() {
                let key = (declaration, owner);
                assert_eq!(decode_mark(encode_mark(declaration, owner).unwrap()), key);
                assert!(marks.insert(key).unwrap());
                assert!(!marks.insert(key).unwrap());
                assert!(marks.contains(&key).unwrap());
                expected.insert(key);
            }
        }
        assert_eq!(
            marks.iter().collect::<Vec<_>>(),
            expected.into_iter().collect::<Vec<_>>()
        );
        let before = marks.len();
        assert_eq!(
            marks.insert((65_536, None)),
            Err(RuntimeError::InvalidExecutionState)
        );
        assert_eq!(
            marks.contains(&(u32::MAX, Some(InstanceId(0)))),
            Err(RuntimeError::InvalidExecutionState)
        );
        assert_eq!(marks.len(), before);
    }

    #[test]
    fn retiring_an_owner_keeps_global_and_other_generation_marks() {
        let mut marks = LifecycleMarks::default();
        for owner in [None, Some(InstanceId(0)), Some(InstanceId(u32::MAX))] {
            marks.insert((7, owner)).unwrap();
        }
        marks.retain(|(_, owner)| *owner != Some(InstanceId(0)));
        assert!(marks.contains(&(7, None)).unwrap());
        assert!(!marks.contains(&(7, Some(InstanceId(0)))).unwrap());
        assert!(marks.contains(&(7, Some(InstanceId(u32::MAX)))).unwrap());
    }

    #[test]
    fn root_index_conversion_never_truncates() {
        assert_eq!(root_index(0), Ok(0));
        assert_eq!(root_index(u32::MAX as usize), Ok(u32::MAX));
        if let Some(too_large) = (u32::MAX as usize).checked_add(1) {
            assert_eq!(
                root_index(too_large),
                Err(RuntimeError::InvalidExecutionState)
            );
        }
    }
}
