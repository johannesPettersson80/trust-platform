//! LSP workspace/executeCommand handlers.

use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
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
use trust_runtime::hmi::{self as runtime_hmi, HmiSourceRef};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::handlers::context::ServerContext;
use crate::handlers::lsp_utils::{offset_to_position, position_to_offset};
use crate::library_graph::build_library_graph;
use crate::state::{path_to_uri, uri_to_path, ServerState};

pub const MOVE_NAMESPACE_COMMAND: &str = "trust-lsp.moveNamespace";
pub const PROJECT_INFO_COMMAND: &str = "trust-lsp.projectInfo";
pub const HMI_INIT_COMMAND: &str = "trust-lsp.hmiInit";
pub const HMI_BINDINGS_COMMAND: &str = "trust-lsp.hmiBindings";

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
struct HmiInitCommandArgs {
    #[serde(default)]
    style: Option<String>,
    #[serde(default)]
    root_uri: Option<Url>,
    #[serde(default)]
    text_document: Option<TextDocumentIdentifier>,
}

#[derive(Debug, Deserialize, Default)]
struct HmiBindingsCommandArgs {
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
        PROJECT_INFO_COMMAND => project_info_value(state, params.arguments),
        HMI_INIT_COMMAND => hmi_init_value(state, params.arguments),
        HMI_BINDINGS_COMMAND => hmi_bindings_value(state, params.arguments),
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

pub(crate) fn hmi_init_value(state: &ServerState, args: Vec<Value>) -> Option<Value> {
    hmi_init_value_with_context(state, args)
}

pub(crate) fn hmi_bindings_value(state: &ServerState, args: Vec<Value>) -> Option<Value> {
    hmi_bindings_value_with_context(state, args)
}

fn hmi_init_value_with_context<C: ServerContext>(context: &C, args: Vec<Value>) -> Option<Value> {
    let parsed = match parse_hmi_init_args(args) {
        Ok(parsed) => parsed,
        Err(error) => return Some(json!({ "ok": false, "error": error })),
    };

    let style = match normalize_hmi_style(parsed.style.as_deref()) {
        Ok(style) => style,
        Err(error) => return Some(json!({ "ok": false, "error": error })),
    };

    let project_root = match resolve_hmi_project_root(context, &parsed) {
        Some(root) => root,
        None => {
            return Some(json!({
                "ok": false,
                "error": "unable to resolve workspace root for trust-lsp.hmiInit",
            }));
        }
    };

    let (sources_root, sources) = match load_hmi_sources(project_root.as_path()) {
        Ok(loaded) => loaded,
        Err(error) => return Some(json!({ "ok": false, "error": error })),
    };

    let compile_sources = sources
        .iter()
        .map(|source| {
            HarnessSourceFile::with_path(
                source.path.to_string_lossy().as_ref(),
                source.text.clone(),
            )
        })
        .collect::<Vec<_>>();

    let runtime = match CompileSession::from_sources(compile_sources).build_runtime() {
        Ok(runtime) => runtime,
        Err(error) => return Some(json!({ "ok": false, "error": error.to_string() })),
    };

    let metadata = runtime.metadata_snapshot();
    let snapshot = DebugSnapshot {
        storage: runtime.storage().clone(),
        now: runtime.current_time(),
    };
    let source_refs = sources
        .iter()
        .map(|source| HmiSourceRef {
            path: source.path.as_path(),
            text: source.text.as_str(),
        })
        .collect::<Vec<_>>();

    let summary = match runtime_hmi::scaffold_hmi_dir_with_sources(
        project_root.as_path(),
        &metadata,
        Some(&snapshot),
        &source_refs,
        style.as_str(),
    ) {
        Ok(summary) => summary,
        Err(error) => return Some(json!({ "ok": false, "error": error.to_string() })),
    };

    Some(json!({
        "ok": true,
        "command": HMI_INIT_COMMAND,
        "root": project_root.display().to_string(),
        "sourcesRoot": sources_root.display().to_string(),
        "style": style,
        "summaryText": summary.render_text(),
        "files": summary.files,
    }))
}

fn hmi_bindings_value_with_context<C: ServerContext>(
    context: &C,
    args: Vec<Value>,
) -> Option<Value> {
    let parsed = match parse_hmi_bindings_args(args) {
        Ok(parsed) => parsed,
        Err(error) => return Some(json!({ "ok": false, "error": error })),
    };

    let project_root = match resolve_hmi_bindings_project_root(context, &parsed) {
        Some(root) => root,
        None => {
            return Some(json!({
                "ok": false,
                "error": "unable to resolve workspace root for trust-lsp.hmiBindings",
            }));
        }
    };

    let (sources_root, sources) = match load_hmi_sources(project_root.as_path()) {
        Ok(loaded) => loaded,
        Err(error) => return Some(json!({ "ok": false, "error": error })),
    };

    let compile_sources = sources
        .iter()
        .map(|source| {
            HarnessSourceFile::with_path(
                source.path.to_string_lossy().as_ref(),
                source.text.clone(),
            )
        })
        .collect::<Vec<_>>();

    let runtime = match CompileSession::from_sources(compile_sources).build_runtime() {
        Ok(runtime) => runtime,
        Err(error) => return Some(json!({ "ok": false, "error": error.to_string() })),
    };

    let metadata = runtime.metadata_snapshot();
    let snapshot = DebugSnapshot {
        storage: runtime.storage().clone(),
        now: runtime.current_time(),
    };
    let source_refs = sources
        .iter()
        .map(|source| HmiSourceRef {
            path: source.path.as_path(),
            text: source.text.as_str(),
        })
        .collect::<Vec<_>>();
    let bindings =
        runtime_hmi::collect_hmi_bindings_catalog(&metadata, Some(&snapshot), &source_refs);

    Some(json!({
        "ok": true,
        "command": HMI_BINDINGS_COMMAND,
        "root": project_root.display().to_string(),
        "sourcesRoot": sources_root.display().to_string(),
        "programs": bindings.programs,
        "globals": bindings.globals,
    }))
}

fn parse_hmi_init_args(args: Vec<Value>) -> Result<HmiInitCommandArgs, String> {
    match args.len() {
        0 => Ok(HmiInitCommandArgs::default()),
        1 => serde_json::from_value(args.into_iter().next().unwrap_or(Value::Null))
            .map_err(|error| format!("invalid trust-lsp.hmiInit arguments: {error}")),
        _ => Err("trust-lsp.hmiInit expects zero or one argument object".to_string()),
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

fn normalize_hmi_style(style: Option<&str>) -> Result<String, String> {
    let raw = style.unwrap_or("industrial");
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok("industrial".to_string());
    }
    match normalized.as_str() {
        "industrial" | "classic" | "mint" => Ok(normalized),
        _ => Err(format!(
            "invalid style '{raw}', expected one of: industrial, classic, mint"
        )),
    }
}

fn resolve_hmi_project_root(
    context: &impl ServerContext,
    args: &HmiInitCommandArgs,
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

fn load_hmi_sources(root: &Path) -> Result<(PathBuf, Vec<LoadedSource>), String> {
    let sources_root = resolve_sources_root(root, None).map_err(|error| error.to_string())?;
    let mut source_paths = BTreeSet::new();
    for pattern in ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"] {
        let glob_pattern = format!("{}/{}", sources_root.display(), pattern);
        let entries = glob::glob(&glob_pattern)
            .map_err(|error| format!("invalid glob '{glob_pattern}': {error}"))?;
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?;
            source_paths.insert(path);
        }
    }

    if source_paths.is_empty() {
        return Err(format!(
            "no ST sources found under {}",
            sources_root.display()
        ));
    }

    let mut sources = Vec::with_capacity(source_paths.len());
    for path in source_paths {
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
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
