//! Private historical fixture behavior; excluded from every product build.
//! Runtime restart and retention.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::error;
use crate::memory::VariableStorage;
use crate::program_model::edge_phase_storage_name;
use crate::task::TaskState;
use crate::value::Value;

use super::core::Runtime;
use super::legacy_retain_snapshot::{retain_on_warm, value_is_retainable};
use super::types::{GlobalInitValue, RestartMode};

pub(super) struct PreparedRestart {
    pub(super) storage: VariableStorage,
}

impl Runtime {
    /// Restart the runtime in the given mode (cold or warm).
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), error::RuntimeError> {
        let prepared = self.prepare_restart(mode)?;
        self.commit_restart(prepared);
        Ok(())
    }

    pub(super) fn prepare_restart(
        &self,
        mode: RestartMode,
    ) -> Result<PreparedRestart, error::RuntimeError> {
        let globals = self.globals.clone();
        let mut retained = IndexMap::new();
        let mut retained_program_vars = Vec::new();
        if matches!(mode, RestartMode::Warm) {
            for (name, meta) in &globals {
                if retain_on_warm(meta.retain)
                    && let Some(value) = self.storage.get_global(name.as_ref())
                {
                    retained.insert(name.clone(), value.clone());
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
                if let Some(inputs) = self.edge_inputs.get(&program.name) {
                    for input in inputs {
                        let Some(var) = program
                            .vars
                            .iter()
                            .find(|var| var.name.eq_ignore_ascii_case(input.name.as_str()))
                        else {
                            continue;
                        };
                        if !retain_on_warm(var.retain) {
                            continue;
                        }
                        let phase_name =
                            edge_phase_storage_name(program.name.as_str(), input.name.as_str());
                        let Some(value) = self
                            .storage
                            .get_instance_var(*id, phase_name.as_str())
                            .cloned()
                        else {
                            continue;
                        };
                        if value_is_retainable(&value) {
                            retained_program_vars.push((program.name.clone(), phase_name, value));
                        }
                    }
                }
            }
        }

        // Compiled runtime metadata keeps ValueRef targets for program/FB instances.
        // Preserve those references across both cold and warm restarts by recreating
        // instances from a stable id sequence before execution resumes.
        let mut storage = self.storage.clone();
        storage.reset_runtime_values(true);

        for (name, meta) in globals {
            let keep = matches!(mode, RestartMode::Warm) && retain_on_warm(meta.retain);
            if keep && let Some(value) = retained.get(&name) {
                storage.set_global(name.clone(), value.clone());
                continue;
            }
            match meta.init {
                GlobalInitValue::Value(value) => {
                    storage.set_global(name.clone(), value);
                }
                GlobalInitValue::FunctionBlock { type_name } => {
                    let key = SmolStr::new(type_name.to_ascii_uppercase());
                    let fb = self
                        .function_blocks
                        .get(&key)
                        .ok_or(error::RuntimeError::UndefinedFunctionBlock(type_name))?;
                    let instance_id = crate::instance::create_fb_instance(
                        &mut storage,
                        &self.registry,
                        &self.profile,
                        &self.classes,
                        &self.function_blocks,
                        &self.functions,
                        &self.stdlib,
                        &self.initializer_catalog,
                        fb,
                    )?;
                    storage.set_global(name.clone(), Value::Instance(instance_id));
                }
                GlobalInitValue::Class { type_name } => {
                    let key = SmolStr::new(type_name.to_ascii_uppercase());
                    let class_def = self
                        .classes
                        .get(&key)
                        .ok_or(error::RuntimeError::TypeMismatch)?;
                    let instance_id = crate::instance::create_class_instance(
                        &mut storage,
                        &self.registry,
                        &self.profile,
                        &self.classes,
                        &self.function_blocks,
                        &self.functions,
                        &self.stdlib,
                        &self.initializer_catalog,
                        class_def,
                    )?;
                    storage.set_global(name.clone(), Value::Instance(instance_id));
                }
            }
        }

        let programs = self.programs.values().cloned().collect::<Vec<_>>();
        for program in programs {
            let instance_id = crate::instance::create_program_instance(
                &mut storage,
                &self.registry,
                &self.profile,
                &self.classes,
                &self.function_blocks,
                &self.functions,
                &self.stdlib,
                &self.initializer_catalog,
                &program,
            )?;
            storage.set_global(program.name.clone(), Value::Instance(instance_id));
        }
        for (program_name, var_name, value) in retained_program_vars {
            let Some(Value::Instance(id)) = storage.get_global(program_name.as_ref()) else {
                continue;
            };
            storage.set_instance_var(*id, var_name, value);
        }

        Ok(PreparedRestart { storage })
    }

    pub(super) fn commit_restart(&mut self, prepared: PreparedRestart) {
        if let Some(debug) = &self.debug {
            debug.clear_runtime_mutations();
        }
        self.storage = prepared.storage;
        self.openot_telemetry.reset_scan_state();
        for state in self.task_state.values_mut() {
            *state = TaskState::new(self.current_time);
        }
        self.faults.clear();
        self.cycle_counter = 0;
    }
}
