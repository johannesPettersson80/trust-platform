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
        let resource = match resource_name {
            Some(name) => metadata
                .resource(name)
                .or_else(|| metadata.primary_resource()),
            None => metadata.primary_resource(),
        }
        .ok_or_else(|| error::RuntimeError::InvalidBytecodeMetadata("resource".into()))?;
        self.apply_resource_metadata(resource)?;
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
        module
            .validate()
            .map_err(|err| error::RuntimeError::InvalidBytecode(err.to_string().into()))?;
        let metadata = module
            .metadata()
            .map_err(|err| error::RuntimeError::InvalidBytecode(err.to_string().into()))?;
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
        let module = crate::bytecode::BytecodeModule::decode(bytes)
            .map_err(|err| error::RuntimeError::InvalidBytecode(err.to_string().into()))?;
        self.apply_bytecode_module(&module, resource_name)
    }

    /// Apply a single resource metadata payload.
    pub fn apply_resource_metadata(
        &mut self,
        resource: &crate::bytecode::ResourceMetadata,
    ) -> Result<(), error::RuntimeError> {
        self.io.resize(
            resource.process_image.inputs,
            resource.process_image.outputs,
            resource.process_image.memory,
        );

        self.tasks.clear();
        self.task_state.clear();

        for task in &resource.tasks {
            self.validate_task(task)?;
            self.register_task(task.clone());
        }
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
