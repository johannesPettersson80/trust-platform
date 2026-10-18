use super::call::context::CallContext;
use alloc::vec::Vec;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::InstanceId;
use crate::value::Value;

use super::module::VmModule;

/// Original edge inputs saved while a call observes edge-transformed values.
#[derive(Debug)]
pub struct EdgeInputTransaction {
    instance_id: InstanceId,
    restores: Vec<(SmolStr, Value)>,
}

impl EdgeInputTransaction {
    /// Apply edge-input transitions while retaining values for restoration. The edge-input
    /// plan is read from the prepared index into the composition's reusable list, and the
    /// saved originals go into its reusable restore list, so a scan allocates neither
    /// (spec 34 MEM-09; census A5).
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
        let inputs = runtime.take_edge_inputs_for_pou(module, pou_id, owner);
        if inputs.is_empty() {
            runtime.recycle_edge_inputs(inputs);
            return Ok(None);
        }
        // Edge qualification mutates both the visible input and hidden phase.
        runtime.check_builtin_call(instance_id)?;

        let mut restores = runtime.take_edge_restores();
        restores.clear();
        let mut mismatched = false;
        for input in &inputs {
            let storage = runtime.storage();
            let Some(raw) = storage
                .read_instance_var(instance_id, input.name.as_str())
                .and_then(|view| storage.borrow(&view))
                .cloned()
            else {
                continue;
            };
            let Value::Bool(raw) = raw else {
                mismatched = true;
                break;
            };
            let previous = storage
                .read_instance_var(instance_id, input.phase_name.as_str())
                .and_then(|view| storage.borrow(&view))
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
            runtime.storage_mut().write_instance_var(
                instance_id,
                input.phase_name.as_str(),
                Value::Bool(raw),
            );
            runtime.storage_mut().write_instance_var(
                instance_id,
                input.name.as_str(),
                Value::Bool(pulse),
            );
            restores.push((input.name.clone(), Value::Bool(raw)));
        }
        runtime.recycle_edge_inputs(inputs);
        if mismatched {
            return Err(RuntimeError::TypeMismatch);
        }

        Ok(Some(Self {
            instance_id,
            restores,
        }))
    }

    /// Restore original input values after the call while keeping updated edge phases. The
    /// reusable restore list goes back to the composition.
    pub fn restore(self, runtime: &mut impl CallContext) {
        let Self {
            instance_id,
            mut restores,
        } = self;
        for (name, value) in restores.drain(..) {
            runtime
                .storage_mut()
                .write_instance_var(instance_id, name.as_str(), value);
        }
        runtime.recycle_edge_restores(restores);
    }
}
