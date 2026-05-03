use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::debug::SourceLocation;
use crate::task::ProgramDef;
use crate::Runtime;
use std::path::Path;
use trust_hir::db::SemanticDatabase;
use trust_hir::{Project, SourceKey};
use trust_syntax::parser;

use super::compiler::{lower_root_global_var_blocks, CompileTimeConsts, LoweringInputs};
use super::config::{
    apply_config_inits, apply_globals, apply_program_retain_overrides,
    attach_fb_instances_to_tasks, attach_programs_to_tasks, ensure_wildcards_resolved,
    register_access_bindings, register_program_instances,
};
use super::types::{CompileError, SourceFile};

pub(super) fn build_runtime_from_source_files(
    sources: &[SourceFile],
    label_errors: bool,
    extra_program_instances: &[SmolStr],
) -> Result<Runtime, CompileError> {
    let mut parses = Vec::with_capacity(sources.len());
    let mut parse_errors = Vec::new();
    for (idx, source) in sources.iter().enumerate() {
        let parse = parser::parse(&source.text);
        if !parse.ok() {
            for err in parse.errors() {
                if label_errors {
                    parse_errors.push(format!("{}: {err}", source_label(source, idx)));
                } else {
                    parse_errors.push(err.to_string());
                }
            }
        }
        parses.push(parse);
    }
    if !parse_errors.is_empty() {
        return Err(CompileError::new(parse_errors.join("\n")));
    }

    let mut project = Project::new();
    let mut file_ids = Vec::with_capacity(sources.len());
    for (idx, source) in sources.iter().enumerate() {
        let key = match source.path.as_deref() {
            Some(path) => SourceKey::from_path(Path::new(path)),
            None => SourceKey::from_virtual(format!("file_{idx}")),
        };
        let file_id = project.set_source_text(key, source.text.clone());
        file_ids.push(file_id);
    }

    let mut diagnostics_errors = Vec::new();
    for (idx, file_id) in file_ids.iter().enumerate() {
        let diagnostics = project.database().diagnostics(*file_id);
        for diag in diagnostics.iter().filter(|diag| diag.is_error()) {
            if label_errors {
                diagnostics_errors.push(format!("{}: {diag}", source_label(&sources[idx], idx)));
            } else {
                diagnostics_errors.push(diag.to_string());
            }
        }
    }
    if !diagnostics_errors.is_empty() {
        return Err(CompileError::new(diagnostics_errors.join("\n")));
    }
    let analyses = file_ids
        .iter()
        .map(|file_id| project.database().analyze(*file_id))
        .collect::<Vec<_>>();

    let mut runtime = Runtime::new();
    let profile = runtime.profile();
    let mut statement_locations: Vec<Vec<SourceLocation>> = vec![Vec::new(); sources.len()];

    for (idx, parse) in parses.iter().enumerate() {
        let syntax = parse.syntax();
        let (registry, initializer_catalog) = runtime.registry_and_initializer_catalog_mut();
        super::lower_type_decls(
            &syntax,
            registry,
            initializer_catalog,
            profile,
            project.database(),
            file_ids[idx],
            file_ids[idx].0,
            &mut statement_locations[idx],
        )?;
    }

    for idx in 0..parses.len() {
        let catalog = analyses[idx].declaration_catalog.as_ref();
        super::predeclare_function_blocks(catalog, file_ids[idx], runtime.registry_mut())?;
        super::predeclare_classes(catalog, file_ids[idx], runtime.registry_mut())?;
        super::predeclare_interfaces(catalog, file_ids[idx], runtime.registry_mut())?;
    }

    let mut compile_time_consts = CompileTimeConsts::default();
    let mut root_globals_per_file = Vec::with_capacity(parses.len());
    for (idx, parse) in parses.iter().enumerate() {
        let syntax = parse.syntax();
        let mut inputs = LoweringInputs::new(
            profile,
            file_ids[idx].0,
            Some(project.database()),
            Some(file_ids[idx]),
            &mut statement_locations[idx],
            std::mem::take(&mut compile_time_consts),
        );
        let globals = lower_root_global_var_blocks(&syntax, runtime.registry_mut(), &mut inputs)?;
        compile_time_consts = inputs.compile_time_consts;
        root_globals_per_file.push(globals);
    }

    let mut interface_names = std::collections::HashSet::new();
    for (idx, parse) in parses.iter().enumerate() {
        let syntax = parse.syntax();
        let mut inputs = LoweringInputs::new(
            profile,
            file_ids[idx].0,
            Some(project.database()),
            Some(file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        let interfaces = super::lower_interfaces(
            &syntax,
            analyses[idx].declaration_catalog.as_ref(),
            file_ids[idx],
            runtime.registry_mut(),
            &mut inputs,
        )?;
        for interface_def in interfaces {
            let key = interface_def.name.to_ascii_uppercase();
            if !interface_names.insert(key.clone()) {
                return Err(CompileError::new(format!(
                    "duplicate INTERFACE name '{}'",
                    interface_def.name
                )));
            }
            runtime.register_interface(interface_def);
        }
    }

    let mut class_names = std::collections::HashSet::new();
    for (idx, parse) in parses.iter().enumerate() {
        let syntax = parse.syntax();
        let mut inputs = LoweringInputs::new(
            profile,
            file_ids[idx].0,
            Some(project.database()),
            Some(file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        let classes = super::lower_classes(
            &syntax,
            analyses[idx].declaration_catalog.as_ref(),
            file_ids[idx],
            runtime.registry_mut(),
            &mut inputs,
        )?;
        for class_def in classes {
            let key = class_def.name.to_ascii_uppercase();
            if !class_names.insert(key.clone()) {
                return Err(CompileError::new(format!(
                    "duplicate CLASS name '{}'",
                    class_def.name
                )));
            }
            runtime.register_class(class_def);
        }
    }

    let mut function_block_names = std::collections::HashSet::new();
    for (idx, parse) in parses.iter().enumerate() {
        let syntax = parse.syntax();
        let mut inputs = LoweringInputs::new(
            profile,
            file_ids[idx].0,
            Some(project.database()),
            Some(file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        let function_blocks = super::lower_function_blocks(
            &syntax,
            analyses[idx].declaration_catalog.as_ref(),
            file_ids[idx],
            runtime.registry_mut(),
            &mut inputs,
        )?;
        for fb in function_blocks {
            let key = fb.name.to_ascii_uppercase();
            if !function_block_names.insert(key.clone()) {
                return Err(CompileError::new(format!(
                    "duplicate FUNCTION_BLOCK name '{}'",
                    fb.name
                )));
            }
            runtime.register_function_block(fb);
        }
    }

    let mut function_names = std::collections::HashSet::new();
    for (idx, parse) in parses.iter().enumerate() {
        let syntax = parse.syntax();
        let mut inputs = LoweringInputs::new(
            profile,
            file_ids[idx].0,
            Some(project.database()),
            Some(file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        let functions = super::lower_functions(
            &syntax,
            analyses[idx].declaration_catalog.as_ref(),
            file_ids[idx],
            runtime.registry_mut(),
            &mut inputs,
        )?;
        for func in functions {
            let key = func.name.to_ascii_uppercase();
            if !function_names.insert(key.clone()) {
                return Err(CompileError::new(format!(
                    "duplicate FUNCTION name '{}'",
                    func.name
                )));
            }
            runtime.register_function(func);
        }
    }

    let mut program_defs = IndexMap::<SmolStr, ProgramDef>::new();
    let mut globals = Vec::new();
    for (idx, parse) in parses.iter().enumerate() {
        let syntax = parse.syntax();
        globals.extend(root_globals_per_file[idx].clone());
        let mut inputs = LoweringInputs::new(
            profile,
            file_ids[idx].0,
            Some(project.database()),
            Some(file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        let lowered = super::lower_programs(
            &syntax,
            analyses[idx].declaration_catalog.as_ref(),
            file_ids[idx],
            runtime.registry_mut(),
            &mut inputs,
        )?;
        for program in lowered {
            let key = program.program.name.to_ascii_uppercase();
            if program_defs.contains_key(key.as_str()) {
                return Err(CompileError::new(format!(
                    "duplicate PROGRAM name '{}'",
                    program.program.name
                )));
            }
            program_defs.insert(key.into(), program.program);
            globals.extend(program.globals);
        }
    }

    let mut config_model = None;
    for (idx, parse) in parses.iter().enumerate() {
        let syntax = parse.syntax();
        let mut inputs = LoweringInputs::new(
            profile,
            file_ids[idx].0,
            Some(project.database()),
            Some(file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        if let Some(config) =
            super::lower_configuration(&syntax, runtime.registry_mut(), &mut inputs)?
        {
            if config_model.is_some() {
                return Err(CompileError::new(
                    "multiple CONFIGURATION declarations not supported",
                ));
            }
            config_model = Some(config);
        }
    }

    if let Some(config) = config_model {
        globals.extend(config.globals);
        apply_program_retain_overrides(&mut program_defs, &config.programs, &config.using)?;
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
        ensure_all_program_declarations_bound(
            &program_defs,
            &config.programs,
            &extra_programs,
            &config.using,
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
            .map(|program| super::ProgramInstanceConfig {
                name: program.name.clone(),
                type_name: program.name.clone(),
                task: None,
                retain: None,
                fb_tasks: Vec::new(),
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

    init_function_static_locals(&mut runtime)?;
    let _ = runtime.ensure_background_thread_id();

    for (idx, locations) in statement_locations.into_iter().enumerate() {
        let file_id = file_ids[idx].0;
        runtime.register_statement_locations(file_id, locations);
        runtime.register_source_text(file_id, sources[idx].text.clone());
        runtime.register_source_label(file_id, format!("file_{file_id}"));
        if let Some(path) = sources[idx].path.as_deref() {
            runtime.register_source_label(file_id, path);
        }
    }

    Ok(runtime)
}

fn init_function_static_locals(runtime: &mut Runtime) -> Result<(), CompileError> {
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
            if let Some(fb_name) = super::function_block_type_name(local.type_id, &registry) {
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
            if let Some(class_name) = super::class_type_name(local.type_id, &registry) {
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
            if super::interface_type_name(local.type_id, &registry).is_some() {
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
            if super::function_block_type_name(local.type_id, &registry).is_some()
                || super::class_type_name(local.type_id, &registry).is_some()
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

pub(super) fn build_bytecode_module_from_source_files(
    sources: &[SourceFile],
    label_errors: bool,
    extra_program_instances: &[SmolStr],
) -> Result<crate::bytecode::BytecodeModule, CompileError> {
    let runtime = build_runtime_from_source_files(sources, label_errors, extra_program_instances)?;
    build_bytecode_module_from_runtime_and_sources(&runtime, sources)
}

fn build_extra_program_instances(
    program_defs: &IndexMap<SmolStr, ProgramDef>,
    configured_programs: &[super::ProgramInstanceConfig],
    extra_program_instances: &[SmolStr],
) -> Result<Vec<super::ProgramInstanceConfig>, CompileError> {
    if extra_program_instances.is_empty() {
        return Ok(Vec::new());
    }

    let configured_names = configured_programs
        .iter()
        .map(|program| program.name.to_ascii_uppercase())
        .collect::<std::collections::HashSet<_>>();
    let mut seen = std::collections::HashSet::new();
    let mut extra_programs = Vec::new();

    for name in extra_program_instances {
        let key = name.to_ascii_uppercase();
        if configured_names.contains(&key) || !seen.insert(key.clone()) {
            continue;
        }
        if !program_defs.contains_key(key.as_str()) {
            return Err(CompileError::new(format!(
                "extra PROGRAM instance '{}' has no matching declaration",
                name
            )));
        }
        extra_programs.push(super::ProgramInstanceConfig {
            name: name.clone(),
            type_name: name.clone(),
            task: None,
            retain: None,
            fb_tasks: Vec::new(),
        });
    }

    Ok(extra_programs)
}

fn ensure_all_program_declarations_bound(
    program_defs: &IndexMap<SmolStr, ProgramDef>,
    configured_programs: &[super::ProgramInstanceConfig],
    extra_programs: &[super::ProgramInstanceConfig],
    using: &[SmolStr],
) -> Result<(), CompileError> {
    if program_defs.is_empty() {
        return Ok(());
    }

    let mut bound_types = std::collections::BTreeSet::new();
    for program in configured_programs.iter().chain(extra_programs.iter()) {
        let type_name = super::resolve_program_type_name(program_defs, &program.type_name, using)?;
        bound_types.insert(type_name.to_ascii_uppercase());
    }

    let unbound = program_defs
        .values()
        .filter(|program| !bound_types.contains(&program.name.to_ascii_uppercase()))
        .map(|program| program.name.as_str())
        .collect::<Vec<_>>();
    if unbound.is_empty() {
        return Ok(());
    }

    Err(CompileError::new(format!(
        "unbound PROGRAM declaration(s) under CONFIGURATION: {}. Bind each declared PROGRAM with RESOURCE ... PROGRAM ... WITH, or register explicit extra program instances for test builders.",
        unbound.join(", ")
    )))
}

fn build_bytecode_module_from_runtime_and_sources(
    runtime: &Runtime,
    sources: &[SourceFile],
) -> Result<crate::bytecode::BytecodeModule, CompileError> {
    let source_refs = sources
        .iter()
        .map(|source| source.text.as_str())
        .collect::<Vec<_>>();
    if sources.iter().all(|source| source.path.is_some()) {
        let paths = sources
            .iter()
            .map(|source| source.path.as_deref().unwrap_or_default())
            .collect::<Vec<_>>();
        crate::bytecode::build_module_from_runtime_with_sources_and_paths(
            runtime,
            &source_refs,
            &paths,
        )
        .map_err(|err| CompileError::new(err.to_string()))
    } else {
        crate::bytecode::build_module_from_runtime_with_sources(runtime, &source_refs)
            .map_err(|err| CompileError::new(err.to_string()))
    }
}

fn source_label(source: &SourceFile, idx: usize) -> String {
    source
        .path
        .as_deref()
        .map(|path| path.to_string())
        .unwrap_or_else(|| format!("file {idx}"))
}
