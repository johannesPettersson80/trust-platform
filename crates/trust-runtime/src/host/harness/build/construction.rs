//! The declared program in its starting state: the globals and program instances with the starting
//! values the build computes, the configuration's initializations, access paths and tasks, which the
//! tools read without running the program (the HMI's catalog, the browse of a project's symbols).
use super::{
    build_extra_program_instances, ensure_all_program_declarations_bound, instance_program_access,
    validate_task_single_bindings, LoweredApplication,
};
use crate::harness::config::{
    apply_config_inits, apply_globals, attach_fb_instances_to_tasks, attach_programs_to_tasks,
    ensure_wildcards_resolved, register_access_bindings, register_program_instances,
};
use crate::harness::{self, CompileError};
use crate::{memory::IoArea, task::ProgramDef, RuntimeBuilder};
use indexmap::IndexMap;
use smol_str::SmolStr;

impl LoweredApplication {
    pub(in crate::harness) fn construct(
        self,
        extra_program_instances: &[SmolStr],
    ) -> Result<(RuntimeBuilder, IndexMap<SmolStr, ProgramDef>), CompileError> {
        let Self {
            builder,
            program_defs,
            mut globals,
            mut program_access,
            configuration: config_model,
            source_metadata,
            test_programs,
        } = self;
        let mut runtime = builder;
        if let Some(config) = config_model {
            if let Some(resource_name) = config.resource_name.as_ref() {
                runtime.set_resource_name(resource_name.clone());
            }
            globals.extend(config.globals);
            let mut wildcards = apply_globals(&mut runtime, &globals)?;
            register_program_instances(
                &mut runtime,
                &program_defs,
                &config.programs,
                &config.using,
                &mut wildcards,
            )?;
            let extra_programs = build_extra_program_instances(
                &program_defs,
                &config.programs,
                extra_program_instances,
            )?;
            program_access = instance_program_access(
                program_access,
                &program_defs,
                &config.programs,
                &config.using,
            )?;
            ensure_all_program_declarations_bound(
                &program_defs,
                &config.programs,
                &extra_programs,
                &config.using,
                &test_programs,
            )?;
            register_program_instances(
                &mut runtime,
                &program_defs,
                &extra_programs,
                &config.using,
                &mut wildcards,
            )?;
            apply_config_inits(&mut runtime, &config.config_inits, &mut wildcards)?;
            ensure_wildcards_resolved(&wildcards)?;
            register_access_bindings(&mut runtime, &config.access)?;
            let mut tasks = config.tasks;
            validate_task_single_bindings(&runtime, &tasks)?;
            attach_programs_to_tasks(&mut tasks, &config.programs)?;
            attach_fb_instances_to_tasks(&runtime, &mut tasks, &config.programs)?;
            for task in tasks {
                runtime.register_task(task);
            }
        } else {
            if program_defs.is_empty() {
                return Err(CompileError::new("missing PROGRAM declaration"));
            }
            let mut wildcards = apply_globals(&mut runtime, &globals)?;
            let default_programs = program_defs
                .values()
                .map(|program| harness::ProgramInstanceConfig {
                    name: program.name.clone(),
                    type_name: program.name.clone(),
                    task: None,
                    retain: None,
                    fb_tasks: Vec::new(),
                    connections: Vec::new(),
                })
                .collect::<Vec<_>>();
            register_program_instances(
                &mut runtime,
                &program_defs,
                &default_programs,
                &[],
                &mut wildcards,
            )?;
            ensure_wildcards_resolved(&wildcards)?;
        }

        register_access_bindings(&mut runtime, &program_access)?;

        resize_process_image_from_bindings(&mut runtime)?;
        init_function_static_locals(&mut runtime)?;

        for source in source_metadata {
            runtime.register_statement_locations(source.file_id, source.locations);
        }

        Ok((runtime, program_defs))
    }
}

