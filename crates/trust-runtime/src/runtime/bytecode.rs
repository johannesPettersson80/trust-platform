//! Bytecode application helpers.

#![allow(missing_docs)]

use smol_str::SmolStr;
use std::sync::Arc;

use crate::error;
use crate::task::TaskConfig;
use crate::value::Value;

use super::core::Runtime;

impl Runtime {
    /// Apply bytecode metadata to configure tasks and process images.
    pub fn apply_bytecode_metadata(
        &mut self,
        metadata: &crate::bytecode::BytecodeMetadata,
        resource_name: Option<&str>,
    ) -> Result<(), error::RuntimeError> {
        let version = metadata.version;
        if version.major != crate::bytecode::SUPPORTED_MAJOR_VERSION {
            return Err(error::RuntimeError::UnsupportedBytecodeVersion {
                major: version.major,
                minor: version.minor,
            });
        }
        let (resource, legacy_identity) = match resource_name {
            Some(name) => {
                if let Some(resource) = metadata.resource(name) {
                    (resource, None)
                } else if let Some(resource) = legacy_placeholder_resource(metadata) {
                    (resource, Some(SmolStr::new(name)))
                } else {
                    return Err(error::RuntimeError::InvalidBytecodeMetadata(
                        format!("resource '{name}'").into(),
                    ));
                }
            }
            None => (
                metadata.primary_resource().ok_or_else(|| {
                    error::RuntimeError::InvalidBytecodeMetadata("resource".into())
                })?,
                None,
            ),
        };
        self.apply_resource_metadata(resource)?;
        if let Some(resource_name) = legacy_identity {
            self.resource_name = resource_name;
        }
        self.vm_module = None;
        self.vm_local_init_plan_cache.invalidate_all();
        self.vm_register_lowering_cache.invalidate_all();
        self.vm_tier1_specialized_executor.invalidate_all();
        Ok(())
    }

    /// Apply bytecode container data to configure tasks and process images.
    pub fn apply_bytecode_module(
        &mut self,
        module: &crate::bytecode::BytecodeModule,
        resource_name: Option<&str>,
    ) -> Result<(), error::RuntimeError> {
        module.validate().map_err(error::RuntimeError::from)?;
        let metadata = module.metadata().map_err(error::RuntimeError::from)?;
        // Materialize VM module before mutating runtime metadata so failures do not
        // leave runtime state updated without a corresponding executable module.
        let vm_module = Arc::new(super::vm::VmModule::from_bytecode(module)?);
        self.apply_bytecode_metadata(&metadata, resource_name)?;
        self.vm_module = Some(vm_module);
        Ok(())
    }

    /// Decode a bytecode container and apply its metadata.
    pub fn apply_bytecode_bytes(
        &mut self,
        bytes: &[u8],
        resource_name: Option<&str>,
    ) -> Result<(), error::RuntimeError> {
        let module =
            crate::bytecode::BytecodeModule::decode(bytes).map_err(error::RuntimeError::from)?;
        self.apply_bytecode_module(&module, resource_name)
    }

    /// Apply a single resource metadata payload.
    pub fn apply_resource_metadata(
        &mut self,
        resource: &crate::bytecode::ResourceMetadata,
    ) -> Result<(), error::RuntimeError> {
        for task in &resource.tasks {
            self.validate_task(task)?;
        }

        self.io.try_resize(
            resource.process_image.inputs,
            resource.process_image.outputs,
            resource.process_image.memory,
        )?;

        self.tasks.clear();
        self.task_state.clear();

        for task in &resource.tasks {
            self.register_task(task.clone());
        }
        self.resource_name = resource.name.clone();
        let _ = self.ensure_background_thread_id();
        Ok(())
    }

    fn validate_task(&self, task: &TaskConfig) -> Result<(), error::RuntimeError> {
        for program in &task.programs {
            let exists = self
                .programs
                .keys()
                .any(|name| name.eq_ignore_ascii_case(program.as_ref()));
            if !exists {
                return Err(error::RuntimeError::UndefinedProgram(program.clone()));
            }
        }
        for fb_ref in &task.fb_instances {
            let instance_id = match self.storage.read_by_ref(fb_ref.clone()) {
                Some(Value::Instance(id)) => *id,
                Some(_) => return Err(error::RuntimeError::TypeMismatch),
                None => return Err(error::RuntimeError::NullReference),
            };
            let instance = self
                .storage
                .get_instance(instance_id)
                .ok_or(error::RuntimeError::NullReference)?;
            let key = SmolStr::new(instance.type_name.to_ascii_uppercase());
            if self.function_blocks.get(&key).is_none() {
                return Err(error::RuntimeError::UndefinedFunctionBlock(
                    instance.type_name.clone(),
                ));
            }
        }
        Ok(())
    }
}

fn legacy_placeholder_resource(
    metadata: &crate::bytecode::BytecodeMetadata,
) -> Option<&crate::bytecode::ResourceMetadata> {
    let resource = metadata.resources.first()?;
    (metadata.resources.len() == 1 && resource.name.eq_ignore_ascii_case("RESOURCE"))
        .then_some(resource)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bytecode::{
        BytecodeMetadata, BytecodeVersion, ProcessImageConfig, ResourceMetadata,
        SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION,
    };
    use crate::value::Duration;

    #[test]
    fn legacy_placeholder_requires_one_resource_named_resource() {
        let placeholder = metadata(vec![resource("rEsOuRcE")]);
        assert_eq!(
            legacy_placeholder_resource(&placeholder).map(|resource| resource.name.as_str()),
            Some("rEsOuRcE")
        );

        for resources in [
            Vec::new(),
            vec![resource("Plant")],
            vec![resource("RESOURCE"), resource("Other")],
        ] {
            assert!(legacy_placeholder_resource(&metadata(resources)).is_none());
        }
    }

    #[test]
    fn resource_metadata_accepts_case_only_program_reference_and_preserves_task_spelling() {
        let mut runtime = Runtime::new();
        runtime
            .register_program(crate::task::ProgramDef {
                name: "Main".into(),
                vars: Vec::new(),
                temps: Vec::new(),
                using: Vec::new(),
                body: Vec::new(),
            })
            .unwrap();
        let mut selected = resource("Plant");
        selected.tasks.push(TaskConfig {
            name: "Fast".into(),
            interval: Duration::from_millis(10),
            single: None,
            priority: 1,
            programs: vec!["mAiN".into()],
            fb_instances: Vec::new(),
        });

        runtime.apply_resource_metadata(&selected).unwrap();

        assert_eq!(runtime.resource_name(), "Plant");
        assert_eq!(runtime.tasks().len(), 1);
        assert_eq!(runtime.tasks()[0].programs, vec![SmolStr::new("mAiN")]);
    }

    fn metadata(resources: Vec<ResourceMetadata>) -> BytecodeMetadata {
        BytecodeMetadata {
            version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION),
            resources,
        }
    }

    fn resource(name: &str) -> ResourceMetadata {
        ResourceMetadata {
            name: name.into(),
            process_image: ProcessImageConfig::default(),
            tasks: Vec::new(),
        }
    }
}
