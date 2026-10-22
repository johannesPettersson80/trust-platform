//! The authoring commands (specification 43, §3 to §6): the operations behind `trust-lsp.hmiAgent.*`.
//!
//! Port of `agent/service.ts` (`AuthoringService.execute`). Each command takes the argument of
//! `argument.rs`, checks the tool contract's envelope through `boundary.rs`, decodes the tool input
//! through `schema.rs` and answers the operation's data. The context, library and inspection answers
//! are in `queries.rs`; the review and the application are in `review_apply.rs`.
//!
//! Rendering (AT-07), the preview and the simulator (AT-08) and the editor transaction (AT-09) stay in
//! the extension. The service answers what they need (a draft's pages and revision) and records what
//! they report.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{json, Map, Value};
use trust_runtime::hmi::page_files::HmiPage;

use crate::state::ServerState;

use super::argument::Argument;
use super::boundary::{self, Success};
use super::compose::compose_agent_page;
use super::documents::{self, DocumentSnapshot};
use super::drafts::{require_retained_pages, Draft, DraftStore, EditResult, PreparedProposal};
use super::edits::edit_page;
use super::error::{AgentError, AgentResult, COMPILATION_FAILED, CONFLICT, PROJECT_UNAVAILABLE};
use super::geometry_quality::{geometry_quality, viewport_profiles};
use super::intent::composition_intent;
use super::native::{self, text_of};
use super::queries::{self, ContextView, DraftPage, InspectRequest, Workspace};
use super::review::describe_page_changes;
use super::review_apply;
use super::routing_quality::routing_quality;
use super::schema::{
    self, is_page_file, ComposeInput, ContextInput, EditInput, InspectInput, LibraryInput,
    ValidateInput,
};

/// The context of a project, read by a tool (`hmi_get_context`).
pub(super) const CONTEXT: &str = "trust-lsp.hmiAgent.context";
/// The component library (`hmi_browse_library`).
pub(super) const LIBRARY: &str = "trust-lsp.hmiAgent.library";
/// The inspection of a page or a draft (`hmi_inspect_page`).
pub(super) const INSPECT: &str = "trust-lsp.hmiAgent.inspect";
/// The composition of new or refined pages (`hmi_compose_page`).
pub(super) const COMPOSE: &str = "trust-lsp.hmiAgent.compose";
/// The edits of a draft (`hmi_edit_draft`).
pub(super) const EDIT: &str = "trust-lsp.hmiAgent.edit";
/// The validation of a draft (`hmi_validate_draft`).
pub(super) const VALIDATE: &str = "trust-lsp.hmiAgent.validate";
/// The draft listing and reads the extension needs (the drafts behind AT-07 to AT-09).
pub(super) const DRAFTS: &str = "trust-lsp.hmiAgent.drafts";
/// The review of a draft, in two phases: prepare, then record after the extension renders it (AT-09).
pub(super) const REVIEW: &str = "trust-lsp.hmiAgent.review";
/// The application of a reviewed draft, in two phases: prepare, then the receipt of the editor
/// transaction (AT-09).
pub(super) const APPLY: &str = "trust-lsp.hmiAgent.apply";
/// The discard of a draft (`hmi_apply_draft` with `discard`).
pub(super) const DISCARD: &str = "trust-lsp.hmiAgent.discard";
/// The schema check of any tool's input, for the tools the extension renders (AT-07 and AT-08).
pub(super) const CHECK_INPUT: &str = "trust-lsp.hmiAgent.checkInput";

/// Every authoring command, advertised through `executeCommandProvider` (specification 43, §3).
pub(in crate::handlers::commands) const AUTHORING_COMMANDS: [&str; 11] = [
    CONTEXT,
    LIBRARY,
    INSPECT,
    COMPOSE,
    EDIT,
    VALIDATE,
    DRAFTS,
    REVIEW,
    APPLY,
    DISCARD,
    CHECK_INPUT,
];

