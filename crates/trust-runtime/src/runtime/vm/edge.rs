use smol_str::SmolStr;
use trust_hir::symbols::EdgeQualifier;

use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::program_model::edge_phase_storage_name;
use crate::value::Value;

use super::VmModule;

pub(super) struct EdgeInputTransaction {
    instance_id: InstanceId,
    restores: Vec<(SmolStr, Value)>,
}

impl EdgeInputTransaction {
    pub(super) fn begin(
        runtime: &mut crate::Runtime,
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
        let Some(inputs) = runtime.edge_inputs.get(owner).cloned() else {
            return Ok(None);
        };

        let mut restores = Vec::with_capacity(inputs.len());
        for input in inputs {
            let Some(raw) = runtime
                .storage
                .get_instance_var(instance_id, input.name.as_str())
                .cloned()
            else {
                continue;
            };
            let Value::Bool(raw) = raw else {
                return Err(RuntimeError::TypeMismatch);
            };
            let phase_name = edge_phase_storage_name(owner, input.name.as_str());
            let previous = runtime
                .storage
                .get_instance_var(instance_id, phase_name.as_str())
                .and_then(|value| match value {
                    Value::Bool(value) => Some(*value),
                    _ => None,
                })
                .unwrap_or(match input.qualifier {
                    EdgeQualifier::Rising => false,
                    EdgeQualifier::Falling => true,
                });
            let pulse = match input.qualifier {
                EdgeQualifier::Rising => raw && !previous,
                EdgeQualifier::Falling => !raw && previous,
            };
            runtime
                .storage
                .set_instance_var(instance_id, phase_name, Value::Bool(raw));
            runtime
                .storage
                .set_instance_var(instance_id, input.name.clone(), Value::Bool(pulse));
            restores.push((input.name, Value::Bool(raw)));
        }

        Ok(Some(Self {
            instance_id,
            restores,
        }))
    }

    pub(super) fn restore(self, runtime: &mut crate::Runtime) {
        for (name, value) in self.restores {
            runtime
                .storage
                .set_instance_var(self.instance_id, name, value);
        }
    }
}
