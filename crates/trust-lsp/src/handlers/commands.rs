//! LSP workspace/executeCommand handlers.

use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{
    CreateFile, CreateFileOptions, DeleteFile, DeleteFileOptions, DocumentChangeOperation,
    DocumentChanges, ExecuteCommandParams, OptionalVersionedTextDocumentIdentifier, Position,
    Range, ResourceOp, TextDocumentEdit, TextDocumentIdentifier, TextEdit, Url, WorkspaceEdit,
};
use tower_lsp::Client;

use text_size::{TextRange, TextSize};
use trust_ide::refactor::parse_namespace_path;
use trust_ide::rename::{RenameResult, TextEdit as IdeTextEdit};
use trust_runtime::bundle_builder::resolve_sources_root;
use trust_runtime::debug::DebugSnapshot;
use trust_runtime::harness::{CompileSession, SourceFile as HarnessSourceFile};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::handlers::context::ServerContext;
use crate::handlers::lsp_utils::{offset_to_position, position_to_offset};
use crate::library_graph::build_library_graph;
use crate::state::{path_to_uri, uri_to_path, ServerState};

#[path = "commands/control_keep.rs"]
mod control_keep;
#[path = "commands/control_links.rs"]
mod control_links;
#[path = "commands/control_symbols.rs"]
mod control_symbols;
#[path = "commands/hmi_authoring/mod.rs"]
pub(crate) mod hmi_authoring;
#[path = "commands/hmi_catalogue.rs"]
mod hmi_catalogue;
#[path = "commands/hmi_checks.rs"]
mod hmi_checks;
mod hmi_compilation_cache;
#[path = "commands/hmi_composition.rs"]
mod hmi_composition;
mod hmi_equipment;
mod hmi_equipment_io;
#[path = "commands/hmi_faceplates.rs"]
mod hmi_faceplates;
#[path = "commands/hmi_files.rs"]
mod hmi_files;
mod hmi_generate;
mod hmi_own_equipment;
mod hmi_page_proposal;
#[path = "commands/hmi_placed.rs"]
mod hmi_placed;
mod hmi_plant_project;
mod hmi_program_edits;
#[path = "commands/hmi_rename.rs"]
pub(crate) mod hmi_rename;
mod hmi_rename_instance;
mod hmi_screens_program;
mod hmi_snapshot;
mod hmi_symbols;
#[path = "commands/hmi_texts.rs"]
mod hmi_texts;

pub const MOVE_NAMESPACE_COMMAND: &str = "trust-lsp.moveNamespace";
pub const PROJECT_INFO_COMMAND: &str = "trust-lsp.projectInfo";
pub const HMI_BINDINGS_COMMAND: &str = "trust-lsp.hmiBindings";
pub const HMI_FILE_PARSE_COMMAND: &str = "trust-lsp.hmiFileParse";
pub const HMI_FILE_FORMAT_COMMAND: &str = "trust-lsp.hmiFileFormat";
pub const HMI_FILE_CHECK_COMMAND: &str = "trust-lsp.hmiFileCheck";
pub const HMI_CATALOGUE_COMMAND: &str = "trust-lsp.hmiCatalogue";
pub const HMI_BUNDLE_COMMAND: &str = "trust-lsp.hmiBundle";
pub const HMI_TEXTS_COMMAND: &str = "trust-lsp.hmiTexts";
pub const HMI_FACEPLATES_COMMAND: &str = "trust-lsp.hmiFaceplates";
pub const HMI_FACEPLATE_INSTANCES_COMMAND: &str = "trust-lsp.hmiFaceplateInstances";
pub const HMI_PLACED_FACEPLATES_COMMAND: &str = "trust-lsp.hmiPlacedFaceplates";
pub const HMI_SCREENS_PROGRAM_COMMAND: &str = "trust-lsp.hmiScreensProgram";
pub const HMI_EQUIPMENT_COMMAND: &str = "trust-lsp.hmiEquipment";
pub const HMI_OWN_EQUIPMENT_COMMAND: &str = "trust-lsp.hmiOwnEquipment";
pub const HMI_ADD_MISSING_VARIABLES_COMMAND: &str = "trust-lsp.hmiAddMissingVariables";
pub const HMI_PROPOSE_PAGE_COMMAND: &str = "trust-lsp.hmiProposePage";
pub const HMI_PROPOSE_SETTINGS_PAGE_COMMAND: &str = "trust-lsp.hmiProposeSettingsPage";
pub const HMI_RENAME_INSTANCE_COMMAND: &str = "trust-lsp.hmiRenameInstance";
pub const CONTROL_KEEP_PARAMETERS_COMMAND: &str = "trust-lsp.controlKeepParameters";