/// Whether `name` is an authoring command.
#[must_use]
pub(in crate::handlers::commands) fn is_command(name: &str) -> bool {
    AUTHORING_COMMANDS.contains(&name)
}

/// Runs the authoring command `name` on its argument, or `None` when it is not one.
#[must_use]
pub(in crate::handlers::commands) fn execute(
    state: &ServerState,
    name: &str,
    arguments: Vec<Value>,
) -> Option<Value> {
    is_command(name).then(|| answer(state, name, arguments))
}

/// The envelope answer of one command: the argument is read, then the boundary checks the request and runs
/// the operation (`invokeAuthoring`).
fn answer(state: &ServerState, name: &str, arguments: Vec<Value>) -> Value {
    let argument = match Argument::read(arguments) {
        Ok(argument) => argument,
        Err(error) => return boundary::failure_envelope(&error),
    };
    let request = argument.request.clone();
    let workspace_id = argument.workspace_id.clone();
    boundary::invoke(&request, &workspace_id, false, |input| {
        run(state, &argument, name, input)
    })
}

/// Dispatches a checked request to its operation.
fn run(
    state: &ServerState,
    argument: &Argument,
    name: &str,
    input: &Map<String, Value>,
) -> AgentResult<Success> {
    let raw = Value::Object(input.clone());
    match name {
        CONTEXT => context(state, argument, raw),
        LIBRARY => library(state, argument, raw),
        INSPECT => inspect(state, argument, raw),
        COMPOSE => compose(state, argument, &raw),
        EDIT => edit(state, argument, &raw),
        VALIDATE => validate(state, argument, &raw),
        DRAFTS => review_apply::drafts(argument),
        REVIEW => review_apply::review(state, argument, &raw),
        APPLY => review_apply::apply(state, argument, &raw),
        DISCARD => review_apply::discard(argument, &raw),
        CHECK_INPUT => check_input(argument, &raw),
        _ => Err(AgentError::invalid("unknown authoring command.")),
    }
}

/// `hmi_get_context`: the project as the context answer shows it (AT-01).
fn context(state: &ServerState, argument: &Argument, raw: Value) -> AgentResult<Success> {
    let input: ContextInput = schema::read("hmi_get_context", raw)?;
    let workspace = queries::load(state, argument, true)?;
    let view = ContextView {
        sections: input.sections,
        file: input.file,
        cursor: input.cursor.unwrap_or(0),
        limit: input.limit.unwrap_or(100),
    };
    let data = queries::context_answer(state, &workspace, argument, &view)?;
    Ok(success(data, queries::revisions_of(&workspace.snapshots)))
}

/// `hmi_browse_library`: the components and composition patterns (AT-02).
fn library(state: &ServerState, argument: &Argument, raw: Value) -> AgentResult<Success> {
    let input: LibraryInput = schema::read("hmi_browse_library", raw)?;
    let data = queries::library(state, argument, &input)?;
    Ok(success(data, BTreeMap::new()))
}

/// `hmi_inspect_page`: a page or a draft's page, inspected (AT-03). A draft is read at its revision and
/// does not alter the source.
fn inspect(state: &ServerState, argument: &Argument, raw: Value) -> AgentResult<Success> {
    let input: InspectInput = schema::read("hmi_inspect_page", raw)?;
    let draft = match (&input.draft_id, &input.revision) {
        (Some(id), Some(revision)) => {
            let store = DraftStore::open(&argument.storage)?;
            let draft = owned_draft(&store, argument, id)?;
            at_revision(&draft, revision)?;
            require_retained_pages(&draft)?;
            Some(draft)
        }
        _ => None,
    };
    let page_draft = match &draft {
        Some(draft) => {
            let text = draft
                .files
                .get(&input.file)
                .ok_or_else(|| AgentError::invalid("select a file in the draft."))?;
            Some(DraftPage {
                draft_id: &draft.id,
                revision: &draft.revision,
                text,
            })
        }
        None => None,
    };
    let request = InspectRequest {
        file: &input.file,
        ids: input.ids.as_deref(),
        region: input.region,
        draft: page_draft,
    };
    let data = queries::inspect(state, argument, request)?;
    Ok(success(data, BTreeMap::new()))
}

