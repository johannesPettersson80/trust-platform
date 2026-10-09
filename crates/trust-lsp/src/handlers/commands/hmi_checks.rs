//! Binding checks of HMI Builder pages for the editor (specification 39, §7.6).
//!
//! Continuous editor requests use declaration metadata. Explicit bundle/build requests retain
//! full compiled-program validation. Both return problems at the relevant document location.

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::{TextDocumentIdentifier, Url};
use trust_runtime::hmi::access::config::hmi_access_from_runtime_toml;
use trust_runtime::hmi::access::page_policy::check_page_policy;
use trust_runtime::hmi::binding_checks::{
    check_faceplate, check_pair, without_fields, BindingProblem,
};
use trust_runtime::hmi::bundle::checks::{check_bundle_with_remote, Program};
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::page_files::{problem_line, HmiFaceplate, PLACED_FACEPLATE};

use super::hmi_authoring::pairs::parse_pair;
use super::hmi_files::{file_errors, refused, single_argument};
use super::{compile_project, HmiBindingsCommandArgs, HMI_BUNDLE_COMMAND, HMI_FILE_CHECK_COMMAND};
use crate::handlers::context::ServerContext;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckArgs {
    #[serde(default)]
    authoring: bool,
    #[serde(default)]
    open: std::collections::BTreeMap<String, String>,
    text: String,
    #[serde(default)]
    remote: std::collections::BTreeMap<String, Value>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    root_uri: Option<Url>,
    #[serde(default)]
    text_document: Option<TextDocumentIdentifier>,
}

