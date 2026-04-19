//! Runtime restart and retention.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::error;
use crate::task::TaskState;
use crate::value::{Duration, Value};

use super::core::Runtime;
use super::types::{GlobalInitValue, RestartMode, RetainPolicy, RetainSnapshot};

impl Runtime {
    /// Restart the runtime in the given mode (cold or warm).
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), error::RuntimeError> {
        let globals = self.globals.clone();
        let mut retained = IndexMap::new();
        let mut retained_program_vars = Vec::new();
        if matches!(mode, RestartMode::Warm) {
            for (name, meta) in &globals {
                if retain_on_warm(meta.retain) {
                    if let Some(value) = self.storage.get_global(name.as_ref()) {
                        retained.insert(name.clone(), value.clone());
                    }
                }
            }
            for program in self.programs.values() {
                let Some(Value::Instance(id)) = self.storage.get_global(program.name.as_ref())
                else {
                    continue;
                };
                for var in &program.vars {
                    if !retain_on_warm(var.retain) {
                        continue;
                    }
                    let Some(value) = self.storage.get_instance_var(*id, var.name.as_ref()) else {
                        continue;
                    };
                    if value_is_retainable(value) {
                        retained_program_vars.push((
                            program.name.clone(),
                            var.name.clone(),
                            value.clone(),
                        ));
                    }
                }
            }
        }

        // Compiled runtime metadata keeps ValueRef targets for program/FB instances.
        // Preserve those references across both cold and warm restarts by recreating
        // instances from a stable id sequence before execution resumes.
        self.storage.reset_runtime_values(true);

        for (name, meta) in globals {
            let keep = matches!(mode, RestartMode::Warm) && retain_on_warm(meta.retain);
            if keep {
                if let Some(value) = retained.get(&name) {
                    self.storage.set_global(name.clone(), value.clone());
                    continue;
                }
            }
            match meta.init {
                GlobalInitValue::Value(value) => {
                    self.storage.set_global(name.clone(), value);
                }
                GlobalInitValue::FunctionBlock { type_name } => {
                    let key = SmolStr::new(type_name.to_ascii_uppercase());
                    let fb = self
                        .function_blocks
                        .get(&key)
                        .ok_or(error::RuntimeError::UndefinedFunctionBlock(type_name))?;
                    let instance_id = crate::instance::create_fb_instance(
                        &mut self.storage,
                        &self.registry,
                        &self.profile,
                        &self.classes,
                        &self.function_blocks,
                        &self.functions,
                        &self.stdlib,
                        fb,
                    )?;
                    self.storage
                        .set_global(name.clone(), Value::Instance(instance_id));
                }
                GlobalInitValue::Class { type_name } => {
                    let key = SmolStr::new(type_name.to_ascii_uppercase());
                    let class_def = self
                        .classes
                        .get(&key)
                        .ok_or(error::RuntimeError::TypeMismatch)?;
                    let instance_id = crate::instance::create_class_instance(
                        &mut self.storage,
                        &self.registry,
                        &self.profile,
                        &self.classes,
                        &self.function_blocks,
                        &self.functions,
                        &self.stdlib,
                        class_def,
                    )?;
                    self.storage
                        .set_global(name.clone(), Value::Instance(instance_id));
                }
            }
        }

        let programs = self.programs.values().cloned().collect::<Vec<_>>();
        for program in programs {
            let instance_id = crate::instance::create_program_instance(
                &mut self.storage,
                &self.registry,
                &self.profile,
                &self.classes,
                &self.function_blocks,
                &self.functions,
                &self.stdlib,
                &program,
            )?;
            self.storage
                .set_global(program.name.clone(), Value::Instance(instance_id));
        }
        for (program_name, var_name, value) in retained_program_vars {
            let Some(Value::Instance(id)) = self.storage.get_global(program_name.as_ref()) else {
                continue;
            };
            self.storage.set_instance_var(*id, var_name, value);
        }

        self.current_time = Duration::ZERO;
        for state in self.task_state.values_mut() {
            *state = TaskState::new(self.current_time);
        }
        self.faults.clear();
        self.cycle_counter = 0;
        Ok(())
    }

    /// Capture retained global values that can be preserved across reloads.
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
                snapshot.values.insert(name.clone(), value.clone());
            }
        }
        snapshot
    }

    /// Apply a retained snapshot to the current runtime.
    pub fn apply_retain_snapshot(&mut self, snapshot: &RetainSnapshot) {
        for (name, value) in &snapshot.values {
            let Some(meta) = self.globals.get(name) else {
                continue;
            };
            if retain_on_warm(meta.retain) && value_is_retainable(value) {
                self.storage.set_global(name.clone(), value.clone());
            }
        }
    }
}

fn retain_on_warm(policy: RetainPolicy) -> bool {
    matches!(policy, RetainPolicy::Retain | RetainPolicy::Persistent)
}

fn value_is_retainable(value: &Value) -> bool {
    match value {
        Value::Array(array) => array.elements.iter().all(value_is_retainable),
        Value::Struct(value) => value.fields.values().all(value_is_retainable),
        Value::Reference(_) | Value::Instance(_) => false,
        _ => true,
    }
}