/// `hmi_compose_page`: one page or several, composed or copied into a draft (AT-04). The creation key
/// replays an earlier request with the same input and refuses another input under the same key.
fn compose(state: &ServerState, argument: &Argument, raw: &Value) -> AgentResult<Success> {
    let input: ComposeInput = schema::read("hmi_compose_page", raw.clone())?;
    let (key, pages) = match &input {
        ComposeInput::Page(page) => (page.key.clone(), vec![raw.clone()]),
        ComposeInput::Pages(group) => (
            group.key.clone(),
            raw.get("pages")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        ),
    };
    let names: Vec<String> = pages
        .iter()
        .map(|page| text_of(page, "file").to_owned())
        .collect();
    let unique: BTreeSet<&String> = names.iter().collect();
    if unique.len() != names.len() || names.iter().any(|file| !is_page_file(file)) {
        return Err(AgentError::invalid(
            "use unique hmi/pages/<name>.toml destinations.",
        ));
    }
    let store = DraftStore::open(&argument.storage)?;
    let created = store.create(&argument.workspace_id, &key, raw, || {
        build_composition(state, argument, raw, &pages, &names)
    })?;
    let mut data = created.data.unwrap_or_else(|| json!({}));
    if let Value::Object(fields) = &mut data {
        fields.insert("draftId".into(), json!(created.draft.id));
        fields.insert("revision".into(), json!(created.draft.revision));
    }
    let revisions = BTreeMap::from([(created.draft.id.clone(), created.draft.revision.clone())]);
    Ok(success(data, revisions))
}

/// The pages of one composition, prepared before any draft is stored: each is copied from its source
/// page or composed from its constraints, then the proposal's intent and program revision are recorded.
fn build_composition(
    state: &ServerState,
    argument: &Argument,
    raw: &Value,
    pages: &[Value],
    names: &[String],
) -> AgentResult<PreparedProposal> {
    let workspace = queries::load(state, argument, false)?;
    let size = screen_size(&workspace.screen)?;
    let mut files = BTreeMap::new();
    let mut models: BTreeMap<String, HmiPage> = BTreeMap::new();
    let mut targets = Vec::with_capacity(names.len());
    for (page, file) in pages.iter().zip(names) {
        let target = documents::snapshot(&argument.root, file, argument.open.get(file))?;
        let (model, text) = match page.get("sourceFile").and_then(Value::as_str) {
            Some(source_file) => copy_source(state, argument, &target, page, source_file)?,
            None => compose_new(state, &workspace, &target, page, size)?,
        };
        targets.push(target);
        files.insert(file.clone(), text);
        models.insert(file.clone(), model);
    }
    let Some(revision) = workspace.program_revision.clone() else {
        return Err(AgentError::new(
            COMPILATION_FAILED,
            "the PLC program has no revision to compose against; fix its compilation first.",
        ));
    };
    let mut merged: Map<String, Value> = pages
        .first()
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(top) = raw.as_object() {
        for (key, value) in top {
            merged.insert(key.clone(), value.clone());
        }
    }
    let title = names
        .first()
        .and_then(|file| models.get(file))
        .map_or_else(String::new, |model| model.title.clone());
    let intent = composition_intent(&Value::Object(merged), &title)?;
    let intent_value = encode(&intent)?;
    let mut base: Vec<DocumentSnapshot> = workspace
        .snapshots
        .iter()
        .filter(|snapshot| !names.contains(&snapshot.file))
        .cloned()
        .collect();
    base.extend(targets);
    let mut data = Map::new();
    if let [only] = names {
        data.insert("file".into(), json!(only));
        data.insert("model".into(), encode_model(&models, only)?);
    } else {
        data.insert("files".into(), json!(names));
        data.insert("models".into(), encode(&models)?);
    }
    data.insert("intent".into(), intent_value.clone());
    data.insert("assumptions".into(), json!(intent.assumptions));
    data.insert("visualReview".into(), json!("required"));
    Ok(PreparedProposal {
        base,
        files,
        program_revision: Some(revision),
        intent: Some(intent_value),
        data: Some(Value::Object(data)),
    })
}

