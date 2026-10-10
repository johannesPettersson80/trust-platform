//! Synchronization policy for optional storage lookup caches.
//! Hosted callers retain poisoning/locking behavior; a portable single owner uses
//! checked interior borrows. No synchronization primitive participates in PLC values.

#[cfg(feature = "std")]
pub(super) use std::sync::RwLock as StorageCache;

#[cfg(feature = "std")]
pub(super) fn recover_read_lock<T>(
    result: std::sync::LockResult<std::sync::RwLockReadGuard<'_, T>>,
) -> Option<std::sync::RwLockReadGuard<'_, T>> {
    Some(result.unwrap_or_else(std::sync::PoisonError::into_inner))
}

#[cfg(not(feature = "std"))]
use core::cell::{BorrowError, BorrowMutError, Ref, RefCell, RefMut};

#[cfg(not(feature = "std"))]
#[derive(Debug, Default)]
pub(super) struct StorageCache<T>(RefCell<T>);

#[cfg(not(feature = "std"))]
impl<T> StorageCache<T> {
    pub(super) fn new(value: T) -> Self {
        Self(RefCell::new(value))
    }
    pub(super) fn read(&self) -> Result<Ref<'_, T>, BorrowError> {
        self.0.try_borrow()
    }
    pub(super) fn write(&self) -> Result<RefMut<'_, T>, BorrowMutError> {
        self.0.try_borrow_mut()
    }
}

#[cfg(not(feature = "std"))]
pub(super) fn recover_read_lock<T>(result: Result<Ref<'_, T>, BorrowError>) -> Option<Ref<'_, T>> {
    result.ok()
}

/// Exclusive storage mutation cannot overlap a live cache borrow.
#[cfg(feature = "std")]
pub(super) fn exclusive<T>(cache: &mut StorageCache<T>) -> &mut T {
    cache
        .get_mut()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
#[cfg(not(feature = "std"))]
pub(super) fn exclusive<T>(cache: &mut StorageCache<T>) -> &mut T {
    cache.0.get_mut()
}

#[cfg(all(test, not(feature = "std")))]
mod tests {
    use crate::memory::VariableStorage;
    use crate::value::Value;

    #[test]
    fn contended_optional_caches_fall_back_to_storage_and_clone_empty() {
        let mut storage = VariableStorage::new();
        let instance = storage.create_instance("FB");
        storage.set_instance_var(instance, "field", Value::Int(7));
        let direct = storage.instance_field_offsets.write().unwrap();
        let recursive = storage
            .recursive_instance_field_resolutions
            .write()
            .unwrap();
        let declared = storage.declared_instance_field_offsets.write().unwrap();
        assert_eq!(
            storage.ref_for_instance(instance, "field").unwrap().offset,
            0
        );
        assert_eq!(
            storage
                .ref_for_instance_recursive(instance, "field")
                .unwrap()
                .offset,
            0
        );
        assert_eq!(
            storage.declared_instance_field_offset(instance, "field"),
            Some(0)
        );
        assert!(storage.ref_for_instance(instance, "missing").is_none());
        let clone = storage.clone();
        assert_eq!(
            clone.get_instance_var(instance, "field"),
            Some(&Value::Int(7))
        );
        drop((direct, recursive, declared));
        assert_eq!(
            storage.ref_for_instance(instance, "field").unwrap().offset,
            0
        );
    }
}