/// Commands the server advertises through `executeCommandProvider`.
pub const EXECUTE_COMMANDS: &[&str] = &[
    MOVE_NAMESPACE_COMMAND,
    PROJECT_INFO_COMMAND,
    HMI_BINDINGS_COMMAND,
    HMI_FILE_PARSE_COMMAND,
    HMI_FILE_FORMAT_COMMAND,
    HMI_FILE_CHECK_COMMAND,
    HMI_CATALOGUE_COMMAND,
    HMI_BUNDLE_COMMAND,
    HMI_TEXTS_COMMAND,
    HMI_FACEPLATES_COMMAND,
    HMI_FACEPLATE_INSTANCES_COMMAND,
    HMI_PLACED_FACEPLATES_COMMAND,
    HMI_ADD_MISSING_VARIABLES_COMMAND,
    HMI_EQUIPMENT_COMMAND,
    HMI_OWN_EQUIPMENT_COMMAND,
    HMI_SCREENS_PROGRAM_COMMAND,
    HMI_PROPOSE_PAGE_COMMAND,
    HMI_PROPOSE_SETTINGS_PAGE_COMMAND,
    HMI_RENAME_INSTANCE_COMMAND,
    CONTROL_KEEP_PARAMETERS_COMMAND,
];

#[derive(Debug, Deserialize)]
pub struct MoveNamespaceCommandArgs {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    pub new_path: String,
    #[serde(default)]
    pub target_uri: Option<Url>,
    #[serde(default)]
    pub return_edit: bool,
}

#[derive(Debug, Deserialize)]
struct ProjectInfoCommandArgs {
    #[serde(default)]
    root_uri: Option<Url>,
    #[serde(default)]
    text_document: Option<TextDocumentIdentifier>,
}

#[derive(Debug, Deserialize, Default)]
struct HmiBindingsCommandArgs {
    #[serde(default)]
    open: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    root_uri: Option<Url>,
    #[serde(default)]
    text_document: Option<TextDocumentIdentifier>,
}

pub async fn execute_command(
    client: &Client,
    state: &ServerState,
    params: ExecuteCommandParams,
) -> Option<Value> {
    match params.command.as_str() {
        MOVE_NAMESPACE_COMMAND => {
            let args = parse_move_namespace_args(params.arguments)?;
            let return_edit = args.return_edit;
            let edit = namespace_move_workspace_edit(state, args)?;
            if return_edit {
                return Some(json!(edit));
            }
            Some(json!(
                apply_workspace_edit_in_safe_phases(client, edit).await?
            ))
        }
        _ => execute_read_command(state, params),
    }
}

