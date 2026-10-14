//! The context, library and inspection answers (specification 43, AT-01 to AT-03), and the one load of
//! the project that every authoring decision starts from.
//!
//! Port of `agent/queries.ts` (`AuthoringQueries`). The HMI documents are read once per request, saved
//! or unsaved, as the editor shows them; the catalogue, bindings, texts and faceplates come from the
//! existing HMI commands. Context and mutations share [`load`], so a decision never mixes two readings
//! of the project.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use serde_json::{json, Map, Value};
use tower_lsp::lsp_types::Url;
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::page_files::HmiObject;
use trust_runtime::hmi::variable_tree::VariableNode;

use super::argument::{Argument, PanelSelection};
use super::documents::{self, DocumentSnapshot};
use super::drafts::DraftStore;
use super::error::{AgentError, AgentResult, INVALID_ARGUMENT, PROJECT_UNAVAILABLE};
use super::geometry::drawn_box;
use super::native::{self, text_of};
use super::pattern_references::{pattern_reference, PatternReference};
use super::patterns::composition_patterns;
use super::query_metadata;
use super::schema::{ContextSection, LibraryInput, ViewRegion};
use crate::handlers::commands::hmi_authoring::declarations::{resolve_nodes, Declarations};
use crate::handlers::commands::hmi_authoring::design_variables;
use crate::handlers::commands::{project_inputs, HmiBindingsCommandArgs, HMI_FILE_CHECK_COMMAND};
use crate::state::ServerState;

/// The most HMI documents one project may hold (specification 43, §4 and `queries.ts`).
const MAX_DOCUMENTS: usize = 1000;
/// The screen of a project without `hmi/screen.toml` (`DEFAULT_SCREEN` in `messages.ts`).
const DEFAULT_SCREEN_SIZE: [u64; 2] = [1920, 1080];

/// The project as one request reads it (AT-01 and §3): the HMI documents, the catalogue, the program's
/// bindings, the texts, the faceplates and the screen. Mutations and context share it.
#[derive(Debug)]
pub(super) struct Workspace {
    /// The HMI documents under the project, saved, unsaved or open clean, sorted by path.
    pub(super) snapshots: Vec<DocumentSnapshot>,
    /// The catalogue as the catalogue command answers it: `catalogue`, `symbols`, `formats`.
    pub(super) catalogue_answer: Value,
    /// The component catalogue, typed for composition and edits.
    pub(super) catalogue: Catalogue,
    /// The bindings of the compiled program, or the record of a failed compilation.
    pub(super) bindings: Value,
    /// The refusal of the compilation, when the program does not compile.
    pub(super) compilation_error: Option<String>,
    /// The project texts, as the texts command answers them.
    pub(super) texts: Value,
    /// The faceplates, as the faceplates command answers them.
    pub(super) faceplates: Value,
    /// The screen: its design size, theme and colours.
    pub(super) screen: Value,
    /// The remote runtime's variables by path (`{}` when none is connected).
    pub(super) remote: Value,
    /// The program revision, when the program compiles.
    pub(super) program_revision: Option<String>,
    /// Whether an unsaved ST source is open in the editor.
    pub(super) unsaved_sources: bool,
}

/// The view a context request asks for (AT-01): its sections, its selected page and its page window.
#[derive(Debug)]
pub(super) struct ContextView {
    /// The sections to return, or `None` for all of them.
    pub(super) sections: Option<Vec<ContextSection>>,
    /// The page the selection is for, when the request names one.
    pub(super) file: Option<String>,
    /// The index of the first page to return.
    pub(super) cursor: u64,
    /// The number of pages to return.
    pub(super) limit: u64,
}

/// The page an inspection reads: a source page on disk, or the page of one draft at its revision.
#[derive(Debug, Clone, Copy)]
pub(super) struct InspectRequest<'a> {
    /// The project-relative page, popup or faceplate.
    pub(super) file: &'a str,
    /// The object IDs to inspect, when named. Exclusive with `region`.
    pub(super) ids: Option<&'a [String]>,
    /// The rectangle whose overlapping objects are inspected. Exclusive with `ids`.
    pub(super) region: Option<ViewRegion>,
    /// The draft the page belongs to, with its revision and text, when the page is a draft's.
    pub(super) draft: Option<DraftPage<'a>>,
}

