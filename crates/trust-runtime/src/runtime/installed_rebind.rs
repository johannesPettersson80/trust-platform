//! Rebinding of the references the hosted controller holds after a committed restart.
//!
//! A warm or cold restart constructs the next installation generation, so every reference
//! the host took before it names no state any more (specification 34 §11.3, MEM-07,
//! UPDATE-04). The host binds each of them again to the same declaration of the restarted
//! application: engineering aliases, task function blocks, the I/O binding records and the
//! ADS and OPC UA client points. A replacement binds them against its candidate instead,
//! before it commits.
use super::installed::Runtime;
use crate::{error::RuntimeError, memory::AccessMap, value::ValueRef, watchdog::FaultDecision};
use std::sync::Arc;

/// The host references of the restarted generation, all resolved before any is replaced.
struct ReboundReferences {
    access: AccessMap,
    task_instances: Vec<Vec<ValueRef>>,
    io: Vec<ValueRef>,
    ads: Vec<Vec<crate::ads::AdsBinding>>,
    opcua: Vec<Vec<crate::opcua::OpcUaClientBinding>>,
}

impl Runtime {
    /// Bind the host's references to the generation a committed restart installed. The
    /// restarted application is the same admitted module, so each declaration resolves
    /// again. If one does not, the controller faults under its fault policy rather than
    /// run with references that name nothing (specification 34 §11.3, UPDATE-07).
    pub(super) fn rebind_restarted_generation(&mut self) -> Result<(), RuntimeError> {
        match self.resolve_rebound_references() {
            Ok(rebound) => {
                self.commit_rebound_references(rebound);
                Ok(())
            }
            Err(error) => {
                let decision = FaultDecision::from_fault_policy(self.fault_policy);
                Err(self.apply_fault(error, decision))
            }
        }
    }

    fn resolve_rebound_references(&mut self) -> Result<ReboundReferences, RuntimeError> {
        let mut access = AccessMap::default();
        let aliases = self
            .metadata
            .access_map()
            .bindings()
            .map(|binding| (binding.name.clone(), binding.partial, binding.writable))
            .collect::<Vec<_>>();
        for (name, partial, writable) in aliases {
            let reference = self.state.resolve_access_reference(&name)?;
            access.bind_with_mode(name, reference, partial, writable);
        }
        let prepared = self.state.shared_prepared();
        let resource = prepared
            .resources()
            .get(self.state.resource_index())
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let mut task_instances = Vec::with_capacity(resource.tasks.len());
        for task in &resource.tasks {
            let mut instances = Vec::with_capacity(task.fb_ref_idx.len());
            for &reference in &task.fb_ref_idx {
                instances.push(self.state.resolve_persistent_reference(reference)?);
            }
            task_instances.push(instances);
        }
        let mut io = Vec::with_capacity(prepared.io_bindings().len());
        for binding in prepared.io_bindings() {
            io.push(self.state.resolve_persistent_reference(binding.ref_idx)?);
        }
        let ads = self.ads.prepare_rebind(self)?;
        let opcua = self.opcua_client.prepare_rebind(self)?;
        Ok(ReboundReferences {
            access,
            task_instances,
            io,
            ads,
            opcua,
        })
    }

    fn commit_rebound_references(&mut self, rebound: ReboundReferences) {
        let mut metadata = (*self.metadata).clone();
        metadata.access = rebound.access;
        for (task, instances) in metadata.tasks.iter_mut().zip(&rebound.task_instances) {
            task.fb_instances.clone_from(instances);
        }
        for (task, instances) in self.tasks.iter_mut().zip(rebound.task_instances) {
            task.fb_instances = instances;
        }
        let metadata = Arc::new(metadata);
        self.state
            .services_mut()
            .set_metadata(Arc::clone(&metadata));
        self.metadata = metadata;
        self.io.interface_mut().rebind_references(rebound.io);
        self.ads.commit_rebind(rebound.ads);
        self.opcua_client.commit_rebind(rebound.opcua);
    }
}