/// CPU-intensive project inspection runs on a blocking worker, never the LSP transport task.
/// These handlers only return a value or proposed edit; the client still applies edits.
pub fn execute_read_command(state: &ServerState, params: ExecuteCommandParams) -> Option<Value> {
    match params.command.as_str() {
        PROJECT_INFO_COMMAND => project_info_value(state, params.arguments),
        HMI_BINDINGS_COMMAND => hmi_bindings_value(state, params.arguments),
        HMI_FILE_PARSE_COMMAND => Some(hmi_files::parse_value(params.arguments)),
        HMI_CATALOGUE_COMMAND => Some(hmi_catalogue::catalogue_value(params.arguments)),
        HMI_TEXTS_COMMAND => Some(hmi_texts::texts_value(params.arguments)),
        HMI_FACEPLATES_COMMAND => Some(hmi_faceplates::faceplates_value(params.arguments)),
        HMI_FACEPLATE_INSTANCES_COMMAND => Some(hmi_placed::instances_value_with_context(
            state,
            params.arguments,
        )),
        HMI_PLACED_FACEPLATES_COMMAND => Some(hmi_placed::placed_value_with_context(
            state,
            params.arguments,
        )),
        HMI_SCREENS_PROGRAM_COMMAND => Some(hmi_screens_program::prepare_project(
            state,
            params.arguments,
        )),
        HMI_EQUIPMENT_COMMAND => Some(hmi_equipment::inspect(state, params.arguments)),
        HMI_OWN_EQUIPMENT_COMMAND => {
            Some(hmi_own_equipment::own_equipment(state, params.arguments))
        }
        HMI_ADD_MISSING_VARIABLES_COMMAND => Some(hmi_program_edits::add_missing_variables_value(
            state,
            params.arguments,
        )),
        HMI_PROPOSE_PAGE_COMMAND => Some(hmi_page_proposal::propose_page_value(
            state,
            params.arguments,
        )),
        HMI_PROPOSE_SETTINGS_PAGE_COMMAND => Some(hmi_page_proposal::propose_settings_page_value(
            state,
            params.arguments,
        )),
        HMI_BUNDLE_COMMAND => Some(hmi_checks::bundle_value_with_context(
            state,
            params.arguments,
        )),
        HMI_RENAME_INSTANCE_COMMAND => Some(hmi_rename_instance::rename_instance(
            state,
            params.arguments,
        )),
        HMI_FILE_FORMAT_COMMAND => Some(hmi_files::format_value(params.arguments)),
        CONTROL_KEEP_PARAMETERS_COMMAND => Some(control_keep::keep(state, params.arguments)),
        HMI_FILE_CHECK_COMMAND => Some(hmi_checks::check_value_with_context(
            state,
            params.arguments,
        )),
        _ => None,
    }
}

async fn apply_workspace_edit_in_safe_phases(client: &Client, edit: WorkspaceEdit) -> Option<bool> {
    for phase in split_workspace_edit_for_safe_apply(edit) {
        let response = client.apply_edit(phase).await.ok()?;
        if !response.applied {
            return Some(false);
        }
    }
    Some(true)
}

fn split_workspace_edit_for_safe_apply(edit: WorkspaceEdit) -> Vec<WorkspaceEdit> {
    let WorkspaceEdit {
        changes,
        document_changes,
        change_annotations,
    } = edit;

    let Some(DocumentChanges::Operations(operations)) = document_changes else {
        return vec![WorkspaceEdit {
            changes,
            document_changes,
            change_annotations,
        }];
    };
    if changes.is_some() || change_annotations.is_some() {
        return vec![WorkspaceEdit {
            changes,
            document_changes: Some(DocumentChanges::Operations(operations)),
            change_annotations,
        }];
    }

    let mut apply_ops = Vec::new();
    let mut cleanup_ops = Vec::new();

    for operation in operations {
        match &operation {
            DocumentChangeOperation::Op(ResourceOp::Create(_))
            | DocumentChangeOperation::Op(ResourceOp::Rename(_))
            | DocumentChangeOperation::Edit(_) => apply_ops.push(operation),
            DocumentChangeOperation::Op(ResourceOp::Delete(_)) => cleanup_ops.push(operation),
        }
    }

    [apply_ops, cleanup_ops]
        .into_iter()
        .filter(|operations| !operations.is_empty())
        .map(|operations| WorkspaceEdit {
            changes: None,
            document_changes: Some(DocumentChanges::Operations(operations)),
            change_annotations: None,
        })
        .collect()
}

fn parse_move_namespace_args(args: Vec<Value>) -> Option<MoveNamespaceCommandArgs> {
    if args.len() != 1 {
        return None;
    }
    serde_json::from_value(args.into_iter().next()?).ok()
}