/// `trust-lsp.hmiFileCheck`: `{ text, file?, root_uri?, text_document? }` to the page's problems.
///
/// A page that does not load is answered like `trust-lsp.hmiFileParse`, with its errors.
pub(super) fn check_value_with_context<C: ServerContext>(context: &C, args: Vec<Value>) -> Value {
    let args: CheckArgs = match single_argument(args, HMI_FILE_CHECK_COMMAND) {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let file = args.file.as_deref().unwrap_or("(unsaved)");
    if args.authoring {
        return super::hmi_authoring::checks::check(
            context,
            &HmiBindingsCommandArgs {
                open: args.open.clone(),
                root_uri: args.root_uri.clone(),
                text_document: args.text_document.clone(),
            },
            &args.text,
            file,
            &args.remote,
        );
    }
    let folder = file.replace('\\', "/");
    if folder.contains("hmi/pairs/") {
        return pair_value(context, &args, file);
    }
    if folder.contains("hmi/symbols/") {
        let selection = HmiBindingsCommandArgs {
            open: args.open.clone(),
            root_uri: args.root_uri.clone(),
            text_document: args.text_document.clone(),
        };
        return super::hmi_symbols::check_value(context, &args.text, file, &selection);
    }
    // A popup's objects are checked as a page's are (§4.9); a faceplate's bind its fields (§8.7).
    let mut faceplate = None;
    let loaded = if folder.contains("hmi/popups/") {
        super::hmi_authoring::documents::page(file, &args.text, true).map(|(page, _)| page)
    } else if folder.contains("hmi/panels/") || folder.contains("hmi/panels/") {
        super::hmi_authoring::documents::faceplate(file, &args.text).map(|loaded| {
            let page = loaded.popup.page.clone();
            faceplate = Some(loaded);
            page
        })
    } else {
        super::hmi_authoring::documents::page(file, &args.text, false).map(|(page, _)| page)
    };
    let page = match loaded {
        Ok(page) => page,
        Err(errors) => return file_errors(&errors),
    };
    let selection = HmiBindingsCommandArgs {
        open: args.open.clone(),
        root_uri: args.root_uri,
        text_document: args.text_document,
    };
    let project = match compile_project(context, &selection, HMI_FILE_CHECK_COMMAND) {
        Ok(project) => project,
        Err(message) => return refused(message),
    };
    let metadata = project.runtime.metadata_snapshot();
    // The project's own symbols are components of its pages (§14.4).
    let catalogue = match super::hmi_symbols::project_catalogue(&project.root, &project.open) {
        Ok(catalogue) => catalogue,
        Err(error) => return refused(error),
    };
    let mut problems = match &faceplate {
        Some(faceplate) => {
            check_faceplate(faceplate, &catalogue, &metadata, project.runtime.storage())
        }
        None => trust_runtime::hmi::binding_checks::check_page_with_remote(
            &page,
            &catalogue,
            &metadata,
            project.runtime.storage(),
            &args.remote,
        ),
    };
    // A placed faceplate's faceplate and its fit to the instance, against the project's and its
    // libraries' faceplates and pairs (§8.9).
    if page
        .objects
        .iter()
        .any(|object| object.kind == PLACED_FACEPLATE)
    {
        if let Ok(bundle) = super::hmi_authoring::bundle::read_edited(&project.root, &project.open)
        {
            problems.extend(super::hmi_authoring::placements::check(
                &page,
                &bundle,
                project.runtime.storage(),
            ));
        }
    }
    // A control bound to a field is checked against the policy when its instance is known.
    problems.extend(policy_problems(
        &without_fields(&page),
        &project.root,
        &catalogue,
    ));
    let problems = problems
        .iter()
        .map(|problem| {
            json!({
                "line": problem_line(&args.text, &problem.object, problem.property.as_deref()),
                "object": problem.object,
                "property": problem.property,
                "message": problem.message,
            })
        })
        .collect::<Vec<_>>();
    json!({ "ok": true, "problems": problems })
}

/// A pair's link check (§8.6) against the compiled program: its problems at the `block` line, at
/// a mapped field's line, or at the `faceplate` line for a field of its own name.
fn pair_value<C: ServerContext>(context: &C, args: &CheckArgs, file: &str) -> Value {
    let pair = match parse_pair(file, &args.text) {
        Ok(pair) => pair,
        Err(errors) => return file_errors(&errors),
    };
    let selection = HmiBindingsCommandArgs {
        open: args.open.clone(),
        root_uri: args.root_uri.clone(),
        text_document: args.text_document.clone(),
    };
    let project = match compile_project(context, &selection, HMI_FILE_CHECK_COMMAND) {
        Ok(project) => project,
        Err(message) => return refused(message),
    };
    let line = |key: &str| key_line(&args.text, None, key);
    let mut problems: Vec<Value> = match faceplate_named(
        &project.root,
        &project.open,
        &pair.faceplate,
    ) {
        None => vec![json!({
            "line": line("faceplate"),
            "message": format!("the project and its libraries have no faceplate `{}`", pair.faceplate),
        })],
        Some(Err(errors)) => vec![json!({
            "line": line("faceplate"),
            "message": format!("the faceplate `{}` does not load: {}", pair.faceplate, errors.first().map(|error| error.message.clone()).unwrap_or_default()),
        })],
        Some(Ok(faceplate)) => check_pair(&pair, &faceplate, &project.runtime.metadata_snapshot())
            .into_iter()
            .map(|problem| {
                let at = match (&problem.field, &problem.binding) {
                    (_, Some(property)) => key_line(&args.text, Some("bind"), property),
                    (None, None) => line("block"),
                    (Some(field), None) if pair.fields.contains_key(field) => {
                        key_line(&args.text, Some("fields"), field)
                    }
                    (Some(_), None) => line("faceplate"),
                };
                json!({ "line": at, "object": problem.field, "message": problem.message })
            })
            .collect(),
    };
    if let Ok(bundle) = super::hmi_authoring::bundle::read_edited(&project.root, &project.open) {
        if let Some((panel_file, bytes)) =
            super::hmi_authoring::bundle::faceplate_text(&bundle, &pair.faceplate)
        {
            if let Ok(panel) = super::hmi_authoring::documents::parse(
                "faceplate",
                panel_file,
                &String::from_utf8_lossy(bytes),
            ) {
                problems.extend(super::hmi_authoring::command_checks::check(&project.runtime.metadata_snapshot(),&pair,&panel).into_iter()
                    .map(|(object,message)|json!({"line":line("faceplate"),"object":object,"message":message})));
            }
        }
    }
    json!({ "ok": true, "problems": problems })
}

/// A faceplate by its name: the project's own, else a library's (§8.7, item 3).
fn faceplate_named(
    root: &std::path::Path,
    open: &std::collections::BTreeMap<std::path::PathBuf, String>,
    name: &str,
) -> Option<Result<HmiFaceplate, Vec<trust_runtime::hmi::page_files::PageFileError>>> {
    match super::hmi_authoring::bundle::read_edited(root, open) {
        Ok(bundle) => super::hmi_authoring::bundle::faceplate(&bundle, name),
        Err(error) => Some(Err(vec![trust_runtime::hmi::page_files::PageFileError {
            file: "hmi".into(),
            line: 1,
            message: error.to_string(),
        }])),
    }
}

/// The one-based line of `key = …` at the top of a TOML text, or in its `[table]`; 1 when absent.
pub(super) fn key_line(text: &str, table: Option<&str>, key: &str) -> usize {
    let mut current: Option<String> = None;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(name) = trimmed
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            current = Some(name.trim().to_string());
            continue;
        }
        let in_table = match table {
            None => current.is_none(),
            Some(table) => current.as_deref() == Some(table),
        };
        let named = trimmed
            .split('=')
            .next()
            .map(|left| left.trim().trim_matches('"'));
        if in_table && named == Some(key) {
            return index + 1;
        }
    }
    1
}