/// Copies an existing page into a draft, with its unsaved text, and retitles it when the request names a
/// different title (`sourceFile` of `compose.ts`).
fn copy_source(
    state: &ServerState,
    argument: &Argument,
    target: &DocumentSnapshot,
    page: &Value,
    source_file: &str,
) -> AgentResult<(HmiPage, String)> {
    if !is_page_file(source_file) {
        return Err(AgentError::invalid(
            "sourceFile must identify a project page.",
        ));
    }
    if target.exists && target.file != source_file {
        return Err(AgentError::conflict("the destination page already exists."));
    }
    let source = documents::snapshot(&argument.root, source_file, argument.open.get(source_file))?;
    if !source.exists {
        return Err(AgentError::new(
            PROJECT_UNAVAILABLE,
            "source page does not exist.",
        ));
    }
    let mut model = native::page(state, &source.text, source_file)?;
    if let Some(title) = page.get("title").and_then(Value::as_str) {
        if title != model.title {
            model.title = title.to_owned();
            let text = native::format_page(state, &model)?;
            return Ok((model, text));
        }
    }
    let text = source.text.clone();
    Ok((model, text))
}

/// Composes a new page from its constraints (`composeAgentPage`), refusing a destination that exists.
fn compose_new(
    state: &ServerState,
    workspace: &Workspace,
    target: &DocumentSnapshot,
    page: &Value,
    size: [u32; 2],
) -> AgentResult<(HmiPage, String)> {
    if target.exists {
        return Err(AgentError::conflict(
            "page exists; use sourceFile to begin a reviewed edit of that page.",
        ));
    }
    let mut composition = Map::new();
    composition.insert("size".into(), json!(size));
    for key in ["title", "pattern", "items", "connections", "layout"] {
        if let Some(value) = page.get(key) {
            composition.insert(key.into(), value.clone());
        }
    }
    let model = compose_agent_page(&Value::Object(composition), &workspace.catalogue)?;
    let text = native::format_page(state, &model)?;
    Ok((model, text))
}

/// `hmi_edit_draft`: applies the operations to a copy of one page, checks the bindings they add, and
/// commits the edited page with its object changes (AT-05). A repeated key returns the first outcome.
fn edit(state: &ServerState, argument: &Argument, raw: &Value) -> AgentResult<Success> {
    let input: EditInput = schema::read("hmi_edit_draft", raw.clone())?;
    let store = DraftStore::open(&argument.storage)?;
    let draft = owned_draft(&store, argument, &input.draft_id)?;
    require_retained_pages(&draft)?;
    let file = selected_file(&draft, input.file.as_deref())?;
    let workspace = queries::load(state, argument, false)?;
    let allow_unresolved = input.allow_unresolved == Some(true);
    let fingerprint = json!({
        "file": file,
        "operations": input.operations,
        "allowUnresolved": allow_unresolved,
    });
    let call = EditCall {
        state,
        argument,
        workspace: &workspace,
        file: &file,
        operations: &input.operations,
        allow_unresolved,
        size: screen_size(&workspace.screen)?,
    };
    let edited = store.record_edit(
        &input.draft_id,
        &input.revision,
        &input.key,
        &fingerprint,
        |current| edit_file(&call, current),
    )?;
    let mut data = edited
        .data
        .unwrap_or_else(|| json!({ "file": file, "validation": "receipt-unavailable" }));
    if let Value::Object(fields) = &mut data {
        fields.insert("draftId".into(), json!(edited.draft.id));
        fields.insert("revision".into(), json!(edited.draft.revision));
    }
    let revisions = BTreeMap::from([(edited.draft.id.clone(), edited.draft.revision.clone())]);
    Ok(success(data, revisions))
}

