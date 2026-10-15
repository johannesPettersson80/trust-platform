//! Symbolic force and release for setExpression: the variable force a named target addresses,
//! and putting back the force a target had before when a forced write fails.

use trust_runtime::debug::ForcedVarTarget;
use trust_runtime::memory::VariableStorage;
use trust_runtime::program_model::LValue;
use trust_runtime::value::Value as RuntimeValue;

use super::super::DebugAdapter;

const UNSUPPORTED_TARGET: &str =
    "force/release is only supported for globals, retains, or instance fields";

#[derive(Clone, Copy)]
pub(super) enum SymbolicForceDirective {
    Force,
    Release,
}

impl DebugAdapter {
    /// The force that the destination of a symbolic force names had before the request. A failed
    /// write puts it back, so a previous force on the same target survives.
    pub(super) fn symbolic_force_previous(
        &self,
        storage: &VariableStorage,
        target: &LValue,
    ) -> Option<RuntimeValue> {
        let key = match target {
            LValue::Name(name) => {
                if storage.get_global(name.as_ref()).is_some() {
                    ForcedVarTarget::Global(name.clone())
                } else if storage.get_retain(name.as_ref()).is_some() {
                    ForcedVarTarget::Retain(name.clone())
                } else {
                    return None;
                }
            }
            LValue::Field { target, field } => {
                let LValue::Name(name) = target.as_ref() else {
                    return None;
                };
                let Some(RuntimeValue::Instance(id)) = storage.get_global(name.as_ref()) else {
                    return None;
                };
                ForcedVarTarget::Instance(*id, field.clone())
            }
            _ => return None,
        };
        self.session.debug_control().forced_var_value(&key)
    }

    /// Force or release the variable a symbolic target names, in the debug control force set.
    pub(super) fn apply_symbolic_force_directive(
        &self,
        storage: &VariableStorage,
        target: &LValue,
        directive: SymbolicForceDirective,
        value: Option<RuntimeValue>,
    ) -> Result<(), String> {
        match target {
            LValue::Name(name) => {
                if storage.get_global(name.as_ref()).is_some() {
                    match directive {
                        SymbolicForceDirective::Force => self
                            .session
                            .debug_control()
                            .try_force_global(name.clone(), value.ok_or("missing force value")?)
                            .map_err(|error| error.to_string())?,
                        SymbolicForceDirective::Release => {
                            self.session.debug_control().release_global(name.as_ref())
                        }
                    }
                    return Ok(());
                }
                if storage.get_retain(name.as_ref()).is_some() {
                    match directive {
                        SymbolicForceDirective::Force => self
                            .session
                            .debug_control()
                            .try_force_retain(name.clone(), value.ok_or("missing force value")?)
                            .map_err(|error| error.to_string())?,
                        SymbolicForceDirective::Release => {
                            self.session.debug_control().release_retain(name.as_ref())
                        }
                    }
                    return Ok(());
                }
                Err(UNSUPPORTED_TARGET.to_string())
            }
            LValue::Field { target, field } => {
                let LValue::Name(name) = target.as_ref() else {
                    return Err(UNSUPPORTED_TARGET.to_string());
                };
                let instance_id = match storage.get_global(name.as_ref()) {
                    Some(RuntimeValue::Instance(id)) => *id,
                    _ => return Err(UNSUPPORTED_TARGET.to_string()),
                };
                match directive {
                    SymbolicForceDirective::Force => self
                        .session
                        .debug_control()
                        .try_force_instance(
                            instance_id,
                            field.clone(),
                            value.ok_or("missing force value")?,
                        )
                        .map_err(|error| error.to_string())?,
                    SymbolicForceDirective::Release => self
                        .session
                        .debug_control()
                        .release_instance(instance_id, field.as_ref()),
                }
                Ok(())
            }
            _ => Err(UNSUPPORTED_TARGET.to_string()),
        }
    }

    /// Put back the force `target` had before a failed forced write: `previous` is re-forced, or
    /// the force is released when the target had none. Re-forcing a target is never refused.
    pub(super) fn restore_symbolic_force(
        &self,
        storage: &VariableStorage,
        target: &LValue,
        previous: Option<RuntimeValue>,
    ) {
        let _ = match previous {
            Some(value) => self.apply_symbolic_force_directive(
                storage,
                target,
                SymbolicForceDirective::Force,
                Some(value),
            ),
            None => self.apply_symbolic_force_directive(
                storage,
                target,
                SymbolicForceDirective::Release,
                None,
            ),
        };
    }
}