pub(crate) fn project_info_value(state: &ServerState, args: Vec<Value>) -> Option<Value> {
    project_info_value_with_context(state, args)
}

fn project_info_value_with_context<C: ServerContext>(
    context: &C,
    args: Vec<Value>,
) -> Option<Value> {
    let mut configs = context.workspace_configs();
    if args.len() == 1 {
        if let Ok(parsed) = serde_json::from_value::<ProjectInfoCommandArgs>(
            args.into_iter().next().unwrap_or(Value::Null),
        ) {
            if let Some(root_uri) = parsed.root_uri {
                configs.retain(|(root, _)| root == &root_uri);
            } else if let Some(text_document) = parsed.text_document {
                if let Some(config) = context.workspace_config_for_uri(&text_document.uri) {
                    let root_uri = path_to_uri(&config.root).unwrap_or(text_document.uri.clone());
                    configs = vec![(root_uri, config)];
                }
            }
        }
    }

    let projects: Vec<Value> = configs
        .into_iter()
        .map(|(root, config)| project_info_for_config(&root, &config))
        .collect();

    Some(json!({ "projects": projects }))
}

fn project_info_for_config(root: &Url, config: &crate::config::ProjectConfig) -> Value {
    let graph = build_library_graph(config);
    let libraries: Vec<Value> = graph
        .nodes
        .into_iter()
        .map(|node| {
            let dependencies: Vec<Value> = node
                .dependencies
                .into_iter()
                .map(|dep| {
                    json!({
                        "name": dep.name,
                        "version": dep.version,
                    })
                })
                .collect();
            json!({
                "name": node.name,
                "version": node.version,
                "path": node.path.display().to_string(),
                "dependencies": dependencies,
            })
        })
        .collect();

    let targets: Vec<Value> = config
        .targets
        .iter()
        .map(|target| {
            json!({
                "name": target.name,
                "profile": target.profile,
                "flags": target.flags,
                "defines": target.defines,
            })
        })
        .collect();

    json!({
        "root": root.to_string(),
        "configPath": config.config_path.as_ref().map(|path| path.display().to_string()),
        "build": {
            "target": config.build.target,
            "profile": config.build.profile,
            "flags": config.build.flags,
            "defines": config.build.defines,
        },
        "targets": targets,
        "libraries": libraries,
    })
}

#[derive(Debug, Clone)]
struct LoadedSource {
    path: PathBuf,
    text: String,
}

pub(crate) fn hmi_bindings_value(state: &ServerState, args: Vec<Value>) -> Option<Value> {
    hmi_bindings_value_with_context(state, args)
}