/// A draft's page as an inspection reads it.
#[derive(Debug, Clone, Copy)]
pub(super) struct DraftPage<'a> {
    /// The draft's identifier.
    pub(super) draft_id: &'a str,
    /// The draft revision the page is read at.
    pub(super) revision: &'a str,
    /// The page's text in that revision.
    pub(super) text: &'a str,
}

/// Loads the project once: its HMI documents, its catalogue, its compiled bindings and its texts. With
/// `partial`, a failed compilation is recorded and the answer still lists the rest (the context tool);
/// without it, the compilation failure is refused (every mutation).
///
/// # Errors
///
/// `COMPILATION_FAILED` for a failed compilation when not `partial`; the refusals of the snapshot walk
/// (`LIMIT_EXCEEDED` past 1000 documents, `PROJECT_UNAVAILABLE` for an unreadable folder) and of the
/// catalogue and texts commands.
pub(super) fn load(
    state: &ServerState,
    argument: &Argument,
    partial: bool,
) -> AgentResult<Workspace> {
    let snapshots = snapshots(argument)?;
    let unsaved = native::unsaved_pages(&argument.open);
    let root_uri = argument.root_uri.as_str();
    let (bindings, compilation_error) = match native::bindings(state, root_uri) {
        Ok(bindings) => (bindings, None),
        Err(error) if partial => (
            json!({
                "compilationError": error.to_string(),
                "variables": null,
                "sourceRevision": null,
                "sourceMode": "unavailable",
            }),
            Some(error.to_string()),
        ),
        Err(error) => return Err(error),
    };
    let catalogue_answer = native::catalogue(state, root_uri, &unsaved)?;
    let catalogue: Catalogue = serde_json::from_value(
        catalogue_answer
            .get("catalogue")
            .cloned()
            .unwrap_or(Value::Null),
    )
    .map_err(|error| {
        AgentError::validation(format!("the component catalogue does not load: {error}"))
    })?;
    let faceplates = native::faceplates(state, root_uri, &unsaved)?;
    let texts = native::texts(state, root_uri, &unsaved)?;
    let screen = match snapshots
        .iter()
        .find(|snapshot| snapshot.file == "hmi/screen.toml")
    {
        Some(snapshot) => native::parse_model(state, "screen", &snapshot.text, &snapshot.file)?,
        None => json!({ "size": DEFAULT_SCREEN_SIZE, "theme": "dark" }),
    };
    let program_revision = bindings
        .get("sourceRevision")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let unsaved_sources = argument
        .open
        .iter()
        .any(|(file, document)| document.dirty && file.to_ascii_lowercase().ends_with(".st"));
    Ok(Workspace {
        snapshots,
        catalogue_answer,
        catalogue,
        bindings,
        compilation_error,
        texts,
        faceplates,
        screen,
        remote: argument.remote.clone(),
        program_revision,
        unsaved_sources,
    })
}

/// The HMI documents under the project, saved or unsaved, sorted by path (`snapshots` in `queries.ts`).
/// The `assets` and `_evidence` folders are not documents. A clean open copy of a deleted file stays
/// deleted (`documents::snapshot`).
///
/// # Errors
///
/// `LIMIT_EXCEEDED` past 1000 documents; `PROJECT_UNAVAILABLE` when a folder cannot be read.
pub(super) fn snapshots(argument: &Argument) -> AgentResult<Vec<DocumentSnapshot>> {
    let mut files = BTreeSet::new();
    collect_documents(&argument.root, "hmi", &mut files)?;
    for file in argument.open.keys() {
        if file.starts_with("hmi/") && file.ends_with(".toml") {
            files.insert(file.clone());
        }
    }
    if files.len() > MAX_DOCUMENTS {
        return Err(AgentError::limit(format!(
            "project has more than {MAX_DOCUMENTS} HMI documents."
        )));
    }
    let mut snapshots = Vec::with_capacity(files.len());
    for file in &files {
        let snapshot = documents::snapshot(&argument.root, file, argument.open.get(file))?;
        if snapshot.exists {
            snapshots.push(snapshot);
        }
    }
    Ok(snapshots)
}

