//! Installed lifecycle transactions delegated to the admitted common engine.
use super::installed::Runtime;
use crate::{error::RuntimeError, value::Duration, RestartMode, RetainSnapshot};

impl Runtime {
    /// Rebuild the same admitted application transactionally. A failed candidate
    /// preserves PLC values, pending debugger mutations and host service state.
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), RuntimeError> {
        self.state.restart(mode)?;
        self.finish_restart();
        Ok(())
    }

    /// Restore a durable checkpoint as part of the warm restart transaction.
    /// Cold restart deliberately does not read the persistence adapter.
    pub fn restart_with_retain_store(&mut self, mode: RestartMode) -> Result<(), RuntimeError> {
        if mode == RestartMode::Cold {
            return self.restart(mode);
        }
        let snapshot = self.retain.load()?;
        let events = self
            .state
            .restart_with_retain_snapshot(mode, &snapshot.inner)?;
        self.finish_restart();
        self.publish_retain_events(events);
        Ok(())
    }

    fn finish_restart(&mut self) {
        if let Some(debug) = self.debug_control() {
            debug.clear_runtime_mutations();
        }
        self.openot_telemetry.reset_scan_state();
        self.cycle_counter = 0;
    }

    /// Apply an entire checkpoint through admitted type/default policy. The
    /// shared owner restores every staged root if any key fails migration.
    pub fn apply_retain_snapshot(&mut self, snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        let events = self.state.apply_retain_snapshot(&snapshot.inner)?;
        self.publish_retain_events(events);
        Ok(())
    }

    pub(super) fn publish_retain_events(
        &self,
        events: Vec<trust_runtime_core::vm::RetainApplyEvent>,
    ) {
        let Some(debug) = self.debug_control() else {
            return;
        };
        for event in events {
            debug.push_runtime_event(match event {
                trust_runtime_core::vm::RetainApplyEvent::Orphan { name } => {
                    crate::debug::RuntimeEvent::RetainOrphanDropped {
                        name,
                        time: self.current_time(),
                    }
                }
                trust_runtime_core::vm::RetainApplyEvent::Migrated {
                    name,
                    before,
                    after,
                } => crate::debug::RuntimeEvent::RetainMigrationApplied {
                    name,
                    detail: super::retain_snapshot::retain_migration_detail(&before, &after),
                    time: self.current_time(),
                },
            });
        }
    }

    /// Capture retained values using the shared declaration and allocation policy.
    pub fn retain_snapshot(&mut self) -> Result<RetainSnapshot, RuntimeError> {
        self.state
            .retain_snapshot()
            .map(|inner| RetainSnapshot { inner })
    }

    /// Replace the persistence adapter without changing the admitted application.
    pub fn set_retain_store(
        &mut self,
        store: Option<Box<dyn crate::retain::RetainStore>>,
        interval: Option<Duration>,
    ) {
        self.retain.configure(store, interval, self.current_time());
    }
    /// Configure the existing synchronous compatibility-profile save interval.
    pub fn set_retain_save_interval(&mut self, interval: Option<Duration>) {
        self.retain.set_save_interval(interval);
    }
    /// Inspect the configured synchronous save interval.
    #[must_use]
    pub fn retain_save_interval(&self) -> Option<Duration> {
        self.retain.save_interval()
    }
    /// Request persistence at the next configured cycle checkpoint.
    pub fn mark_retain_dirty(&mut self) {
        self.retain.mark_dirty();
    }
    /// Load and atomically migrate the configured durable snapshot.
    pub fn load_retain_store(&mut self) -> Result<(), RuntimeError> {
        let snapshot = self.retain.load()?;
        self.apply_retain_snapshot(&snapshot)
    }
    /// Persist a coherent shared-owner snapshot; failed writes do not acknowledge durability.
    pub fn save_retain_store(&mut self) -> Result<(), RuntimeError> {
        let snapshot = self.retain_snapshot()?;
        self.retain.save_snapshot(snapshot, self.current_time())
    }
    /// Preserve the compatibility-profile synchronous checkpoint timing.
    pub fn maybe_save_retain_store(&mut self) -> Result<(), RuntimeError> {
        if self.retain.should_save(self.current_time()) {
            self.save_retain_store()?;
        }
        Ok(())
    }
}

