mod construction;

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::debug::SourceLocation;
use crate::task::ProgramDef;
use crate::RuntimeBuilder;
use std::{path::Path, sync::Arc};
use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::{semantic::DeclarationCatalog, Dialect, Project, SourceKey};
use trust_syntax::{parser, SyntaxNode};

use super::compiler::{lower_root_global_var_blocks, LoweringInputs};

use super::config::attach_programs_to_tasks;
use super::types::{CompileError, SourceFile};
use crate::stdlib::conversions::ConversionProfile;

/// Lowered declarations and original initialization expressions, before any storage is constructed.
/// The program image's build reads them with the sources' syntax trees (specification 44 §3), and the
/// declarations view constructs the starting values from them for the tools.
pub(crate) struct LoweredApplication {
    pub(crate) builder: RuntimeBuilder,
    pub(crate) program_defs: IndexMap<SmolStr, ProgramDef>,
    pub(crate) globals: Vec<super::compiler::GlobalInit>,
    pub(crate) program_access: Vec<super::compiler::AccessDecl>,
    pub(crate) configuration: Option<super::compiler::ConfigModel>,
    pub(crate) source_metadata: Vec<LoweredSourceMetadata>,
    /// The `TEST_PROGRAM` declarations, by upper-case name: test entry points that only the test
    /// runner instantiates (specification 22).
    pub(crate) test_programs: std::collections::BTreeSet<String>,
}

pub(crate) struct LoweredSourceMetadata {
    pub(crate) file_id: u32,
    pub(crate) locations: Vec<SourceLocation>,
}

#[cfg(test)]
pub(crate) fn lower_application(
    sources: &[SourceFile],
    label_errors: bool,
) -> Result<LoweredApplication, CompileError> {
    lower_application_with(sources, label_errors, ConversionProfile::Iec)
}

/// [`lower_application`] for a project with the conversion rules `conversions`, checked under the
/// language rules of the same vendor profile.
pub(crate) fn lower_application_with(
    sources: &[SourceFile],
    label_errors: bool,
    conversions: ConversionProfile,
) -> Result<LoweredApplication, CompileError> {
    lower_application_and_frontend(sources, label_errors, true, conversions)
        .map(|(lowered, _)| lowered)
}

/// The language rules of the vendor profile whose conversion rules are `conversions`: the
/// project's `vendor_profile` selects both (specification 07 §2.10, specification 14), so a
/// program is never checked under one profile and converted under the other.
pub(crate) fn dialect_of(conversions: ConversionProfile) -> Dialect {
    match conversions {
        ConversionProfile::Iec => Dialect::Iec,
        ConversionProfile::Codesys => Dialect::Codesys,
    }
}

/// The checked semantic model of a project's sources: the project database, each source's file id
/// and syntax tree, in source order.
pub(crate) struct AnalyzedSourceFiles {
    pub(crate) project: Project,
    pub(crate) file_ids: Vec<FileId>,
    parses: Vec<parser::Parse>,
    declaration_catalogs: Vec<Arc<DeclarationCatalog>>,
    pub(crate) syntaxes: Vec<SyntaxNode>,
}