/// Adds the `.toml` files under `relative` to `files`, skipping the `assets` and `_evidence` folders.
fn collect_documents(root: &Path, relative: &str, files: &mut BTreeSet<String>) -> AgentResult<()> {
    let entries = match fs::read_dir(root.join(relative)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(AgentError::new(
                PROJECT_UNAVAILABLE,
                format!("cannot read {relative}: {error}"),
            ))
        }
    };
    let mut children: Vec<(String, bool)> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            AgentError::new(
                PROJECT_UNAVAILABLE,
                format!("cannot read {relative}: {error}"),
            )
        })?;
        let is_directory = entry.file_type().is_ok_and(|kind| kind.is_dir());
        children.push((
            entry.file_name().to_string_lossy().into_owned(),
            is_directory,
        ));
    }
    children.sort();
    for (name, is_directory) in children {
        let child = format!("{relative}/{name}");
        if is_directory && name != "assets" && name != "_evidence" {
            collect_documents(root, &child, files)?;
        } else if name.ends_with(".toml") {
            files.insert(child);
        }
    }
    Ok(())
}

/// The context answer (AT-01): the sections the request names, always with the compilation state, the
/// selection, the provenance, the revisions, the program revision, the source state, the page window and
/// the open drafts (`context` in `queries.ts`). A failed compiler is named, with `variables: null`; the
/// pages still list (`partial`).
///
/// # Errors
///
/// `VALIDATION_FAILED` for a page that cannot be parsed is reported inside its entry, not refused; the
/// refusals of the draft listing (`PROJECT_UNAVAILABLE` for an unreadable storage folder).
pub(super) fn context_answer(
    state: &ServerState,
    workspace: &Workspace,
    argument: &Argument,
    view: &ContextView,
) -> AgentResult<Value> {
    let pages = page_entries(state, workspace)?;
    let total = u64::try_from(pages.len()).unwrap_or(u64::MAX);
    let window_end = view.cursor.saturating_add(view.limit);
    let start = usize::try_from(view.cursor).unwrap_or(usize::MAX);
    let take = usize::try_from(view.limit).unwrap_or(usize::MAX);
    let window: Vec<Value> = pages.iter().skip(start).take(take).cloned().collect();
    let next_cursor = if window_end < total {
        json!(window_end)
    } else {
        Value::Null
    };
    let selection = selection_for(&argument.selections, view.file.as_deref());
    let selection_value = selection.map_or(Value::Null, |selected| {
        json!({
            "file": selected.file,
            "ids": selected.ids,
            "documentVersion": selected.document_version,
        })
    });
    let compiled = workspace
        .bindings
        .get("variables")
        .cloned()
        .unwrap_or(Value::Null);
    let mut variables = offered_variables(state, argument, compiled)?;
    if let Value::Array(list) = &mut variables {
        list.extend(remote_variables(&workspace.remote));
    }
    let drafts = DraftStore::open(&argument.storage)?.list(&argument.workspace_id)?;
    let errors: Vec<String> = workspace.compilation_error.iter().cloned().collect();
    let mut data = Map::new();
    data.insert("remoteCatalogue".into(), workspace.remote.clone());
    data.insert(
        "simulatorSessions".into(),
        Value::Array(argument.simulator_sessions.clone()),
    );
    data.insert(
        "compilation".into(),
        json!({
            "status": if workspace.compilation_error.is_some() { "failed" } else { "ready" },
            "errors": errors,
        }),
    );
    data.insert(
        "partial".into(),
        json!(workspace.compilation_error.is_some() || window_end < total),
    );
    data.insert("selection".into(), selection_value);
    data.insert(
        "provenance".into(),
        json!({
            "variables": "compiled-program",
            "formats": "project-catalogue",
            "topology": "not-inferred",
            "revisions": "editor-buffer-or-disk",
        }),
    );
    data.insert("screen".into(), workspace.screen.clone());
    data.insert("texts".into(), workspace.texts.clone());
    data.insert(
        "languages".into(),
        workspace
            .texts
            .get("languages")
            .cloned()
            .unwrap_or_else(|| json!([])),
    );
    data.insert("variables".into(), variables);
    data.insert(
        "alarms".into(),
        workspace
            .bindings
            .get("alarms")
            .cloned()
            .unwrap_or_else(|| json!([])),
    );
    data.insert(
        "catalogue".into(),
        workspace
            .catalogue_answer
            .get("catalogue")
            .cloned()
            .unwrap_or(Value::Null),
    );
    data.insert(
        "symbols".into(),
        workspace
            .catalogue_answer
            .get("symbols")
            .cloned()
            .unwrap_or_else(|| json!([])),
    );
    data.insert(
        "formats".into(),
        workspace
            .catalogue_answer
            .get("formats")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    data.insert(
        "faceplates".into(),
        workspace
            .faceplates
            .get("faceplates")
            .cloned()
            .unwrap_or_else(|| json!([])),
    );
    data.insert("pages".into(), Value::Array(window));
    data.insert("pageCount".into(), json!(total));
    data.insert("nextCursor".into(), next_cursor);
    data.insert(
        "revisions".into(),
        json!(revisions_of(&workspace.snapshots)),
    );
    data.insert(
        "programRevision".into(),
        workspace
            .program_revision
            .clone()
            .map_or(Value::Null, Value::String),
    );
    data.insert(
        "sourceState".into(),
        json!(if workspace.unsaved_sources {
            "editor"
        } else {
            "saved"
        }),
    );
    data.insert(
        "topology".into(),
        json!({ "status": "unspecified", "connections": [] }),
    );
    data.insert(
        "drafts".into(),
        serde_json::to_value(drafts).unwrap_or_else(|_| json!([])),
    );
    let Some(sections) = &view.sections else {
        return Ok(Value::Object(data));
    };
    if sections.contains(&ContextSection::Sources) {
        data.insert("sources".into(), sources_of(&workspace.snapshots));
    }
    const ALWAYS: [&str; 10] = [
        "compilation",
        "partial",
        "selection",
        "provenance",
        "revisions",
        "programRevision",
        "sourceState",
        "nextCursor",
        "pageCount",
        "drafts",
    ];
    let wanted: BTreeSet<&str> = sections
        .iter()
        .map(|section| section_key(*section))
        .collect();
    Ok(Value::Object(
        data.into_iter()
            .filter(|(key, _)| ALWAYS.contains(&key.as_str()) || wanted.contains(key.as_str()))
            .collect(),
    ))
}

/// The text and unsaved state of each existing page and popup, as the documents observe them (specification 43, review with
/// `explore: true`): the snapshot an exploration renders, which it bounds itself.
fn sources_of(snapshots: &[DocumentSnapshot]) -> Value {
    Value::Array(
        snapshots
            .iter()
            .filter(|snapshot| {
                snapshot.exists
                    && (snapshot.file.starts_with("hmi/pages/")
                        || snapshot.file.starts_with("hmi/popups/"))
            })
            .map(|snapshot| {
                json!({ "file": snapshot.file, "text": snapshot.text, "dirty": snapshot.dirty })
            })
            .collect(),
    )
}

/// The answer key of a context section.
const fn section_key(section: ContextSection) -> &'static str {
    match section {
        ContextSection::Screen => "screen",
        ContextSection::Texts => "texts",
        ContextSection::Languages => "languages",
        ContextSection::Variables => "variables",
        ContextSection::Alarms => "alarms",
        ContextSection::Catalogue => "catalogue",
        ContextSection::Symbols => "symbols",
        ContextSection::Formats => "formats",
        ContextSection::Faceplates => "faceplates",
        ContextSection::Pages => "pages",
        ContextSection::Topology => "topology",
        ContextSection::RemoteCatalogue => "remoteCatalogue",
        ContextSection::SimulatorSessions => "simulatorSessions",
        ContextSection::Sources => "sources",
    }
}

