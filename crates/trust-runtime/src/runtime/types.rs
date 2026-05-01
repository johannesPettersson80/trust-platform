//! Runtime shared types.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::value::Value;

pub(super) use trust_runtime_core::cycle::ReadyTask;
pub(super) use trust_runtime_core::retain::{RestartMode, RetainPolicy};

/// Snapshot of retained global values for hot reload.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RetainSnapshot {
    pub(crate) values: IndexMap<SmolStr, Value>,
}

impl RetainSnapshot {
    pub fn insert(&mut self, name: impl Into<SmolStr>, value: Value) {
        self.values.insert(name.into(), value);
    }

    #[must_use]
    pub fn values(&self) -> &IndexMap<SmolStr, Value> {
        &self.values
    }
}

#[derive(Debug, Clone)]
pub(crate) enum GlobalInitValue {
    Value(Value),
    FunctionBlock { type_name: SmolStr },
    Class { type_name: SmolStr },
}

#[derive(Debug, Clone)]
pub(crate) struct GlobalVarMeta {
    // Preserved for upcoming diagnostics/profiling surfaces that report declared type metadata.
    #[allow(dead_code)]
    pub type_id: trust_hir::TypeId,
    pub retain: RetainPolicy,
    pub init: GlobalInitValue,
}