/// What one edit of a page needs: the state, the project, the page, its operations and the design size.
struct EditCall<'a> {
    /// The server state the existing commands run on.
    state: &'a ServerState,
    /// The argument of the request.
    argument: &'a Argument,
    /// The project as the edit reads it.
    workspace: &'a Workspace,
    /// The page the edit changes.
    file: &'a str,
    /// The operations, applied in order.
    operations: &'a [Value],
    /// Whether unresolved process-first bindings may stay in the private draft.
    allow_unresolved: bool,
    /// The project's design size, which regional arrangement needs.
    size: [u32; 2],
}

/// Applies one edit to the draft's page and returns the new pages with the object changes
/// (`editDetailed`'s update in `service.ts`).
fn edit_file(call: &EditCall<'_>, current: &Draft) -> AgentResult<EditResult> {
    let source = current
        .files
        .get(call.file)
        .ok_or_else(|| AgentError::invalid("select a file in the draft."))?;
    let before = native::page(call.state, source, call.file)?;
    let model = edit_page(
        &before,
        call.operations,
        &call.workspace.catalogue,
        Some(call.size),
    )?;
    let text = native::format_page(call.state, &model)?;
    let after = native::page(call.state, &text, call.file)?;
    let bound = bind_ids(call.operations);
    if !bound.is_empty() && !call.allow_unresolved {
        let problems = native::file_problems(
            call.state,
            &call.argument.root_uri,
            call.file,
            &text,
            &call.workspace.remote,
        )?;
        let invalid: Vec<&str> = problems
            .iter()
            .filter(|problem| bound.contains(text_of(problem, "object")))
            .filter_map(|problem| problem.get("message").and_then(Value::as_str))
            .collect();
        if !invalid.is_empty() {
            return Err(AgentError::validation(invalid.join("; ")));
        }
    }
    let mut files = current.files.clone();
    files.insert(call.file.to_owned(), text);
    let change = with_field(
        encode(&describe_page_changes(Some(&before), &after))?,
        "file",
        json!(call.file),
    );
    Ok(EditResult {
        files,
        data: Some(json!({
            "file": call.file,
            "objectChanges": [change],
            "findings": [],
            "validation": "structural-only",
        })),
    })
}

/// The IDs of the objects an edit binds (`bindingEdits` of `service.ts`).
fn bind_ids(operations: &[Value]) -> BTreeSet<String> {
    operations
        .iter()
        .filter(|operation| operation.get("op").and_then(Value::as_str) == Some("bind"))
        .map(|operation| text_of(operation, "id").to_owned())
        .collect()
}

/// `hmi_validate_draft`: the engineering checks and the geometry and routing findings of a draft at its
/// revision, with its viewports and, when asked, the PLC proposals it needs (AT-06). Nothing is applied.
fn validate(state: &ServerState, argument: &Argument, raw: &Value) -> AgentResult<Success> {
    let input: ValidateInput = schema::read("hmi_validate_draft", raw.clone())?;
    let store = DraftStore::open(&argument.storage)?;
    let draft = owned_draft(&store, argument, &input.draft_id)?;
    at_revision(&draft, &input.revision)?;
    require_retained_pages(&draft)?;
    let workspace = queries::load(state, argument, false)?;
    let mut data = validate_draft(state, argument, &workspace, &draft, raw.get("viewports"))?;
    if input.propose_program == Some(true) {
        let mut proposals = Vec::with_capacity(draft.files.len());
        for (file, text) in &draft.files {
            let answer = native::missing_variables(state, &argument.root_uri, file, text)?;
            proposals.push(with_field(answer, "file", json!(file)));
        }
        data = with_field(
            data,
            "programProposals",
            json!({
                "applied": false,
                "requiresSeparateReview": true,
                "programRevision": workspace.program_revision,
                "proposals": proposals,
            }),
        );
    }
    let revisions = BTreeMap::from([(draft.id.clone(), draft.revision.clone())]);
    Ok(success(data, revisions))
}