fn resize_process_image_from_bindings(runtime: &mut RuntimeBuilder) -> Result<(), CompileError> {
    let mut inputs = runtime.io().inputs().len();
    let mut outputs = runtime.io().outputs().len();
    let mut memory = runtime.io().memory().len();

    for binding in runtime.io().bindings() {
        let address = &binding.address;
        if address.wildcard || address.path.len() > 1 {
            continue;
        }
        let span = match address.size {
            crate::io::IoSize::Bit | crate::io::IoSize::Byte => 1usize,
            crate::io::IoSize::Word => 2usize,
            crate::io::IoSize::DWord => 4usize,
            crate::io::IoSize::LWord => 8usize,
            crate::io::IoSize::Bytes(len) => len as usize,
        };
        let required = address.byte as usize + span;
        match address.area {
            IoArea::Input => inputs = inputs.max(required),
            IoArea::Output => outputs = outputs.max(required),
            IoArea::Memory => memory = memory.max(required),
        }
    }

    runtime
        .io_mut()
        .try_resize(inputs, outputs, memory)
        .map_err(|err| CompileError::new(err.to_string()))
}

fn init_function_static_locals(runtime: &mut RuntimeBuilder) -> Result<(), CompileError> {
    let registry = runtime.registry().clone();
    let profile = runtime.profile();
    let functions = runtime.functions().clone();
    let stdlib = runtime.stdlib().clone();
    let function_blocks = runtime.function_blocks().clone();
    let classes = runtime.classes().clone();
    let initializer_catalog = runtime.initializer_catalog().clone();
    let storage = runtime.storage_mut();

    for function in functions.values() {
        for local in &function.static_locals {
            let key = crate::program_model::static_storage_name(&function.name, &local.name);
            if let Some(fb_name) = harness::function_block_type_name(local.type_id, &registry) {
                if local.initializer.is_some() {
                    return Err(CompileError::new(
                        "function VAR_STAT function block instances cannot have initializers",
                    ));
                }
                let fb_key = SmolStr::new(fb_name.to_ascii_uppercase());
                let fb = function_blocks.get(&fb_key).ok_or_else(|| {
                    CompileError::new(format!("unknown function block '{fb_name}'"))
                })?;
                let instance_id = crate::instance::create_fb_instance(
                    storage,
                    &registry,
                    &profile,
                    &classes,
                    &function_blocks,
                    &functions,
                    &stdlib,
                    &initializer_catalog,
                    fb,
                )
                .map_err(|err| CompileError::new(err.to_string()))?;
                storage.set_global(key, crate::value::Value::Instance(instance_id));
                continue;
            }
            if let Some(class_name) = harness::class_type_name(local.type_id, &registry) {
                if local.initializer.is_some() {
                    return Err(CompileError::new(
                        "function VAR_STAT class instances cannot have initializers",
                    ));
                }
                let class_key = SmolStr::new(class_name.to_ascii_uppercase());
                let class_def = classes
                    .get(&class_key)
                    .ok_or_else(|| CompileError::new(format!("unknown class '{class_name}'")))?;
                let instance_id = crate::instance::create_class_instance(
                    storage,
                    &registry,
                    &profile,
                    &classes,
                    &function_blocks,
                    &functions,
                    &stdlib,
                    &initializer_catalog,
                    class_def,
                )
                .map_err(|err| CompileError::new(err.to_string()))?;
                storage.set_global(key, crate::value::Value::Instance(instance_id));
                continue;
            }
            if harness::interface_type_name(local.type_id, &registry).is_some() {
                storage.set_global(key, crate::value::Value::Null);
                continue;
            }
            let value = crate::harness::initializer::default_value_for_type_id(
                storage,
                &registry,
                &initializer_catalog,
                &profile,
                None,
                &stdlib,
                local.type_id,
            )
            .map_err(|err| CompileError::new(format!("default value error: {err}")))?;
            storage.set_global(key, value);
        }
    }

    for function in functions.values() {
        for local in &function.static_locals {
            if harness::function_block_type_name(local.type_id, &registry).is_some()
                || harness::class_type_name(local.type_id, &registry).is_some()
            {
                continue;
            }
            let Some(expr) = &local.initializer else {
                continue;
            };
            let value = crate::harness::initializer::evaluate_initializer(
                storage,
                &registry,
                &initializer_catalog,
                &profile,
                None,
                &stdlib,
                expr,
                local.type_id,
            )
            .map_err(|err| CompileError::new(format!("initializer error: {err}")))?;
            let key = crate::program_model::static_storage_name(&function.name, &local.name);
            storage.set_global(key, value);
        }
    }

    Ok(())
}
