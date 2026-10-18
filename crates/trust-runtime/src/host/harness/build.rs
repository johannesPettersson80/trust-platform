mod legacy_construction;

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::debug::SourceLocation;
use crate::task::ProgramDef;
use crate::{Runtime, RuntimeBuilder};
use std::{path::Path, sync::Arc};
use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::{semantic::DeclarationCatalog, Project, SourceKey};
use trust_syntax::{parser, SyntaxNode};

use super::compiler::{lower_root_global_var_blocks, LoweringInputs};

use super::config::{apply_program_retain_overrides, attach_programs_to_tasks};
use super::types::{CompileError, SourceFile};

/// Lowered declarations and original initialization expressions, before any runtime
/// storage construction. Legacy execution and source-free artifact authoring consume
/// the same frontend result, but only the legacy consumer evaluates startup values.
pub(crate) struct LoweredApplication {
    pub(crate) builder: RuntimeBuilder,
    pub(crate) program_defs: IndexMap<SmolStr, ProgramDef>,
    /// Borrowed global aliases needed by source configuration checks, never runtime fields.
    pub(crate) program_externals: IndexMap<SmolStr, Vec<SmolStr>>,
    pub(crate) globals: Vec<super::compiler::GlobalInit>,
    pub(crate) program_access: Vec<super::compiler::AccessDecl>,
    pub(crate) configuration: Option<super::compiler::ConfigModel>,
    pub(crate) source_metadata: Vec<LoweredSourceMetadata>,
}

pub(crate) struct LoweredSourceMetadata {
    pub(crate) file_id: u32,
    pub(crate) locations: Vec<SourceLocation>,
    pub(crate) text: String,
    pub(crate) path: Option<String>,
}

pub(super) fn build_runtime_from_source_files(
    sources: &[SourceFile],
    label_errors: bool,
    extra_program_instances: &[SmolStr],
) -> Result<Runtime, CompileError> {
    lower_application(sources, label_errors)?.materialize(extra_program_instances)
}

pub(crate) fn lower_application(
    sources: &[SourceFile],
    label_errors: bool,
) -> Result<LoweredApplication, CompileError> {
    lower_application_with_abi(sources, label_errors, crate::value::StorageAbi::default())
}

struct AnalyzedSourceFiles {
    project: Project,
    file_ids: Vec<FileId>,
    parses: Vec<parser::Parse>,
    declaration_catalogs: Vec<Arc<DeclarationCatalog>>,
    syntaxes: Vec<SyntaxNode>,
}