fn hmi_bindings_value_with_context<C: ServerContext>(
    context: &C,
    args: Vec<Value>,
) -> Option<Value> {
    let authoring = args
        .first()
        .and_then(|value| value.get("authoring"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let parsed = match parse_hmi_bindings_args(args) {
        Ok(parsed) => parsed,
        Err(error) => return Some(json!({ "ok": false, "error": error })),
    };

    let inputs = match project_inputs(context, &parsed, HMI_BINDINGS_COMMAND) {
        Ok(inputs) => inputs,
        Err(error) => return Some(json!({ "ok": false, "error": error })),
    };
    if authoring {
        let declarations = hmi_authoring::declarations::Declarations::read(&inputs.sources);
        let variables = match hmi_authoring::design_variables::json(&inputs, declarations.as_ref())
        {
            Ok(variables) => variables,
            Err(error) => return Some(json!({"ok":false,"error":error})),
        };
        return Some(
            json!({"ok":true,"sourceMode":"editor-overlay","inspectionMode":"declarations","sourceRevision":source_revision(&inputs.sources),"variables":variables,
            "alarms":[],"alarmsStatus":"Runtime alarm messages are available in Live mode"}),
        );
    }
    let cache_key =
        hmi_compilation_cache::bindings_key(&inputs.root, &inputs.sources_root, &inputs.sources);
    if let Some(value) = cache_key
        .as_ref()
        .and_then(hmi_compilation_cache::read_bindings)
    {
        return Some(value);
    }
    let CompiledProject {
        root: project_root,
        sources_root,
        sources,
        runtime,
        ..
    } = match compile_inputs(inputs) {
        Ok(project) => project,
        Err(error) => return Some(json!({ "ok": false, "error": error })),
    };

    let metadata = runtime.metadata_snapshot();
    let snapshot = DebugSnapshot {
        storage: runtime.storage().clone(),
        now: runtime.current_time(),
    };
    let texts = sources
        .iter()
        .map(|source| source.text.as_str())
        .collect::<Vec<_>>();
    // Compiled storage names a PROGRAM instance after itself; report the type its configuration declares.
    let configured = control_links::program_types(&texts);
    let configured_type = |instance: &str| {
        configured
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(instance))
            .map(|(_, kind)| kind.clone())
    };
    // The HMI Builder's variable picker (specification 39, §7.1).
    let (variables, variables_error) =
        match trust_runtime::hmi::variable_tree::variable_tree(&metadata, &snapshot.storage) {
            Ok(mut tree) => {
                for root in &mut tree {
                    if root.kind == trust_runtime::hmi::variable_tree::VariableKind::Program {
                        if let Some(kind) = configured_type(&root.name) {
                            root.type_name = kind;
                        }
                    }
                }
                (json!(tree), Value::Null)
            }
            Err(error) => (json!([]), json!(error)),
        };
    let io_snapshot = runtime.io().snapshot();
    let mut commands = plant_signal_entries(&io_snapshot.outputs);
    let mut feedbacks = plant_signal_entries(&io_snapshot.inputs);
    let mut memory = plant_signal_entries(&io_snapshot.memory);
    commands.sort_by(plant_signal_order);
    feedbacks.sort_by(plant_signal_order);
    memory.sort_by(plant_signal_order);
    let addressed = commands
        .iter()
        .chain(&feedbacks)
        .chain(&memory)
        .cloned()
        .collect::<Vec<_>>();
    let mut control_symbols = control_symbols::catalog(&metadata, &snapshot, &addressed)
        .unwrap_or_else(|error| json!({ "error": error }));
    if let Some(controllers) = control_symbols["controllers"].as_array_mut() {
        for entry in controllers.iter_mut() {
            if let Some(kind) = entry["instance"].as_str().and_then(configured_type) {
                entry["type"] = json!(kind);
            }
        }
    }
    if let Some(controllers) = control_symbols["controllers"].as_array() {
        let instances = controllers
            .iter()
            .filter_map(|entry| {
                Some((
                    entry["instance"].as_str()?.to_string(),
                    entry["type"].as_str()?.to_string(),
                ))
            })
            .collect::<Vec<_>>();
        let facts = control_links::facts(&texts, &instances);
        control_symbols["links"] = json!(facts.links);
        control_symbols["writes"] = json!(facts.writes);
    }
    // Dynamic alarm text belongs to the running PLC. Metadata discovery must never execute
    // a scan merely to discover text (including callers outside the visual builder).
    let alarms: Vec<Value> = Vec::new();

    // Bind authoring context to all compiled text, including open editor overlays and packages.
    let source_revision = source_revision(&sources);
    let value = json!({
        "sourceRevision": source_revision,
        "sourceMode": "editor-overlay",
        "ok": true,
        "command": HMI_BINDINGS_COMMAND,
        "root": project_root.display().to_string(),
        "sourcesRoot": sources_root.display().to_string(),
        "controlSymbols": control_symbols,
        "variables": variables,
        "variablesError": variables_error,
        "alarms": alarms,
        "processImage": {
            "commands": commands,
            "feedbacks": feedbacks,
            "memory": memory,
        },
    });
    if let Some(key) = cache_key {
        hmi_compilation_cache::store_bindings(key, value.clone());
    }
    Some(value)
}

fn source_revision(sources: &[LoadedSource]) -> String {
    use sha2::{Digest, Sha256};
    let mut ordered = sources.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.path.cmp(&right.path));
    let mut digest = Sha256::new();
    for source in ordered {
        digest.update(source.path.to_string_lossy().as_bytes());
        digest.update([0]);
        digest.update(source.text.as_bytes());
        digest.update([0]);
    }
    format!("{:x}", digest.finalize())
}

