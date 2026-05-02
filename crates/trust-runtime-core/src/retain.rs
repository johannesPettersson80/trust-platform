//! Portable retain and restart policy records.

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::value::Value;

/// Retentive behavior for variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RetainPolicy {
    /// Retentive across warm restarts.
    Retain,
    /// Always reinitialized on restart.
    NonRetain,
    /// No explicit qualifier; treat as non-retentive on warm restart.
    #[default]
    Unspecified,
    /// Persistent across warm restarts.
    Persistent,
}

/// Restart mode for a resource/configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartMode {
    /// Cold restart: reinitialize all variables.
    Cold,
    /// Warm restart: retain RETAIN/PERSISTENT variables.
    Warm,
}

/// Snapshot of retained global values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RetainSnapshot {
    values: IndexMap<SmolStr, Value>,
}

impl RetainSnapshot {
    /// Create a snapshot from already-collected retained values.
    #[must_use]
    pub fn from_values(values: IndexMap<SmolStr, Value>) -> Self {
        Self { values }
    }

    /// Insert or replace a retained value.
    pub fn insert(&mut self, name: impl Into<SmolStr>, value: Value) {
        self.values.insert(name.into(), value);
    }

    /// Return the retained values in snapshot order.
    #[must_use]
    pub fn values(&self) -> &IndexMap<SmolStr, Value> {
        &self.values
    }

    /// Consume the snapshot and return its retained values.
    #[must_use]
    pub fn into_values(self) -> IndexMap<SmolStr, Value> {
        self.values
    }
}

#[cfg(test)]
mod tests {
    use super::{RestartMode, RetainPolicy, RetainSnapshot};
    use crate::value::Value;
    use alloc::vec::Vec;

    #[test]
    fn retain_policy_preserves_default_and_warm_restart_contract() {
        assert_eq!(RetainPolicy::default(), RetainPolicy::Unspecified);
        assert_ne!(RetainPolicy::Retain, RetainPolicy::NonRetain);
        assert_ne!(RetainPolicy::Persistent, RetainPolicy::Unspecified);
        assert_ne!(RestartMode::Cold, RestartMode::Warm);
    }

    #[test]
    fn retain_snapshot_preserves_insert_order_and_values() {
        let mut snapshot = RetainSnapshot::default();
        snapshot.insert("FIRST", Value::DInt(1));
        snapshot.insert("SECOND", Value::Bool(true));

        let entries = snapshot.values().iter().collect::<Vec<_>>();
        assert_eq!(entries[0].0.as_str(), "FIRST");
        assert_eq!(entries[0].1, &Value::DInt(1));
        assert_eq!(entries[1].0.as_str(), "SECOND");
        assert_eq!(entries[1].1, &Value::Bool(true));
    }
}
