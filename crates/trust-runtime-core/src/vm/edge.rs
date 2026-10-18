use super::call::context::CallContext;
use alloc::vec::Vec;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::{InstanceId, MemoryLocation};
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
            let Some(view) = storage.read_instance_var(instance_id, input.name.as_str()) else {
                continue;
            };
            let Some(Value::Bool(raw)) = view.scalar()? else {
                mismatched = true;
                break;
            };
            let previous = match storage.read_instance_var(instance_id, input.phase_name.as_str()) {
                Some(view) => match view.scalar()? {
                    Some(Value::Bool(value)) => value,
                    _ => !input.rising,
                },
                None => !input.rising,
            };
            let pulse = if input.rising {
                raw && !previous
            } else {
                !raw && previous
            };
            // The members are written by ordinal, which the store reads from the instance's layout
            // (C1-9d1, R4): a name over 23 bytes is not converted to a `SmolStr` on each scan.
            let phase = storage
                .ref_for_instance(instance_id, input.phase_name.as_str())
                .ok_or(RuntimeError::NullReference)?;
            let name = storage
                .ref_for_instance(instance_id, input.name.as_str())
                .ok_or(RuntimeError::NullReference)?;
            runtime.storage_mut().write_slot(
                MemoryLocation::Instance(instance_id),
                phase.offset as usize,
                Value::Bool(raw),
            );
            runtime.storage_mut().write_slot(
                MemoryLocation::Instance(instance_id),
                name.offset as usize,
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