fn analyze_source_files(
    sources: &[SourceFile],
    label_errors: bool,
    dialect: Dialect,
) -> Result<AnalyzedSourceFiles, CompileError> {
    let parsing = trust_hir::timing::CompilerStage::new("compiler.parse", sources.len());
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

    drop(parsing);
    let _analysis =
        trust_hir::timing::CompilerStage::new("compiler.semantic_analysis", sources.len());
    let mut project = Project::new();
    project.database_mut().set_dialect(dialect);
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

/// The lowered declarations of `sources` together with the semantic model they were lowered from,
/// which the compiled-program generator reads the statements' types and resolved names from
/// (specification 44 §3). The POUs' statements are lowered only when `lower_bodies` is set: the
/// compiled-program build generates them from the syntax tree.
pub(crate) fn lower_application_and_frontend(
    sources: &[SourceFile],
    label_errors: bool,
    lower_bodies: bool,
    conversions: ConversionProfile,
) -> Result<(LoweredApplication, AnalyzedSourceFiles), CompileError> {
    let frontend = analyze_source_files(sources, label_errors, dialect_of(conversions))?;

    let _lowering =
        trust_hir::timing::CompilerStage::new("compiler.lower_and_initialize", sources.len());
    let mut builder = RuntimeBuilder::new();
    let mut statement_locations: Vec<Vec<SourceLocation>> = vec![Vec::new(); sources.len()];
    let (compile_time_consts, root_globals_per_file) =
        lower_project_types(&frontend, &mut builder, &mut statement_locations)?;
    lower_named_declarations(
        &frontend,
        &mut builder,
        &mut statement_locations,
        &compile_time_consts,
        lower_bodies,
    )?;
    let lowered_programs = lower_programs_and_configuration(
        &frontend,
        &mut builder,
        &mut statement_locations,
        &compile_time_consts,
        root_globals_per_file,
        lower_bodies,
    )?;
    let source_metadata = statement_locations
        .into_iter()
        .enumerate()
        .map(|(idx, locations)| LoweredSourceMetadata {
            file_id: frontend.file_ids[idx].0,
            locations,
        })
        .collect();
    let test_programs = test_program_names(&frontend.syntaxes)?;
    Ok((
        LoweredApplication {
            builder,
            program_defs: lowered_programs.program_defs,
            globals: lowered_programs.globals,
            program_access: lowered_programs.program_access,
            configuration: lowered_programs.configuration,
            source_metadata,
            test_programs,
        },
        frontend,
    ))
}

/// The upper-case names of the sources' `TEST_PROGRAM` declarations.
fn test_program_names(
    syntaxes: &[SyntaxNode],
) -> Result<std::collections::BTreeSet<String>, CompileError> {
    let mut names = std::collections::BTreeSet::new();
    for node in syntaxes.iter().flat_map(SyntaxNode::descendants) {
        let test = node.kind() == trust_syntax::SyntaxKind::Program
            && node
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .find(|token| !token.kind().is_trivia())
                .is_some_and(|token| token.kind() == trust_syntax::SyntaxKind::KwTestProgram);
        if test {
            names.insert(super::compiler::qualified_pou_name(&node)?.to_ascii_uppercase());
        }
    }
    Ok(names)
}

fn lower_project_types(
    frontend: &AnalyzedSourceFiles,
    builder: &mut RuntimeBuilder,
    statement_locations: &mut [Vec<SourceLocation>],
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
    )?;
    // docs/IEC_DEVIATIONS.md DEV-033: plain initializer-backed globals act as
    // declaration sizes. Evaluated once against the true constants and
    // absorbed into every later constant set.
    let declared_sizes = super::resolve_project_declared_sizes(
        &frontend.syntaxes,
        builder.registry_mut(),
        profile,
        frontend.project.database(),
        &frontend.file_ids,
        statement_locations,
        &preliminary_consts,
    );
    let mut preliminary_consts = preliminary_consts;
    preliminary_consts.absorb_declared_sizes(declared_sizes.clone());

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
    super::add_struct_bases(&frontend.syntaxes, builder.registry_mut())?;
    super::validate_project_aliases(&type_ids, builder.registry())?;

    let mut compile_time_consts = super::resolve_project_global_constants(
        &frontend.syntaxes,
        builder.registry_mut(),
        profile,
        frontend.project.database(),
        &frontend.file_ids,
        statement_locations,
        false,
    )?;
    compile_time_consts.absorb_declared_sizes(declared_sizes.clone());
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
    lower_bodies: bool,
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
        inputs.lower_bodies = lower_bodies;
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
        inputs.lower_bodies = lower_bodies;
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
        inputs.lower_bodies = lower_bodies;
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
        inputs.lower_bodies = lower_bodies;
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
    lower_bodies: bool,
) -> Result<LoweredPrograms, CompileError> {
    let profile = builder.profile();
    let mut program_defs = IndexMap::<SmolStr, ProgramDef>::new();
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
        inputs.lower_bodies = lower_bodies;
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
        inputs.lower_bodies = lower_bodies;
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
        globals,
        program_access,
        configuration: config_model,
    })
}