impl Runtime {
    /// Decode, admit and stage replacement code. Context-free legacy 1.1 is not
    /// executable; use the authoring composition's explicit construction bridge.
    pub fn apply_bytecode_bytes(
        &mut self,
        bytes: &[u8],
        resource_name: Option<&str>,
    ) -> Result<(), RuntimeError> {
        let module = crate::bytecode::BytecodeModule::decode(bytes).map_err(RuntimeError::from)?;
        self.apply_bytecode_module(&module, resource_name)
    }

    /// Publish only after shared construction, retain compatibility and every
    /// host binding have succeeded. This is an owner-boundary operation.
    pub fn apply_bytecode_module(
        &mut self,
        module: &crate::bytecode::BytecodeModule,
        resource_name: Option<&str>,
    ) -> Result<(), RuntimeError> {
        self.replace_module(module, resource_name, None, None)
    }

    /// Retain the existing online-change warm restart and durable-store policy.
    pub fn apply_online_change_bytes(
        &mut self,
        bytes: &[u8],
        resource_name: Option<&str>,
    ) -> Result<super::RuntimeMetadata, RuntimeError> {
        let snapshot = self.retain.load()?;
        let module = crate::bytecode::BytecodeModule::decode(bytes).map_err(RuntimeError::from)?;
        self.replace_module(&module, resource_name, None, Some(&snapshot))?;
        Ok(self.metadata_snapshot())
    }

    fn replace_module(
        &mut self,
        module: &crate::bytecode::BytecodeModule,
        resource_name: Option<&str>,
        metadata: Option<std::sync::Arc<super::RuntimeMetadata>>,
        durable: Option<&RetainSnapshot>,
    ) -> Result<(), RuntimeError> {
        use std::sync::Arc;
        use trust_runtime_core::vm::PreparedModule;
        // Preserve the legacy placeholder naming convention by changing the
        // candidate's resource record, never only the live wrapper label.
        let mut named;
        let module = if let Some(name) = resource_name {
            let metadata = module.metadata().map_err(RuntimeError::from)?;
            if metadata.resource(name).is_none()
                && metadata.resources.len() == 1
                && metadata.resources[0].name.eq_ignore_ascii_case("RESOURCE")
            {
                named = module.clone();
                let index = match named.section_mut(crate::bytecode::SectionId::StringTable) {
                    Some(crate::bytecode::SectionData::StringTable(strings)) => {
                        let index = u32::try_from(strings.entries.len())
                            .map_err(|_| RuntimeError::Overflow)?;
                        strings.entries.push(name.into());
                        index
                    }
                    _ => return Err(RuntimeError::InvalidBytecodeMetadata("string table".into())),
                };
                match named.section_mut(crate::bytecode::SectionId::ResourceMeta) {
                    Some(crate::bytecode::SectionData::ResourceMeta(meta)) => {
                        meta.resources[0].name_idx = index
                    }
                    _ => return Err(RuntimeError::InvalidBytecodeMetadata("resource".into())),
                }
                &named
            } else {
                module
            }
        } else {
            module
        };
        let bytes = module.encode().map_err(RuntimeError::from)?;
        let (prepared, artifact) =
            PreparedModule::from_bytes_with_artifact(&bytes, self.prepared().limits())?;
        let prepared = Arc::new(prepared);
        let resource = match resource_name {
            Some(name) => prepared
                .resources()
                .iter()
                .position(|resource| {
                    prepared.strings()[resource.name_idx as usize].eq_ignore_ascii_case(name)
                })
                .ok_or_else(|| {
                    RuntimeError::InvalidBytecodeMetadata(format!("resource '{name}'").into())
                })?,
            None => 0,
        };
        let mut services = super::hosted_services::HostedExecutionServices::new(
            Arc::clone(&prepared),
            None,
            self.debug_control(),
        );
        services.set_register_configuration(self.state.register_configuration());
        services.restore_execution_deadline(self.state.services().deadline_state());
        let (state, events) = self.state.prepare_replacement(
            prepared,
            resource,
            services,
            durable.map(|snapshot| &snapshot.inner),
        )?;
        let mut candidate = Self::from_owned_with_metadata(
            state,
            metadata,
            Some(Arc::new(artifact.into())),
            resource,
        )?;
        self.io.validate_replacement_policy(&mut candidate.state)?;
        let ads = self.ads.prepare_rebind(&candidate)?;
        let opcua = self.opcua_client.prepare_rebind(&candidate)?;
        if let Some(control) = self.debug_control() {
            candidate.set_debug_control(control);
        }
        // All fallible construction/binding work is complete. Preserve transport
        // workers, queues, physical drivers, health sinks and persistence adapter.
        self.ads.commit_rebind(ads);
        self.opcua_client.commit_rebind(opcua);
        self.io.replace_bindings_from(&mut candidate.io);
        self.state = candidate.state;
        self.metadata = candidate.metadata;
        self.artifact = candidate.artifact;
        self.original_artifact = candidate.original_artifact;
        self.resource_name = candidate.resource_name;
        self.tasks = candidate.tasks;
        self.finish_restart();
        if let Some(control) = self.debug_control() {
            control.invalidate_schema_observations();
        }
        self.publish_retain_events(events);
        Ok(())
    }
}