/// The entries of the project's pages: each page's title, order, visibility, group and icon, or the
/// reason it does not load (`context` in `queries.ts`).
fn page_entries(state: &ServerState, workspace: &Workspace) -> AgentResult<Vec<Value>> {
    let mut entries = Vec::new();
    for snapshot in workspace
        .snapshots
        .iter()
        .filter(|snapshot| snapshot.file.starts_with("hmi/pages/"))
    {
        let entry = match native::parse_model(state, "page", &snapshot.text, &snapshot.file) {
            Ok(model) => json!({
                "file": snapshot.file,
                "revision": snapshot.revision,
                "dirty": snapshot.dirty,
                "title": model.get("title").cloned().unwrap_or(Value::Null),
                "order": model.get("order").cloned().unwrap_or(Value::Null),
                "hidden": model.get("hidden").and_then(Value::as_bool).unwrap_or(false),
                "group": model.get("group").cloned().unwrap_or(Value::Null),
                "icon": model.get("icon").cloned().unwrap_or(Value::Null),
            }),
            Err(error) => json!({
                "file": snapshot.file,
                "revision": snapshot.revision,
                "dirty": snapshot.dirty,
                "error": error.to_string(),
            }),
        };
        entries.push(entry);
    }
    Ok(entries)
}

/// The revision of each document, by path.
#[must_use]
pub(super) fn revisions_of(snapshots: &[DocumentSnapshot]) -> BTreeMap<String, String> {
    snapshots
        .iter()
        .map(|snapshot| (snapshot.file.clone(), snapshot.revision.clone()))
        .collect()
}