impl LoweredApplication {
    /// Resolve configuration identities without creating storage or evaluating startup code.
    pub(crate) fn prepare_authoring(
        mut self,
        extra_program_instances: &[SmolStr],
    ) -> Result<Self, CompileError> {
        let configured = self.configuration.is_some();
        let mut config = match self.configuration.take() {
            Some(config) => config,
            None => {
                if self.program_defs.is_empty() {
                    return Err(CompileError::new("missing PROGRAM declaration"));
                }
                super::compiler::ConfigModel {
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
                            connections: Vec::new(),
                        })
                        .collect(),
                }
            }
        };
        self.globals.append(&mut config.globals);
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
            &self.test_programs,
        )?;
        config.programs.extend(extras);
        // A program type may have several instances, each with its own variables and its own
        // `RETAIN` or `NON_RETAIN`, which the layout applies (IEC 61131-3 §6.8.2, §6.5.6.2).
        let mut names = std::collections::HashSet::new();
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
        }
        attach_programs_to_tasks(&mut config.tasks, &config.programs)?;
        self.configuration = Some(config);
        for source in &self.source_metadata {
            self.builder
                .register_statement_locations(source.file_id, source.locations.clone());
        }
        Ok(self)
    }
}

/// The access paths the programs declare, one for each instance of the program that declares it
/// (IEC 61131-3 §6.5.2.2: an access path names a variable of the program, so each instance has
/// its own): named `<instance>.<name>`, and by its own name too when the program has one instance.
fn instance_program_access(
    access: Vec<super::compiler::AccessDecl>,
    program_defs: &IndexMap<SmolStr, ProgramDef>,
    programs: &[super::ProgramInstanceConfig],
    using: &[SmolStr],
) -> Result<Vec<super::compiler::AccessDecl>, CompileError> {
    let mut types = Vec::with_capacity(programs.len());
    for program in programs {
        types.push(super::resolve_program_type_name(
            program_defs,
            &program.type_name,
            using,
        )?);
    }
    let mut instanced = Vec::with_capacity(access.len());
    for declaration in access {
        let owner = match &declaration.path {
            super::compiler::AccessPath::Parts(parts) => match parts.first() {
                Some(super::compiler::AccessPart::Name(owner)) => Some(owner.clone()),
                _ => None,
            },
            super::compiler::AccessPath::Direct { .. } => None,
        };
        let Some(owner) = owner else {
            instanced.push(declaration);
            continue;
        };
        let instances: Vec<&SmolStr> = programs
            .iter()
            .zip(&types)
            .filter(|(_, type_name)| type_name.eq_ignore_ascii_case(owner.as_str()))
            .map(|(program, _)| &program.name)
            .collect();
        for instance in &instances {
            let mut own = declaration.clone();
            if let super::compiler::AccessPath::Parts(parts) = &mut own.path {
                parts[0] = super::compiler::AccessPart::Name((*instance).clone());
            }
            if instances.len() == 1 {
                instanced.push(own.clone());
            }
            own.name = SmolStr::new(format!("{instance}.{}", declaration.name));
            instanced.push(own);
        }
    }
    Ok(instanced)
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
            connections: Vec::new(),
        });
    }

    Ok(extra_programs)
}

/// Every ordinary `PROGRAM` under a configuration must be bound to a resource. A `TEST_PROGRAM` is
/// a test entry point, not part of the production program: a production build leaves it
/// unbound, and only the test runner registers it (specification 22).
fn ensure_all_program_declarations_bound(
    program_defs: &IndexMap<SmolStr, ProgramDef>,
    configured_programs: &[super::ProgramInstanceConfig],
    extra_programs: &[super::ProgramInstanceConfig],
    using: &[SmolStr],
    test_programs: &std::collections::BTreeSet<String>,
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
        .filter(|program| {
            let name = program.name.to_ascii_uppercase();
            !bound_types.contains(&name) && !test_programs.contains(&name)
        })
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
