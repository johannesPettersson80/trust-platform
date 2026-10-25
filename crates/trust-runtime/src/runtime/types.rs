//! Runtime shared types.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::value::Value;

/// Snapshot of retained values for restart, persistence, and hot reload.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RetainSnapshot {
    pub(crate) inner: trust_runtime_core::retain::RetainSnapshot,
}

impl RetainSnapshot {
    pub(crate) fn from_values(values: IndexMap<SmolStr, Value>) -> Self {
        Self {
            inner: trust_runtime_core::retain::RetainSnapshot::from_values(values),
        }
    }

    pub fn insert(&mut self, name: impl Into<SmolStr>, value: Value) {
        self.inner.insert(name, value);
    }

    #[must_use]
    pub fn values(&self) -> &IndexMap<SmolStr, Value> {
        self.inner.values()
    }
}