/// The selection a context answer reports (`contextSelection` in `queryMetadata.ts`). For a named page,
/// the panel that shows it; otherwise the active panel. Exactly one match is a selection; several are
/// ambiguous and report none.
#[must_use]
pub(super) fn selection_for<'selections>(
    selections: &'selections [PanelSelection],
    file: Option<&str>,
) -> Option<&'selections PanelSelection> {
    let mut matches = selections.iter().filter(|selection| match file {
        Some(file) => selection.file == file,
        None => selection.active,
    });
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

/// The variables the context offers: the compiled program's, kept only where the binding checks resolve their path, so
/// the picker never offers a variable the checks would refuse (specification 39, §7.1). A compile failure stays `null`,
/// which the context still names.
///
/// # Errors
///
/// The refusals of [`declared_variables`].
fn offered_variables(
    state: &ServerState,
    argument: &Argument,
    compiled: Value,
) -> AgentResult<Value> {
    let mut nodes = match compiled {
        Value::Array(nodes) => nodes,
        other => return Ok(other),
    };
    let declared = declared_variables(state, argument)?;
    keep_declared(&mut nodes, &declared);
    Ok(Value::Array(nodes))
}

/// The declaration tree the binding checks resolve paths in, for this project: the sources, design variables and
/// declarations that `trust-lsp.hmiFileCheck` with `authoring` reads (`hmi_authoring/checks.rs`).
///
/// # Errors
///
/// `VALIDATION_FAILED` with the reason when the project's sources or design cannot be read.
fn declared_variables(state: &ServerState, argument: &Argument) -> AgentResult<Vec<VariableNode>> {
    let root_uri = Url::parse(&argument.root_uri).map_err(|error| {
        AgentError::validation(format!("the project address is not a URI: {error}"))
    })?;
    let selection = HmiBindingsCommandArgs {
        root_uri: Some(root_uri),
        ..HmiBindingsCommandArgs::default()
    };
    let inputs = project_inputs(state, &selection, HMI_FILE_CHECK_COMMAND)
        .map_err(AgentError::validation)?;
    let index = Declarations::read(&inputs.sources);
    design_variables::variables(&inputs, index.as_ref()).map_err(AgentError::validation)
}

