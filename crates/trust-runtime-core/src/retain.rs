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
    use smol_str::SmolStr;

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

    #[test]
    fn retain_snapshot_insert_appends_distinct_names_and_replaces_in_place() {
        let mut snapshot = RetainSnapshot::default();
        snapshot.insert("Motor", Value::DInt(1));
        snapshot.insert("SIBLING", Value::Bool(true));
        snapshot.insert("Motor", Value::String("replacement".into()));

        let entries = snapshot.values().iter().collect::<Vec<_>>();
        assert_eq!(entries.len(), 2, "replacement must not append a duplicate");
        assert_eq!(entries[0].0.as_str(), "Motor");
        assert_eq!(entries[0].1, &Value::String("replacement".into()));
        assert_eq!(entries[1].0.as_str(), "SIBLING");
        assert_eq!(entries[1].1, &Value::Bool(true));

        snapshot.insert("MOTOR", Value::UDInt(7));
        let entries = snapshot.values().iter().collect::<Vec<_>>();
        assert_eq!(entries.len(), 3, "a distinct resolved key must append");
        assert_eq!(entries[0].0.as_str(), "Motor");
        assert_eq!(entries[1].0.as_str(), "SIBLING");
        assert_eq!(entries[2].0.as_str(), "MOTOR");
        assert_eq!(entries[2].1, &Value::UDInt(7));
    }

    #[test]
    fn retain_snapshot_map_round_trip_preserves_order_keys_and_value_tags() {
        let values: indexmap::IndexMap<SmolStr, Value> = [
            ("Motor".into(), Value::DInt(-7)),
            ("motor".into(), Value::Bool(true)),
            ("Nested".into(), Value::String("exact".into())),
        ]
        .into_iter()
        .collect();

        let snapshot = RetainSnapshot::from_values(values.clone());
        assert_eq!(snapshot.values(), &values);
        assert_eq!(
            snapshot
                .values()
                .keys()
                .map(SmolStr::as_str)
                .collect::<Vec<_>>(),
            ["Motor", "motor", "Nested"]
        );

        let round_trip = snapshot.into_values();
        assert_eq!(round_trip, values);
        assert!(matches!(round_trip.get("Motor"), Some(Value::DInt(-7))));
        assert!(matches!(round_trip.get("motor"), Some(Value::Bool(true))));
        assert!(matches!(
            round_trip.get("Nested"),
            Some(Value::String(value)) if value.as_str() == "exact"
        ));
    }
}
