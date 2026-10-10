use super::call::context::CallContext;
use alloc::vec::Vec;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::value::Value;

use super::module::VmModule;

/// Original edge inputs saved while a call observes edge-transformed values.
pub struct EdgeInputTransaction {
    instance_id: InstanceId,
    restores: Vec<(SmolStr, Value)>,
}

impl EdgeInputTransaction {
    /// Apply edge-input transitions while retaining values for restoration.
    pub fn begin(
        runtime: &mut impl CallContext,
        module: &VmModule,
        pou_id: u32,
        instance_id: Option<InstanceId>,
    ) -> Result<Option<Self>, RuntimeError> {
        let Some(instance_id) = instance_id else {
            return Ok(None);
        };
        let Some(owner) = module.pou_name(pou_id) else {
            return Ok(None);
        };
        let inputs = runtime.edge_inputs(owner);
        if inputs.is_empty() {
            return Ok(None);
        }
        // Edge qualification mutates both the visible input and hidden phase.
        runtime.check_builtin_call(instance_id)?;

        let mut restores = Vec::with_capacity(inputs.len());
        for input in inputs {
            let Some(raw) = runtime
                .storage()
                .get_instance_var(instance_id, input.name.as_str())
                .cloned()
            else {
                continue;
            };
            let Value::Bool(raw) = raw else {
                return Err(RuntimeError::TypeMismatch);
            };
            let phase_name = input.phase_name;
            let previous = runtime
                .storage()
                .get_instance_var(instance_id, phase_name.as_str())
                .and_then(|value| match value {
                    Value::Bool(value) => Some(*value),
                    _ => None,
                })
                .unwrap_or(!input.rising);
            let pulse = if input.rising {
                raw && !previous
            } else {
                !raw && previous
            };
            runtime
                .storage_mut()
                .set_instance_var(instance_id, phase_name, Value::Bool(raw));
            runtime.storage_mut().set_instance_var(
                instance_id,
                input.name.clone(),
                Value::Bool(pulse),
            );
            restores.push((input.name, Value::Bool(raw)));
        }

        Ok(Some(Self {
            instance_id,
            restores,
        }))
    }

    /// Restore original input values after the call while keeping updated edge phases.
    pub fn restore(self, runtime: &mut impl CallContext) {
        for (name, value) in self.restores {
            runtime
                .storage_mut()
                .set_instance_var(self.instance_id, name, value);
        }
    }
}