fn analyze_source_files(
    sources: &[SourceFile],
    label_errors: bool,
    storage_abi: crate::value::StorageAbi,
) -> Result<AnalyzedSourceFiles, CompileError> {
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
    project
        .database_mut()
        .set_temporal_storage(match storage_abi {
            crate::value::StorageAbi::LegacyTemporal32 => trust_hir::TemporalStorage::Legacy32,
            crate::value::StorageAbi::UniformTemporal64 => trust_hir::TemporalStorage::Uniform64,
        });
    let mut file_ids = Vec::with_capacity(sources.len());
    for (idx, source) in sources.iter().enumerate() {
        let key = match source.path.as_deref() {
            Some(path) => SourceKey::from_path(Path::new(path)),
            None => SourceKey::from_virtual(format!("file_{idx}")),
        };
        file_ids.push(project.set_source_text(key, source.text.clone()));
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
    let declaration_catalogs = file_ids
        .iter()
        .map(|file_id| Arc::clone(&project.database().analyze(*file_id).declaration_catalog))
        .collect();
    let syntaxes = parses.iter().map(|parse| parse.syntax()).collect();
    Ok(AnalyzedSourceFiles {
        project,
        file_ids,
        parses,
        declaration_catalogs,
        syntaxes,
    })
}

pub(crate) fn lower_application_with_abi(
    sources: &[SourceFile],
    label_errors: bool,
    storage_abi: crate::value::StorageAbi,
) -> Result<LoweredApplication, CompileError> {
    let frontend = analyze_source_files(sources, label_errors, storage_abi)?;

    let mut builder = RuntimeBuilder::new();
    builder.storage_abi = storage_abi;
    let mut statement_locations: Vec<Vec<SourceLocation>> = vec![Vec::new(); sources.len()];
    let (compile_time_consts, root_globals_per_file) = lower_project_types(
        &frontend,
        &mut builder,
        &mut statement_locations,
        storage_abi,
    )?;
    lower_named_declarations(
        &frontend,
        &mut builder,
        &mut statement_locations,
        &compile_time_consts,
    )?;
    let lowered_programs = lower_programs_and_configuration(
        &frontend,
        &mut builder,
        &mut statement_locations,
        &compile_time_consts,
        root_globals_per_file,
    )?;
    let source_metadata = statement_locations
        .into_iter()
        .enumerate()
        .map(|(idx, locations)| LoweredSourceMetadata {
            file_id: frontend.file_ids[idx].0,
            locations,
            text: sources[idx].text.clone(),
            path: sources[idx].path.clone(),
        })
        .collect();
    Ok(LoweredApplication {
        builder,
        program_defs: lowered_programs.program_defs,
        program_externals: lowered_programs.program_externals,
        globals: lowered_programs.globals,
        program_access: lowered_programs.program_access,
        configuration: lowered_programs.configuration,
        source_metadata,
    })
}

fn lower_project_types(
    frontend: &AnalyzedSourceFiles,
    builder: &mut RuntimeBuilder,
    statement_locations: &mut [Vec<SourceLocation>],
    storage_abi: crate::value::StorageAbi,
) -> Result<
    (
        super::compiler::CompileTimeConsts,
        Vec<Vec<super::compiler::GlobalInit>>,
    ),
    CompileError,
> {
    let profile = builder.profile();
    let type_ids = super::predeclare_project_types(&frontend.syntaxes, builder.registry_mut())?;

    for idx in 0..frontend.parses.len() {
        let catalog = frontend.declaration_catalogs[idx].as_ref();
        super::predeclare_function_blocks(catalog, frontend.file_ids[idx], builder.registry_mut())?;
        super::predeclare_classes(catalog, frontend.file_ids[idx], builder.registry_mut())?;
        super::predeclare_interfaces(catalog, frontend.file_ids[idx], builder.registry_mut())?;
    }

    let preliminary_consts = super::resolve_project_global_constants(
        &frontend.syntaxes,
        builder.registry_mut(),
        profile,
        frontend.project.database(),
        &frontend.file_ids,
        statement_locations,
        true,
        storage_abi,
    )?;

    for (idx, syntax) in frontend.syntaxes.iter().enumerate() {
        let (registry, initializer_catalog) = builder.registry_and_initializer_catalog_mut();
        super::lower_type_decls(
            syntax,
            registry,
            initializer_catalog,
            profile,
            frontend.project.database(),
            frontend.file_ids[idx],
            frontend.file_ids[idx].0,
            &mut statement_locations[idx],
            preliminary_consts.clone(),
        )?;
    }
    super::validate_project_aliases(&type_ids, builder.registry())?;

    let mut compile_time_consts = super::resolve_project_global_constants(
        &frontend.syntaxes,
        builder.registry_mut(),
        profile,
        frontend.project.database(),
        &frontend.file_ids,
        statement_locations,
        false,
        storage_abi,
    )?;
    let mut root_globals_per_file = Vec::with_capacity(frontend.parses.len());
    for (idx, syntax) in frontend.syntaxes.iter().enumerate() {
        let mut inputs = LoweringInputs::new(
            profile,
            frontend.file_ids[idx].0,
            Some(frontend.project.database()),
            Some(frontend.file_ids[idx]),
            &mut statement_locations[idx],
            std::mem::take(&mut compile_time_consts),
        );
        let globals = lower_root_global_var_blocks(syntax, builder.registry_mut(), &mut inputs)?;
        compile_time_consts = inputs.compile_time_consts;
        root_globals_per_file.push(globals);
    }

    Ok((compile_time_consts, root_globals_per_file))
}

fn lower_named_declarations(
    frontend: &AnalyzedSourceFiles,
    builder: &mut RuntimeBuilder,
    statement_locations: &mut [Vec<SourceLocation>],
    compile_time_consts: &super::compiler::CompileTimeConsts,
) -> Result<(), CompileError> {
    let profile = builder.profile();
    let mut interface_names = std::collections::HashSet::new();
    for (idx, parse) in frontend.parses.iter().enumerate() {
        let syntax = parse.syntax();
        let mut inputs = LoweringInputs::new(
            profile,
            frontend.file_ids[idx].0,
            Some(frontend.project.database()),
            Some(frontend.file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        let interfaces = super::lower_interfaces(
            &syntax,
            frontend.declaration_catalogs[idx].as_ref(),
            frontend.file_ids[idx],
            builder.registry_mut(),
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
            builder.register_interface(interface_def);
        }
    }

    let mut class_names = std::collections::HashSet::new();
    for (idx, parse) in frontend.parses.iter().enumerate() {
        let syntax = parse.syntax();
        let mut inputs = LoweringInputs::new(
            profile,
            frontend.file_ids[idx].0,
            Some(frontend.project.database()),
            Some(frontend.file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        let classes = super::lower_classes(
            &syntax,
            frontend.declaration_catalogs[idx].as_ref(),
            frontend.file_ids[idx],
            builder.registry_mut(),
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
            builder.register_class(class_def);
        }
    }

    let mut function_block_names = std::collections::HashSet::new();
    for (idx, parse) in frontend.parses.iter().enumerate() {
        let syntax = parse.syntax();
        let mut inputs = LoweringInputs::new(
            profile,
            frontend.file_ids[idx].0,
            Some(frontend.project.database()),
            Some(frontend.file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        let function_blocks = super::lower_function_blocks(
            &syntax,
            frontend.declaration_catalogs[idx].as_ref(),
            frontend.file_ids[idx],
            builder.registry_mut(),
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
            builder.register_function_block(fb);
        }
    }

    let mut function_names = std::collections::HashSet::new();
    for (idx, parse) in frontend.parses.iter().enumerate() {
        let syntax = parse.syntax();
        let mut inputs = LoweringInputs::new(
            profile,
            frontend.file_ids[idx].0,
            Some(frontend.project.database()),
            Some(frontend.file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        let functions = super::lower_functions(
            &syntax,
            frontend.declaration_catalogs[idx].as_ref(),
            frontend.file_ids[idx],
            builder.registry_mut(),
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
            builder.register_function(func);
        }
    }

    Ok(())
}

struct LoweredPrograms {
    program_defs: IndexMap<SmolStr, ProgramDef>,
    program_externals: IndexMap<SmolStr, Vec<SmolStr>>,
    globals: Vec<super::compiler::GlobalInit>,
    program_access: Vec<super::compiler::AccessDecl>,
    configuration: Option<super::compiler::ConfigModel>,
}

fn lower_programs_and_configuration(
    frontend: &AnalyzedSourceFiles,
    builder: &mut RuntimeBuilder,
    statement_locations: &mut [Vec<SourceLocation>],
    compile_time_consts: &super::compiler::CompileTimeConsts,
    root_globals_per_file: Vec<Vec<super::compiler::GlobalInit>>,
) -> Result<LoweredPrograms, CompileError> {
    let profile = builder.profile();
    let mut program_defs = IndexMap::<SmolStr, ProgramDef>::new();
    let mut program_externals = IndexMap::new();
    let mut globals = Vec::new();
    let mut program_access = Vec::new();
    for (idx, parse) in frontend.parses.iter().enumerate() {
        let syntax = parse.syntax();
        globals.extend(root_globals_per_file[idx].clone());
        let mut inputs = LoweringInputs::new(
            profile,
            frontend.file_ids[idx].0,
            Some(frontend.project.database()),
            Some(frontend.file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        let lowered = super::lower_programs(
            &syntax,
            frontend.declaration_catalogs[idx].as_ref(),
            frontend.file_ids[idx],
            builder.registry_mut(),
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
            program_externals.insert(SmolStr::new(&key), program.external_names);
            program_defs.insert(key.into(), program.program);
            globals.extend(program.globals);
            program_access.extend(program.access);
        }
    }

    for syntax in &frontend.syntaxes {
        for (owner, inputs) in super::collect_edge_declarations(syntax)? {
            builder.register_edge_inputs(owner, inputs);
        }
    }

    let mut config_model = None;
    for (idx, parse) in frontend.parses.iter().enumerate() {
        let syntax = parse.syntax();
        let mut inputs = LoweringInputs::new(
            profile,
            frontend.file_ids[idx].0,
            Some(frontend.project.database()),
            Some(frontend.file_ids[idx]),
            &mut statement_locations[idx],
            compile_time_consts.clone(),
        );
        if let Some(config) =
            super::lower_configuration(&syntax, builder.registry_mut(), &mut inputs)?
        {
            if config_model.is_some() {
                return Err(CompileError::new(
                    "multiple CONFIGURATION declarations not supported",
                ));
            }
            config_model = Some(config);
        }
    }

    Ok(LoweredPrograms {
        program_defs,
        program_externals,
        globals,
        program_access,
        configuration: config_model,
    })
}

impl LoweredApplication {
    pub(crate) fn has_repeated_program_types(&self) -> bool {
        let Some(config) = &self.configuration else {
            return false;
        };
        let mut types = std::collections::HashSet::new();
        config
            .programs
            .iter()
            .any(|program| !types.insert(program.type_name.to_ascii_uppercase()))
    }

    pub(crate) fn from_builder(
        mut builder: RuntimeBuilder,
        globals: Vec<super::GlobalInit>,
        access: Vec<super::AccessDecl>,
        config_inits: Vec<super::ConfigInit>,
        tasks: Vec<crate::task::TaskConfig>,
        task_bindings: Vec<(SmolStr, Vec<super::AccessPart>)>,
    ) -> Result<Self, CompileError> {
        let mut program_defs = IndexMap::new();
        for program in builder.programs().values() {
            if program_defs
                .insert(
                    SmolStr::new(program.name.to_ascii_uppercase()),
                    program.clone(),
                )
                .is_some()
            {
                return Err(CompileError::new("duplicate builder program name"));
            }
        }
        let programs = program_defs
            .values()
            .map(|program| super::ProgramInstanceConfig {
                name: program.name.clone(),
                type_name: program.name.clone(),
                task: None,
                retain: None,
                fb_tasks: Vec::new(),
            })
            .collect::<Vec<_>>();
        builder.manual_task_bindings = task_bindings;
        let resource_name = Some(builder.resource_name().clone());
        Ok(Self {
            builder,
            program_defs,
            program_externals: IndexMap::new(),
            globals,
            program_access: Vec::new(),
            source_metadata: Vec::new(),
            configuration: Some(super::compiler::ConfigModel {
                configuration_name: None,
                resource_name,
                globals: Vec::new(),
                tasks,
                programs,
                using: Vec::new(),
                access,
                config_inits,
            }),
        })
    }

    /// Resolve configuration identities without creating storage or evaluating startup code.
    pub(crate) fn prepare_authoring(
        self,
        extra_program_instances: &[SmolStr],
    ) -> Result<Self, CompileError> {
        self.prepare_with_program_type_policy(extra_program_instances, false)
    }

    /// Prepare source declarations as context for importing supplied legacy
    /// bodies. The importer can bind several configured instance identities to
    /// one compatible source PROGRAM template without authoring a new source
    /// artifact; ordinary source authoring keeps its stricter duplicate-type rule.
    pub(crate) fn prepare_legacy_import_context(
        self,
        extra_program_instances: &[SmolStr],
    ) -> Result<Self, CompileError> {
        self.prepare_with_program_type_policy(extra_program_instances, true)
    }

    fn prepare_with_program_type_policy(
        mut self,
        extra_program_instances: &[SmolStr],
        allow_repeated_program_types: bool,
    ) -> Result<Self, CompileError> {
        let configured = self.configuration.is_some();
        let mut config = match self.configuration.take() {
            Some(config) => config,
            None => {
                if self.program_defs.is_empty() {
                    return Err(CompileError::new("missing PROGRAM declaration"));
                }
                super::compiler::ConfigModel {
                    configuration_name: None,
                    resource_name: None,
                    globals: Vec::new(),
                    tasks: Vec::new(),
                    using: Vec::new(),
                    access: Vec::new(),
                    config_inits: Vec::new(),
                    programs: self
                        .program_defs
                        .values()
                        .map(|program| super::ProgramInstanceConfig {
                            name: program.name.clone(),
                            type_name: program.name.clone(),
                            task: None,
                            retain: None,
                            fb_tasks: Vec::new(),
                        })
                        .collect(),
                }
            }
        };
        self.globals.append(&mut config.globals);
        apply_program_retain_overrides(&mut self.program_defs, &config.programs, &config.using)?;
        let extras = if configured {
            build_extra_program_instances(
                &self.program_defs,
                &config.programs,
                extra_program_instances,
            )?
        } else {
            Vec::new()
        };
        ensure_all_program_declarations_bound(
            &self.program_defs,
            &config.programs,
            &extras,
            &config.using,
        )?;
        remap_program_access_instances(
            &mut self.program_access,
            &self.program_defs,
            &config.programs,
            &config.using,
        )?;
        config.programs.extend(extras);
        let mut names = std::collections::HashSet::new();
        let mut types = std::collections::HashSet::new();
        for program in &mut config.programs {
            program.type_name = super::resolve_program_type_name(
                &self.program_defs,
                &program.type_name,
                &config.using,
            )?;
            if !names.insert(program.name.to_ascii_uppercase()) {
                return Err(CompileError::new(format!(
                    "duplicate PROGRAM instance name '{}'",
                    program.name
                )));
            }
            if !types.insert(program.type_name.to_ascii_uppercase())
                && !allow_repeated_program_types
            {
                return Err(CompileError::new(
                    "multiple instances of the same PROGRAM type are not supported yet",
                ));
            }
        }
        attach_programs_to_tasks(&mut config.tasks, &config.programs)?;
        self.configuration = Some(config);
        for source in &self.source_metadata {
            self.builder
                .register_statement_locations(source.file_id, source.locations.clone());
            self.builder
                .register_source_text(source.file_id, source.text.clone());
            self.builder.register_source_label(
                source.file_id,
                source
                    .path
                    .clone()
                    .unwrap_or_else(|| format!("file_{}", source.file_id)),
            );
        }
        Ok(self)
    }

    pub(crate) fn materialize(
        self,
        extra_program_instances: &[SmolStr],
    ) -> Result<Runtime, CompileError> {
        let input = self.prepare_authoring(extra_program_instances)?;
        let projected = crate::bytecode::build_module_with_type_projection(&input)
            .map_err(|error| CompileError::new(error.to_string()))?;
        let mut metadata = input.metadata_snapshot(&projected.module)?;
        metadata.artifact_types = projected.artifact_types;
        metadata.io_display_names = projected.io_display_names;
        Runtime::from_module_with_metadata(projected.module, Some(std::sync::Arc::new(metadata)), 0)
            .map_err(|error| CompileError::new(error.to_string()))
    }

    pub(crate) fn metadata_snapshot(
        &self,
        module: &crate::bytecode::BytecodeModule,
    ) -> Result<crate::RuntimeMetadata, CompileError> {
        let config = self
            .configuration
            .as_ref()
            .ok_or_else(|| CompileError::new("authoring configuration not prepared"))?;
        let mut instances = IndexMap::new();
        for binding in &config.programs {
            let mut template = self
                .program_defs
                .values()
                .find(|program| program.name.eq_ignore_ascii_case(&binding.type_name))
                .cloned()
                .ok_or_else(|| CompileError::new("program template missing"))?;
            template.name = binding.name.clone();
            instances.insert(binding.name.clone(), template);
        }
        let metadata = module
            .metadata()
            .map_err(|error| CompileError::new(error.to_string()))?;
        let resource = metadata
            .resources
            .first()
            .ok_or_else(|| CompileError::new("resource missing"))?;
        self.builder
            .metadata_for_configuration(
                self.program_defs.clone(),
                instances,
                resource.tasks.clone(),
            )
            .map_err(|error| CompileError::new(error.to_string()))
    }
}

fn remap_program_access_instances(
    access: &mut [super::compiler::AccessDecl],
    program_defs: &IndexMap<SmolStr, ProgramDef>,
    programs: &[super::ProgramInstanceConfig],
    using: &[SmolStr],
) -> Result<(), CompileError> {
    for declaration in access {
        let super::compiler::AccessPath::Parts(parts) = &mut declaration.path else {
            continue;
        };
        let Some(super::compiler::AccessPart::Name(owner)) = parts.first_mut() else {
            continue;
        };
        for program in programs {
            let type_name =
                super::resolve_program_type_name(program_defs, &program.type_name, using)?;
            if owner.eq_ignore_ascii_case(type_name.as_str()) {
                *owner = program.name.clone();
                break;
            }
        }
    }
    Ok(())
}

fn validate_task_single_bindings(
    runtime: &RuntimeBuilder,
    tasks: &[crate::task::TaskConfig],
) -> Result<(), CompileError> {
    for task in tasks {
        let Some(single) = task.single.as_deref() else {
            continue;
        };
        match runtime.storage().get_global(single) {
            Some(crate::value::Value::Bool(_)) => {}
            Some(_) => {
                return Err(CompileError::new(format!(
                    "TASK '{}' SINGLE variable '{}' must have BOOL type",
                    task.name, single
                )));
            }
            None => {
                return Err(CompileError::new(format!(
                    "TASK '{}' SINGLE refers to unknown variable '{}'",
                    task.name, single
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn build_bytecode_module_from_source_files(
    sources: &[SourceFile],
    label_errors: bool,
    extra_program_instances: &[SmolStr],
) -> Result<crate::bytecode::BytecodeModule, CompileError> {
    let builder = lower_application_with_abi(
        sources,
        label_errors,
        crate::value::StorageAbi::LegacyTemporal32,
    )?
    .materialize_legacy(extra_program_instances)?;
    build_bytecode_module_from_builder_and_sources(&builder, sources)
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

fn build_bytecode_module_from_builder_and_sources(
    builder: &RuntimeBuilder,
    sources: &[SourceFile],
) -> Result<crate::bytecode::BytecodeModule, CompileError> {
    let source_refs = sources
        .iter()
        .map(|source| source.text.as_str())
        .collect::<Vec<_>>();
    let all_paths = sources.iter().all(|source| source.path.is_some());
    let labels = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            if all_paths {
                source.path.clone().unwrap_or_default()
            } else {
                format!("file_{index}")
            }
        })
        .collect::<Vec<_>>();
    let paths = labels.iter().map(String::as_str).collect::<Vec<_>>();
    crate::bytecode::build_legacy_module_from_builder(builder, &source_refs, &paths)
        .map_err(|error| CompileError::new(error.to_string()))
}

fn source_label(source: &SourceFile, idx: usize) -> String {
    source
        .path
        .as_deref()
        .map(|path| path.to_string())
        .unwrap_or_else(|| format!("file {idx}"))
}

#[cfg(test)]
#[path = "config_contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
#[path = "config_access_contract_tests.rs"]
mod access_contract_tests;

#[cfg(test)]
#[path = "authoring_boundary_tests.rs"]
mod authoring_boundary_tests;

/// Historical register/stack fixtures only; never reachable from a product API.
#[cfg(test)]
pub(super) fn build_legacy_fixture(
    sources: &[SourceFile],
    labels: bool,
    extras: &[SmolStr],
) -> Result<crate::runtime::LegacyRuntimeFixture, CompileError> {
    let builder =
        lower_application_with_abi(sources, labels, crate::value::StorageAbi::LegacyTemporal32)?
            .materialize_legacy(extras)?;
    let mut fixture = crate::runtime::LegacyRuntimeFixture::from_authoring_fixture(builder);
    fixture
        .ensure_vm_module_loaded()
        .map_err(|error| CompileError::new(error.to_string()))?;
    Ok(fixture)
}