impl Runtime {
    /// Resource metadata is admitted without restarting PLC values.
    pub fn apply_bytecode_metadata(
        &mut self,
        metadata: &crate::bytecode::BytecodeMetadata,
        resource_name: Option<&str>,
    ) -> Result<(), RuntimeError> {
        if !metadata.version.is_supported() {
            return Err(RuntimeError::UnsupportedBytecodeVersion {
                major: metadata.version.major,
                minor: metadata.version.minor,
            });
        }
        let resource = match resource_name {
            Some(name) => metadata.resource(name).or_else(|| {
                (metadata.resources.len() == 1
                    && metadata.resources[0].name.eq_ignore_ascii_case("RESOURCE"))
                .then_some(&metadata.resources[0])
            }),
            None => metadata.primary_resource(),
        }
        .ok_or_else(|| {
            RuntimeError::InvalidBytecodeMetadata(match resource_name {
                Some(name) => format!("resource '{name}'").into(),
                None => "resource".into(),
            })
        })?;
        let mut resource = resource.clone();
        if let Some(name) = resource_name {
            resource.name = name.into();
        }
        self.apply_resource_metadata(&resource)
    }

    /// Re-admit task/image changes against the immutable saved application.
    pub fn apply_resource_metadata(
        &mut self,
        resource: &crate::bytecode::ResourceMetadata,
    ) -> Result<(), RuntimeError> {
        use crate::bytecode::{ResourceEntry, SectionData, SectionId, TaskEntry};
        for (area, size) in [
            ("input", resource.process_image.inputs),
            ("output", resource.process_image.outputs),
            ("memory", resource.process_image.memory),
        ] {
            trust_runtime_core::io_image::validate_process_image_area_len(area, size)?;
        }

        let mut module = self
            .artifact
            .as_ref()
            .ok_or_else(|| {
                RuntimeError::InvalidBytecodeMetadata(
                    "installed artifact archive unavailable".into(),
                )
            })?
            .as_ref()
            .clone();
        let references = match module.section(SectionId::RefTable) {
            Some(SectionData::RefTable(table)) => table.entries.len(),
            _ => {
                return Err(RuntimeError::InvalidBytecodeMetadata(
                    "reference table".into(),
                ))
            }
        };
        let mut bound = Vec::new();
        for id in 0..references {
            // Only persistent, publicly resolvable bindings can be task targets.
            match self.state.resolve_persistent_reference(id as u32) {
                Ok(reference) => bound.push((reference, id as u32)),
                // Frame-only and uninstantiated template references cannot be
                // persistent task targets. Budget/deadline/metadata faults are
                // never treated as a missing candidate.
                Err(RuntimeError::TypeMismatch | RuntimeError::NullReference) => {}
                Err(error) => return Err(error),
            }
        }
        let strings = match module.section_mut(SectionId::StringTable) {
            Some(SectionData::StringTable(table)) => table,
            _ => return Err(RuntimeError::InvalidBytecodeMetadata("string table".into())),
        };
        let mut intern = |name: &smol_str::SmolStr| -> Result<u32, RuntimeError> {
            if let Some(index) = strings.entries.iter().position(|old| old == name) {
                return u32::try_from(index).map_err(|_| RuntimeError::Overflow);
            }
            let index = u32::try_from(strings.entries.len()).map_err(|_| RuntimeError::Overflow)?;
            strings.entries.push(name.clone());
            Ok(index)
        };
        let mut tasks = Vec::new();
        for task in &resource.tasks {
            let fb_ref_idx = task
                .fb_instances
                .iter()
                .map(|reference| {
                    bound
                        .iter()
                        .find(|(old, _)| old == reference)
                        .map(|(_, id)| *id)
                        .ok_or_else(|| {
                            RuntimeError::InvalidBytecodeMetadata(
                                "task FB reference is not an admitted persistent binding".into(),
                            )
                        })
                })
                .collect::<Result<_, _>>()?;
            tasks.push(TaskEntry {
                name_idx: intern(&task.name)?,
                priority: task.priority,
                interval_nanos: task.interval.as_nanos(),
                single_name_idx: task.single.as_ref().map(&mut intern).transpose()?,
                program_name_idx: task
                    .programs
                    .iter()
                    .map(&mut intern)
                    .collect::<Result<_, _>>()?,
                fb_ref_idx,
            });
        }
        let entry = ResourceEntry {
            name_idx: intern(&resource.name)?,
            inputs_size: u32::try_from(resource.process_image.inputs)
                .map_err(|_| RuntimeError::Overflow)?,
            outputs_size: u32::try_from(resource.process_image.outputs)
                .map_err(|_| RuntimeError::Overflow)?,
            memory_size: u32::try_from(resource.process_image.memory)
                .map_err(|_| RuntimeError::Overflow)?,
            tasks,
        };
        match module.section_mut(SectionId::ResourceMeta) {
            Some(SectionData::ResourceMeta(meta)) if meta.resources.len() == 1 => {
                meta.resources[0] = entry
            }
            _ => {
                return Err(RuntimeError::InvalidBytecodeMetadata(
                    "resource profile".into(),
                ))
            }
        }
        self.reconfigure_resource_module(module, std::sync::Arc::clone(&self.metadata), true)
    }