/// The validation answer of a draft (`validate` in `service.ts`): the navigation, incompleteness, binding
/// and geometry findings of each page, the routing findings, the viewports and the engineering validity.
///
/// The editor's quality warnings (units, unbound objects, cut text, contrast) are not in this answer: the
/// editor asks `trust-lsp.hmiQuality` for them with the texts its drawing cuts (specification 39, §14.1 item 4).
pub(super) fn validate_draft(
    state: &ServerState,
    argument: &Argument,
    workspace: &Workspace,
    draft: &Draft,
    viewports: Option<&Value>,
) -> AgentResult<Value> {
    let profiles = viewport_profiles(viewports)?;
    let size = screen_size(&workspace.screen)?;
    let available: BTreeSet<String> = workspace
        .snapshots
        .iter()
        .map(|snapshot| snapshot.file.clone())
        .chain(draft.files.keys().cloned())
        .collect();
    let mut findings: Vec<Value> = Vec::new();
    let mut viewport_reviews: Vec<Value> = Vec::new();
    let mut omitted = 0_usize;
    for (file, text) in &draft.files {
        let page = native::page(state, text, file)?;
        let model = encode(&page)?;
        findings.extend(navigation_findings(file, &model, &available));
        if page.incomplete {
            findings.push(json!({
                "file": file,
                "severity": "error",
                "rule": "incomplete-design",
                "message": "This page is marked incomplete; finish its design and explicitly clear the marker before deployment.",
            }));
        }
        for problem in
            native::file_problems(state, &argument.root_uri, file, text, &workspace.remote)?
        {
            let finding = with_field(problem, "file", json!(file));
            findings.push(with_fields(
                finding,
                &[("severity", json!("error")), ("rule", json!("binding"))],
            ));
        }
        for finding in routing_quality(&page, size) {
            findings.push(with_field(encode(&finding)?, "file", json!(file)));
        }
        let review = geometry_quality(&page, size, &workspace.catalogue, &profiles);
        for finding in review.findings {
            findings.push(with_field(encode(&finding)?, "file", json!(file)));
        }
        for viewport in review.viewports {
            viewport_reviews.push(with_field(encode(&viewport)?, "file", json!(file)));
        }
        omitted += review.omitted;
    }
    let engineering_valid = !findings
        .iter()
        .any(|finding| finding.get("severity").and_then(Value::as_str) == Some("error"));
    Ok(json!({
        "draftId": draft.id,
        "revision": draft.revision,
        "engineeringValid": engineering_valid,
        "findings": findings,
        "viewports": viewport_reviews,
        "geometryFindingsTruncated": omitted > 0,
        "omittedGeometryFindings": omitted,
        "visualReview": "required",
    }))
}

/// The navigation findings of a page: each page or popup link whose target the proposal does not hold.
fn navigation_findings(file: &str, model: &Value, available: &BTreeSet<String>) -> Vec<Value> {
    let mut found = Vec::new();
    for object in model
        .get("object")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = text_of(object, "id");
        let Some(click) = object.get("click").and_then(Value::as_object) else {
            continue;
        };
        for (kind, target) in click {
            let folder = match kind.as_str() {
                "page" => "pages",
                "popup" => "popups",
                _ => continue,
            };
            let Some(target) = target.as_str() else {
                continue;
            };
            let destination = format!("hmi/{folder}/{target}.toml");
            if !available.contains(&destination) {
                found.push(json!({
                    "file": file,
                    "object": id,
                    "severity": "error",
                    "rule": "missing-navigation-target",
                    "message": format!("{id} links to missing {destination}; choose an existing target or include it in this proposal."),
                }));
            }
        }
    }
    found
}

/// `hmi_render_draft`, `hmi_preview_states` and the others the extension renders: their input is checked
/// here, the rendering is the extension's.
fn check_input(argument: &Argument, raw: &Value) -> AgentResult<Success> {
    let tool = argument
        .raw
        .get("tool")
        .and_then(Value::as_str)
        .ok_or_else(|| AgentError::invalid("tool is required."))?;
    schema::check_tool_input(tool, raw.clone())?;
    Ok(success(
        json!({ "tool": tool, "valid": true }),
        BTreeMap::new(),
    ))
}

