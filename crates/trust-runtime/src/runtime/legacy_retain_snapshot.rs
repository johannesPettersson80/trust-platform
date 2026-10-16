//! Private historical fixture capture; excluded from every product build.
//! Retain restore and migration belong to the installed shared engine.

use super::core::Runtime;
use super::types::RetainSnapshot;
use crate::value::Value;
use smol_str::SmolStr;

pub(super) use trust_runtime_core::retain::{retain_on_warm, value_is_retainable};

const PROGRAM_RETAIN_KEY_PREFIX: &str = "@program/";

impl Runtime {
    /// Capture retained values that can be preserved across reloads.
    #[must_use]
    pub fn retain_snapshot(&self) -> RetainSnapshot {
        let mut snapshot = RetainSnapshot::default();
        for (name, meta) in &self.globals {
            if !retain_on_warm(meta.retain) {
                continue;
            }
            let Some(value) = self.storage.get_global(name.as_ref()) else {
                continue;
            };
            if value_is_retainable(value) {
                snapshot.insert(name.clone(), value.clone());
            }
        }
        for program in self.programs.values() {
            let Some(Value::Instance(instance)) = self.storage.get_global(program.name.as_ref())
            else {
                continue;
            };
            for var in &program.vars {
                if !retain_on_warm(var.retain) {
                    continue;
                }
                let Some(value) = self.storage.get_instance_var(*instance, var.name.as_ref())
                else {
                    continue;
                };
                if value_is_retainable(value) {
                    snapshot.insert(program_retain_key(&program.name, &var.name), value.clone());
                }
            }
        }
        snapshot
    }
}

fn program_retain_key(program: &str, variable: &str) -> SmolStr {
    SmolStr::new(format!("{PROGRAM_RETAIN_KEY_PREFIX}{program}/{variable}"))
}