    /// Prepare all resource presentation off-state, then publish the common
    /// resource transaction. Persistent references/types are unchanged, so ADS
    /// and OPC UA bindings keep their exact live destinations and queue state.
    pub(super) fn reconfigure_resource_module(
        &mut self,
        module: crate::bytecode::BytecodeModule,
        metadata: std::sync::Arc<super::RuntimeMetadata>,
        reset_task_timing: bool,
    ) -> Result<(), RuntimeError> {
        use std::sync::Arc;
        let bytes = module.encode().map_err(RuntimeError::from)?;
        let (prepared, artifact) =
            trust_runtime_core::vm::PreparedModule::from_bytes_with_artifact(
                &bytes,
                self.prepared().limits(),
            )?;
        let prepared = Arc::new(prepared);
        let (name, tasks, metadata, mut io) =
            Self::prepare_host_schema(&prepared, &mut self.state, Some(metadata), 0)?;
        // Historical task registration retains thread identities even after a
        // task is temporarily removed; new task names receive fresh ids.
        let mut metadata = (*metadata).clone();
        metadata.task_thread_ids = self.metadata.task_thread_ids.clone();
        metadata.background_thread_id = self.metadata.background_thread_id;
        let mut next_thread = metadata
            .task_thread_ids
            .values()
            .copied()
            .chain(metadata.background_thread_id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(RuntimeError::Overflow)?;
        for task in &tasks {
            if !metadata.task_thread_ids.contains_key(&task.name) {
                metadata
                    .task_thread_ids
                    .insert(task.name.clone(), next_thread);
                next_thread = next_thread.checked_add(1).ok_or(RuntimeError::Overflow)?;
            }
        }
        if metadata.has_background_programs() && metadata.background_thread_id.is_none() {
            metadata.background_thread_id = Some(next_thread);
        }
        let metadata = Arc::new(metadata);
        let mut plan = self.state.prepare_resource_reconfiguration(
            Arc::clone(&prepared),
            0,
            reset_task_timing,
        )?;
        self.io.validate_resource_policy(&mut plan)?;
        plan.commit()?;
        self.state.services_mut().set_prepared(prepared);
        self.state
            .services_mut()
            .set_metadata(Arc::clone(&metadata));
        self.io.replace_bindings_from(&mut io);
        self.metadata = metadata;
        self.artifact = Some(Arc::new(artifact.into()));
        self.original_artifact = None;
        self.resource_name = name;
        self.tasks = tasks;
        Ok(())
    }
}

#[cfg(test)]
mod hosted_observation_tests;
#[cfg(test)]
mod installed_write_resource_tests;
#[cfg(test)]
mod tests;