/// Controls against the write policy of the project's `runtime.toml` (§12.2.3). Without the
/// file there is no policy, so nothing may be written; a policy that does not load is named.
pub(super) fn policy_problems(
    page: &trust_runtime::hmi::page_files::HmiPage,
    root: &std::path::Path,
    catalogue: &Catalogue,
) -> Vec<BindingProblem> {
    let text = std::fs::read_to_string(root.join("runtime.toml")).unwrap_or_default();
    match hmi_access_from_runtime_toml(&text) {
        Ok(access) => check_page_policy(page, catalogue, &access.write),
        Err(error) => check_page_policy(page, catalogue, &[])
            .into_iter()
            .map(|problem| BindingProblem {
                message: format!(
                    "{} (the write policy in runtime.toml does not load: {error})",
                    problem.message
                ),
                ..problem
            })
            .collect(),
    }
}

/// `trust-lsp.hmiBundle`: `{ root_uri?, text_document? }` to the project's HMI bundle, its
/// version and its problems against the compiled program and the project's write policy
/// (§16.2.1).
pub(super) fn bundle_value_with_context<C: ServerContext>(context: &C, args: Vec<Value>) -> Value {
    let args: BundleArgs = match single_argument(args, HMI_BUNDLE_COMMAND) {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let selection = HmiBindingsCommandArgs {
        open: args.open.clone(),
        root_uri: args.root_uri,
        text_document: args.text_document,
    };
    let project = match compile_project(context, &selection, HMI_BUNDLE_COMMAND) {
        Ok(project) => project,
        Err(message) => return refused(message),
    };
    let bundle = match super::hmi_authoring::bundle::read_edited(&project.root, &project.open) {
        Ok(bundle) => bundle,
        Err(error) => return refused(format!("the hmi folder cannot be read: {error}")),
    };
    let mut problems: Vec<Value> = Vec::new();
    let policy = match hmi_access_from_runtime_toml(
        &std::fs::read_to_string(project.root.join("runtime.toml")).unwrap_or_default(),
    ) {
        Ok(access) => access.write,
        Err(error) => {
            problems.push(json!({ "file": "runtime.toml", "line": 1, "message": format!("the write policy does not load: {error}") }));
            Vec::new()
        }
    };
    let metadata = project.runtime.metadata_snapshot();
    let program = Program {
        metadata: &metadata,
        storage: project.runtime.storage(),
    };
    problems.extend(
        check_bundle_with_remote(&bundle, Catalogue::shipped(), &program, &policy,&args.remote)
            .into_iter()
            .map(|problem| json!({ "file": problem.file, "line": problem.line, "message": problem.message })),
    );
    json!({ "ok": true, "bundle": bundle.to_json(), "version": bundle.version(), "problems": problems })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleArgs {
    #[serde(default)]
    open: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    root_uri: Option<Url>,
    #[serde(default)]
    text_document: Option<TextDocumentIdentifier>,
    #[serde(default)]
    remote: std::collections::BTreeMap<String, Value>,
}
