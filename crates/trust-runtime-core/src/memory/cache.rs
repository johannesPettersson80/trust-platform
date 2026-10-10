//! Hosted optional lookup caches preserve poisoning and locking behavior.
//! Portable storage uses the variable map's existing hash index directly.
pub(super) use std::sync::RwLock as StorageCache;

pub(super) fn recover_read_lock<T>(
    result: std::sync::LockResult<std::sync::RwLockReadGuard<'_, T>>,
) -> Option<std::sync::RwLockReadGuard<'_, T>> {
    Some(result.unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// Exclusive storage mutation cannot overlap a live cache borrow.
pub(super) fn exclusive<T>(cache: &mut StorageCache<T>) -> &mut T {
    cache
        .get_mut()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
