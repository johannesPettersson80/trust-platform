//! Runtime shared types.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::value::Value;

#[cfg(test)]
pub(super) use trust_runtime_core::cycle::ReadyTask;
#[cfg(test)]
pub(super) use trust_runtime_core::retain::RestartMode;
pub(super) use trust_runtime_core::retain::RetainPolicy;

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

#[derive(Debug, Clone)]
pub(crate) enum GlobalInitValue {
    Value(Value),
    FunctionBlock {
        // Only the historical restart fixture resolves constructors by name.
        // Production serialization uses GlobalVarMeta::type_id and its registry.
        #[cfg(test)]
        type_name: SmolStr,
    },
    Class {
        #[cfg(test)]
        type_name: SmolStr,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct GlobalVarMeta {
    // Preserved for upcoming diagnostics/profiling surfaces that report declared type metadata.
    #[allow(dead_code)]
    pub type_id: trust_hir::TypeId,
    pub retain: RetainPolicy,
    pub init: GlobalInitValue,
}