/// Keeps the nodes whose path the declaration tree resolves, and the members of each that it resolves. The protected
/// and private members of a function block are absent from that tree, so they are dropped.
fn keep_declared(nodes: &mut Vec<Value>, declared: &[VariableNode]) {
    nodes.retain(|node| {
        node.get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| resolve_nodes(declared, path).is_some())
    });
    for node in nodes.iter_mut() {
        keep_members(node, declared);
    }
}

/// Keeps the members of one node, and of its array element template, that the declaration tree resolves. A node left
/// without members loses its `children` key, as the serialized tree omits empty children.
fn keep_members(node: &mut Value, declared: &[VariableNode]) {
    if let Some(element) = node
        .get_mut("array")
        .and_then(|array| array.get_mut("element"))
    {
        keep_members(element, declared);
    }
    let emptied = match node.get_mut("children").and_then(Value::as_array_mut) {
        Some(children) => {
            keep_declared(children, declared);
            children.is_empty()
        }
        None => false,
    };
    if emptied {
        if let Some(fields) = node.as_object_mut() {
            fields.remove("children");
        }
    }
}

/// The remote runtime's variables as the program's variables, each named by its path (`remoteVariableContext`).
#[must_use]
pub(super) fn remote_variables(remote: &Value) -> Vec<Value> {
    remote
        .as_object()
        .map(|nodes| {
            nodes
                .values()
                .map(|node| {
                    let mut variable = node.clone();
                    if let Value::Object(fields) = &mut variable {
                        let path = fields.get("path").cloned().unwrap_or(Value::Null);
                        fields.insert("name".into(), path);
                    }
                    variable
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The library answer (AT-02): the components the query and category select, one page of them with
/// their definition revisions, the faceplates, the formats, and the composition patterns the query
/// names (with the reference of an exact pattern ID). Symbol states are not listed here (see the port
/// report: `drawing/previews.ts` stays in the extension for now).
///
/// # Errors
///
/// `VALIDATION_FAILED` when the catalogue cannot be read or a pattern reference cannot be built.
pub(super) fn library(
    state: &ServerState,
    argument: &Argument,
    input: &LibraryInput,
) -> AgentResult<Value> {
    let query = input.query.clone().unwrap_or_default();
    let cursor = usize::try_from(input.cursor.unwrap_or(0)).unwrap_or(usize::MAX);
    let limit = usize::try_from(input.limit.unwrap_or(12)).unwrap_or(12);
    let unsaved = native::unsaved_pages(&argument.open);
    let root_uri = argument.root_uri.as_str();
    let answer = native::catalogue(state, root_uri, &unsaved)?;
    let catalogue = answer.get("catalogue").cloned().unwrap_or(Value::Null);
    let symbols = answer.get("symbols").cloned().unwrap_or_else(|| json!([]));
    let formats = answer.get("formats").cloned().unwrap_or_else(|| json!({}));
    let faceplates = native::faceplates(state, root_uri, &unsaved)?;
    let needle = query.to_lowercase();
    let components: Vec<Value> = catalogue
        .get("components")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let matches: Vec<&Value> = components
        .iter()
        .filter(|component| {
            let kind = text_of(component, "type");
            let category = text_of(component, "category");
            let group = text_of(component, "group");
            let category_matches = match input.category.as_deref() {
                None => true,
                Some(wanted) => wanted.is_empty() || category == wanted,
            };
            category_matches
                && format!("{kind} {category} {group}")
                    .to_lowercase()
                    .contains(&needle)
        })
        .collect();
    let total = matches.len();
    let page: Vec<Value> = matches
        .iter()
        .skip(cursor)
        .take(limit)
        .map(|component| {
            let mut entry = (*component).clone();
            let revision = documents::sha256_hex(component.to_string().as_bytes());
            if let Value::Object(fields) = &mut entry {
                fields.insert("definitionRevision".into(), json!(revision));
            }
            entry
        })
        .collect();
    let patterns = composition_patterns(&query);
    let mut pattern_values = Vec::with_capacity(patterns.len());
    for pattern in patterns {
        let mut value = serde_json::to_value(&pattern).map_err(|error| {
            AgentError::validation(format!("pattern cannot be encoded: {error}"))
        })?;
        if query == pattern.id.as_str() {
            let typed: Catalogue = serde_json::from_value(catalogue.clone()).map_err(|error| {
                AgentError::validation(format!("the component catalogue does not load: {error}"))
            })?;
            let reference = pattern_reference(pattern.id, &typed)?;
            if let Value::Object(fields) = &mut value {
                fields.insert("reference".into(), reference_value(&reference)?);
            }
        }
        pattern_values.push(value);
    }
    let revision_input = json!({
        "catalogue": catalogue.clone(),
        "symbols": symbols.clone(),
        "formats": formats.clone(),
    });
    let next_cursor = if cursor.saturating_add(limit) < total {
        json!(cursor.saturating_add(limit))
    } else {
        Value::Null
    };
    Ok(json!({
        "components": page,
        "catalogueRevision": documents::sha256_hex(revision_input.to_string().as_bytes()),
        "faceplates": faceplates.get("faceplates").cloned().unwrap_or_else(|| json!([])),
        "formats": formats,
        "total": total,
        "nextCursor": next_cursor,
        "symbols": symbols,
        "format": catalogue.get("format").cloned().unwrap_or(Value::Null),
        "compositionPatterns": pattern_values,
    }))
}

/// A pattern reference as JSON: its origin, size, page model and the components the catalogue lacks.
fn reference_value(reference: &PatternReference) -> AgentResult<Value> {
    Ok(json!({
        "origin": reference.origin,
        "size": reference.size,
        "model": serde_json::to_value(&reference.model)
            .map_err(|error| AgentError::validation(format!("reference cannot be encoded: {error}")))?,
        "missingComponents": reference.missing_components,
    }))
}

/// The inspection answer (AT-03): the selected objects in page order with their pipes, neighbours and
/// original z-order, and the metadata of the page (groups, value provenance, placed faceplates, navigation
/// and unresolved references). A draft's page inspects the draft's text and does not alter the source.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for a path that is not a page, popup or faceplate, for `ids` with `region`, for a
/// region that is not finite and positive, or for an unknown object; `PROJECT_UNAVAILABLE` when the page
/// does not exist.
pub(super) fn inspect(
    state: &ServerState,
    argument: &Argument,
    request: InspectRequest<'_>,
) -> AgentResult<Value> {
    check_inspectable(request.file)?;
    if request.ids.is_some() && request.region.is_some() {
        return Err(AgentError::invalid(
            "select object IDs or one finite positive region, not both.",
        ));
    }
    let snapshot = match request.draft {
        Some(draft) => DocumentSnapshot {
            file: request.file.to_owned(),
            exists: true,
            text: draft.text.to_owned(),
            revision: draft.revision.to_owned(),
            version: None,
            dirty: false,
        },
        None => documents::snapshot(
            &argument.root,
            request.file,
            argument.open.get(request.file),
        )?,
    };
    if !snapshot.exists {
        return Err(AgentError::new(
            PROJECT_UNAVAILABLE,
            "the selected page does not exist.",
        ));
    }
    let kind = page_kind(request.file);
    let model = native::parse_model(state, kind, &snapshot.text, request.file)?;
    let objects: Vec<Value> = model
        .get("object")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let pipes: Vec<Value> = model
        .get("pipe")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let selected: BTreeSet<String> = match (request.ids, request.region) {
        (Some(ids), _) => ids.iter().cloned().collect(),
        (None, Some(region)) => {
            let mut overlapping = BTreeSet::new();
            for object in &objects {
                let typed: HmiObject = serde_json::from_value(object.clone()).map_err(|error| {
                    AgentError::validation(format!("{}: {error}", request.file))
                })?;
                if overlaps(drawn_box(&typed), region) {
                    overlapping.insert(text_of(object, "id").to_owned());
                }
            }
            overlapping
        }
        (None, None) => objects
            .iter()
            .map(|object| text_of(object, "id").to_owned())
            .collect(),
    };
    let known: BTreeSet<&str> = objects.iter().map(|object| text_of(object, "id")).collect();
    if let Some(unknown) = selected.iter().find(|id| !known.contains(id.as_str())) {
        return Err(AgentError::invalid(format!("unknown object {unknown}.")));
    }
    let joined: Vec<Value> = pipes
        .iter()
        .filter(|pipe| {
            selected.contains(port_object(pipe, "from"))
                || selected.contains(port_object(pipe, "to"))
        })
        .cloned()
        .collect();
    let mut neighbours: Vec<String> = Vec::new();
    for pipe in &joined {
        for end in [port_object(pipe, "from"), port_object(pipe, "to")] {
            if !selected.contains(end) && !neighbours.iter().any(|known| known.as_str() == end) {
                neighbours.push(end.to_owned());
            }
        }
    }
    let text_for_placed = request
        .draft
        .map_or(snapshot.text.as_str(), |draft| draft.text);
    let metadata = query_metadata::inspect_metadata(
        state,
        argument,
        &model,
        request.file,
        text_for_placed,
        request.draft.is_some(),
    )?;
    let z_order: Map<String, Value> = objects
        .iter()
        .enumerate()
        .filter(|(_, object)| selected.contains(text_of(object, "id")))
        .map(|(index, object)| (text_of(object, "id").to_owned(), json!(index)))
        .collect();
    let chosen: Vec<Value> = objects
        .iter()
        .filter(|object| selected.contains(text_of(object, "id")))
        .cloned()
        .collect();
    let mut answer = metadata;
    if let Value::Object(fields) = &mut answer {
        fields.insert("file".into(), json!(request.file));
        fields.insert("revision".into(), json!(snapshot.revision));
        fields.insert("dirty".into(), json!(snapshot.dirty));
        match request.draft {
            Some(draft) => {
                fields.insert("draftId".into(), json!(draft.draft_id));
                fields.insert("origin".into(), json!("draft"));
            }
            None => {
                fields.insert("origin".into(), json!("source"));
            }
        }
        fields.insert(
            "title".into(),
            model.get("title").cloned().unwrap_or(Value::Null),
        );
        fields.insert("objects".into(), Value::Array(chosen));
        fields.insert("pipes".into(), Value::Array(joined));
        fields.insert("neighbours".into(), json!(neighbours));
        fields.insert("zOrder".into(), Value::Object(z_order));
    }
    Ok(answer)
}

/// The command kind of an inspected file: a popup, a faceplate (`hmi/panels/`) or a page.
#[must_use]
fn page_kind(file: &str) -> &'static str {
    if file.starts_with("hmi/popups/") {
        "popup"
    } else if file.starts_with("hmi/panels/") {
        "faceplate"
    } else {
        "page"
    }
}

/// The object a pipe port belongs to: the text before its first dot (`split(".")[0]`).
#[must_use]
fn port_object<'value>(pipe: &'value Value, end: &str) -> &'value str {
    text_of(pipe, end).split('.').next().unwrap_or("")
}

/// Whether a drawn box overlaps a region's interior (the test of `inspect` in `queries.ts`).
#[must_use]
fn overlaps(box_: super::geometry::Rect, region: ViewRegion) -> bool {
    box_.x < region.x + region.width
        && box_.x + box_.width > region.x
        && box_.y < region.y + region.height
        && box_.y + box_.height > region.y
}

/// Checks that a request's `file` is a page, popup or faceplate: `hmi/pages/`, `hmi/popups/` or
/// `hmi/panels/`, then one TOML file name with no further folder (`queries.ts`).
///
/// # Errors
///
/// `INVALID_ARGUMENT` when the path is outside the inspectable folders.
pub(super) fn check_inspectable(file: &str) -> AgentResult<()> {
    let named = ["hmi/pages/", "hmi/popups/", "hmi/panels/"]
        .iter()
        .find_map(|prefix| file.strip_prefix(prefix))
        .and_then(|rest| rest.strip_suffix(".toml"))
        .is_some_and(|name| !name.is_empty() && !name.contains('/') && !name.contains('\\'));
    if !named {
        return Err(AgentError::new(
            INVALID_ARGUMENT,
            "select an HMI page, popup or faceplate.",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "queries_tests.rs"]
mod tests;