fn plant_signal_entries(entries: &[trust_runtime::io::IoSnapshotEntry]) -> Vec<Value> {
    entries
        .iter()
        .filter(|entry| !entry.address.wildcard)
        .filter_map(|entry| {
            let value_type = entry
                .value_type
                .and_then(trust_runtime::io::io_value_type_name)?;
            if !matches!(
                value_type,
                "SINT"
                    | "INT"
                    | "DINT"
                    | "LINT"
                    | "USINT"
                    | "UINT"
                    | "UDINT"
                    | "ULINT"
                    | "REAL"
                    | "LREAL"
            ) {
                return None;
            }
            Some(json!({
                "symbol": entry.name.as_ref()?.as_str(),
                "address": canonical_io_address(&entry.address),
                "type": value_type,
            }))
        })
        .collect()
}

fn plant_signal_order(left: &Value, right: &Value) -> std::cmp::Ordering {
    plant_signal_field(left, "address")
        .cmp(plant_signal_field(right, "address"))
        .then_with(|| plant_signal_field(left, "symbol").cmp(plant_signal_field(right, "symbol")))
}

fn plant_signal_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn canonical_io_address(address: &trust_runtime::io::IoAddress) -> String {
    use trust_runtime::io::IoSize;
    let area = match address.area {
        trust_runtime::memory::IoArea::Input => "I",
        trust_runtime::memory::IoArea::Output => "Q",
        trust_runtime::memory::IoArea::Memory => "M",
    };
    let size = match address.size {
        IoSize::Bit => "X",
        IoSize::Byte | IoSize::Bytes(_) => "B",
        IoSize::Word => "W",
        IoSize::DWord => "D",
        IoSize::LWord => "L",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    let mut path = address
        .path
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".");
    if address.size == IoSize::Bit {
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(&address.bit.to_string());
    }
    if path.is_empty() {
        format!("%{area}{size}{}", address.byte)
    } else {
        format!("%{area}{size}{path}")
    }
}

fn parse_hmi_bindings_args(args: Vec<Value>) -> Result<HmiBindingsCommandArgs, String> {
    match args.len() {
        0 => Ok(HmiBindingsCommandArgs::default()),
        1 => serde_json::from_value(args.into_iter().next().unwrap_or(Value::Null))
            .map_err(|error| format!("invalid trust-lsp.hmiBindings arguments: {error}")),
        _ => Err("trust-lsp.hmiBindings expects zero or one argument object".to_string()),
    }
}

/// A project compiled for the HMI commands.
pub(super) struct CompiledProject {
    root: PathBuf,
    sources_root: PathBuf,
    sources: Vec<LoadedSource>,
    runtime: hmi_compilation_cache::CachedRuntime,
    open: std::collections::BTreeMap<PathBuf, String>,
}

/// Selects the project as `trust-lsp.hmiBindings` does (root URI, then the document's project,
/// then the first workspace) and compiles it.
fn compile_project<C: ServerContext>(
    context: &C,
    selection: &HmiBindingsCommandArgs,
    command: &str,
) -> Result<CompiledProject, String> {
    compile_inputs(project_inputs(context, selection, command)?)
}

struct ProjectInputs {
    root: PathBuf,
    sources_root: PathBuf,
    sources: Vec<LoadedSource>,
    open: std::collections::BTreeMap<PathBuf, String>,
}

