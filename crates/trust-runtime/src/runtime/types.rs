//! Runtime shared types.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::value::{Duration, Value};

#[derive(Debug, Clone)]
pub(super) struct ReadyTask {
    pub index: usize,
    pub due_at: Duration,
}

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