/// The draft `id` of this workspace. A draft of another workspace is refused; a discarded draft is
/// `EXPIRED_DRAFT` (from the store).
///
/// # Errors
///
/// `PROJECT_UNAVAILABLE` for another workspace; the store's refusals for a missing or discarded draft.
pub(super) fn owned_draft(store: &DraftStore, argument: &Argument, id: &str) -> AgentResult<Draft> {
    let draft = store.get(id)?;
    if draft.workspace_id != argument.workspace_id {
        return Err(AgentError::new(
            PROJECT_UNAVAILABLE,
            "draft belongs to another workspace.",
        ));
    }
    Ok(draft)
}

/// Refuses a request made against another revision of the draft (`draft()` of `service.ts`).
///
/// # Errors
///
/// `CONFLICT` when the revision changed.
pub(super) fn at_revision(draft: &Draft, revision: &str) -> AgentResult<()> {
    if draft.revision != revision {
        return Err(AgentError::new(CONFLICT, "draft revision changed."));
    }
    Ok(())
}

/// The page an edit or a validation names: the one the request gives, or the draft's only page
/// (`selectedFile` of `service.ts`).
///
/// # Errors
///
/// `INVALID_ARGUMENT` when no page is named or the named page is not in the draft.
pub(super) fn selected_file(draft: &Draft, file: Option<&str>) -> AgentResult<String> {
    let chosen = match file {
        Some(file) => Some(file.to_owned()),
        None if draft.files.len() == 1 => draft.files.keys().next().cloned(),
        None => None,
    };
    match chosen {
        Some(file) if draft.files.contains_key(&file) => Ok(file),
        _ => Err(AgentError::invalid("select a file in the draft.")),
    }
}

/// The page size of a screen model (`screen.size`).
///
/// # Errors
///
/// `VALIDATION_FAILED` when the screen has no two-part size.
pub(super) fn screen_size(screen: &Value) -> AgentResult<[u32; 2]> {
    let sides = screen
        .get("size")
        .and_then(Value::as_array)
        .filter(|sides| sides.len() == 2)
        .ok_or_else(|| AgentError::validation("the screen has no design size."))?;
    let side = |value: &Value| {
        value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .ok_or_else(|| AgentError::validation("the screen size is not a whole number."))
    };
    Ok([side(&sides[0])?, side(&sides[1])?])
}

/// The success answer of an operation.
#[must_use]
pub(super) fn success(data: Value, revisions: BTreeMap<String, String>) -> Success {
    Success {
        data,
        revisions,
        warnings: Vec::new(),
    }
}

/// A value encoded as JSON, a refusal when it cannot be.
///
/// # Errors
///
/// `VALIDATION_FAILED` when the value cannot be encoded.
pub(super) fn encode<T: Serialize>(value: &T) -> AgentResult<Value> {
    serde_json::to_value(value).map_err(|error| AgentError::validation(error.to_string()))
}

/// One page of a composed model, encoded.
fn encode_model(models: &BTreeMap<String, HmiPage>, file: &str) -> AgentResult<Value> {
    models
        .get(file)
        .ok_or_else(|| AgentError::validation("the composed page is missing."))
        .and_then(encode)
}

/// A JSON object with one field set; another value is returned unchanged.
#[must_use]
pub(super) fn with_field(mut value: Value, key: &str, field: Value) -> Value {
    if let Value::Object(fields) = &mut value {
        fields.insert(key.to_owned(), field);
    }
    value
}

/// A JSON object with several fields set, in order.
#[must_use]
pub(super) fn with_fields(mut value: Value, fields: &[(&str, Value)]) -> Value {
    for (key, field) in fields {
        value = with_field(value, key, field.clone());
    }
    value
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