fn project_inputs<C: ServerContext>(
    context: &C,
    selection: &HmiBindingsCommandArgs,
    command: &str,
) -> Result<ProjectInputs, String> {
    let root = resolve_hmi_bindings_project_root(context, selection)
        .ok_or_else(|| format!("unable to resolve workspace root for {command}"))?;
    let open = hmi_snapshot::paths(&root, &selection.open)?;
    let (sources_root, sources) = load_hmi_sources_edited(root.as_path(), context, &open)?;
    Ok(ProjectInputs {
        root,
        sources_root,
        sources,
        open,
    })
}

fn compile_inputs(inputs: ProjectInputs) -> Result<CompiledProject, String> {
    let _trace = trust_hir::timing::CompilerStage::new("hmi.compile_project", inputs.sources.len());
    trust_hir::timing::event(
        "source_bytes",
        inputs.sources.iter().map(|source| source.text.len()).sum(),
    );
    let ProjectInputs {
        root,
        sources_root,
        sources,
        open,
    } = inputs;
    let compile_sources = sources
        .iter()
        .map(|source| {
            HarnessSourceFile::with_path(
                source.path.to_string_lossy().as_ref(),
                source.text.clone(),
            )
        })
        .collect::<Vec<_>>();
    let runtime = hmi_compilation_cache::compile(&root, &sources, || {
        CompileSession::from_sources(compile_sources)
            .build_runtime()
            .map_err(|error| error.to_string())
    })?;
    Ok(CompiledProject {
        root,
        sources_root,
        sources,
        runtime,
        open,
    })
}

fn resolve_hmi_bindings_project_root(
    context: &impl ServerContext,
    args: &HmiBindingsCommandArgs,
) -> Option<PathBuf> {
    if let Some(root_uri) = &args.root_uri {
        return uri_to_path(root_uri);
    }

    if let Some(text_document) = &args.text_document {
        if let Some(config) = context.workspace_config_for_uri(&text_document.uri) {
            return Some(config.root);
        }
        let doc_path = uri_to_path(&text_document.uri)?;
        if doc_path.is_dir() {
            return Some(doc_path);
        }
        return doc_path.parent().map(Path::to_path_buf);
    }

    if let Some((_root_uri, config)) = context.workspace_configs().into_iter().next() {
        return Some(config.root);
    }

    context
        .workspace_folders()
        .into_iter()
        .next()
        .and_then(|uri| uri_to_path(&uri))
}

fn load_hmi_sources_edited<C: ServerContext>(
    root: &Path,
    context: &C,
    open: &std::collections::BTreeMap<PathBuf, String>,
) -> Result<(PathBuf, Vec<LoadedSource>), String> {
    let sources_root = if root.join("src").is_dir() {
        resolve_sources_root(root, None).map_err(|error| error.to_string())?
    } else {
        root.join("src")
    };
    // The same collection the runtime build uses: project sources plus declared local package dependencies.
    let collected =
        hmi_authoring::sources::collect(root, open).map_err(|error| error.to_string())?;
    let mut sources = Vec::with_capacity(collected.len());
    for source in collected {
        let path = source
            .path
            .map(PathBuf::from)
            .unwrap_or_else(|| sources_root.clone());
        // Open editor text is authoritative, including compilation errors and opt-out headers.
        // Closed-document cache entries must not override a subsequently changed disk file.
        let text = open
            .get(&path)
            .cloned()
            .or_else(|| {
                Url::from_file_path(&path)
                    .ok()
                    .and_then(|uri| context.get_document(&uri))
                    .filter(|document| document.is_open)
                    .map(|document| document.content)
            })
            .unwrap_or(source.text);
        if text
            .lines()
            .take(20)
            .any(|line| line.contains("@trustlsp:runtime-ignore"))
        {
            continue;
        }
        sources.push(LoadedSource { path, text });
    }

    Ok((sources_root, sources))
}

pub(crate) fn namespace_move_workspace_edit(
    state: &ServerState,
    args: MoveNamespaceCommandArgs,
) -> Option<WorkspaceEdit> {
    namespace_move_workspace_edit_with_context(state, args)
}

include!("commands/hmi_namespace_and_edits.rs");
include!("commands/path_ranges_and_tests.rs");
