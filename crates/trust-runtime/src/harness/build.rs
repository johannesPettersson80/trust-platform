use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::debug::SourceLocation;
use crate::task::ProgramDef;
use crate::Runtime;
use std::path::Path;
use trust_hir::db::SemanticDatabase;
use trust_hir::{Project, SourceKey};
use trust_syntax::parser;

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

    let mut runtime = Runtime::new();
    let profile = runtime.profile();
    let mut statement_locations: Vec<Vec<SourceLocation>> = vec![Vec::new(); sources.len()];

    for (idx, parse) in parses.iter().enumerate() {
        let syntax = parse.syntax();
        super::lower_type_decls(
            &syntax,
            runtime.registry_mut(),
            profile,
            file_ids[idx].0,
            &mut statement_locations[idx],
        )?;
    }

    for parse in &parses {
        let syntax = parse.syntax();
        super::predeclare_function_blocks(&syntax, runtime.registry_mut())?;
        super::predeclare_classes(&syntax, runtime.registry_mut())?;
        super::predeclare_interfaces(&syntax, runtime.registry_mut())?;
    }

    let mut interface_names = std::collections::HashSet::new();
    for (idx, parse) in parses.iter().enumerate() {
        let syntax = parse.syntax();
        let interfaces = super::lower_interfaces(
            &syntax,
            runtime.registry_mut(),
            profile,
            file_ids[idx].0,
            &mut statement_locations[idx],
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
        let classes = super::lower_classes(
            &syntax,
            runtime.registry_mut(),
            profile,
            file_ids[idx].0,
            &mut statement_locations[idx],
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
        let function_blocks = super::lower_function_blocks(
            &syntax,
            runtime.registry_mut(),
            profile,
            file_ids[idx].0,
            &mut statement_locations[idx],
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
        let functions = super::lower_functions(
            &syntax,
            runtime.registry_mut(),
            profile,
            file_ids[idx].0,
            &mut statement_locations[idx],
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
        let lowered = super::lower_programs(
            &syntax,
            runtime.registry_mut(),
            profile,
            file_ids[idx].0,
            &mut statement_locations[idx],
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
        if let Some(config) = super::lower_configuration(
            &syntax,
            runtime.registry_mut(),
            profile,
            file_ids[idx].0,
            &mut statement_locations[idx],
        )? {
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
        register_program_instances(
            &mut runtime,
            &program_defs,
            &extra_programs,
            &config.using,
            &mut wildcards,
        )?;
        apply_config_inits(
            &mut runtime,
            &config.config_inits,
            &config.using,
            &mut wildcards,
        )?;
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
        crate::bytecode::BytecodeModule::from_runtime_with_sources_and_paths(
            runtime,
            &source_refs,
            &paths,
        )
        .map_err(|err| CompileError::new(err.to_string()))
    } else {
        crate::bytecode::BytecodeModule::from_runtime_with_sources(runtime, &source_refs)
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
