//! Faceplates placed on a page (specification 39, §8.9): the instances a faceplate fits, for the
//! editor to offer when one is placed.

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::{TextDocumentIdentifier, Url};

use trust_runtime::hmi::bundle::HmiBundle;
use trust_runtime::hmi::faceplate_fit::placed_faceplate;
use trust_runtime::hmi::page_files::PLACED_FACEPLATE;

use super::hmi_files::{file_errors, refused, single_argument};
use super::{
    project_inputs, HmiBindingsCommandArgs, HMI_FACEPLATE_INSTANCES_COMMAND,
    HMI_PLACED_FACEPLATES_COMMAND,
};
use crate::handlers::context::ServerContext;

/// The most instances offered.
const MAX_INSTANCES: usize = 500;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstancesArgs {
    #[serde(default)]
    open: std::collections::BTreeMap<String, String>,
    faceplate: String,
    #[serde(default)]
    root_uri: Option<Url>,
    #[serde(default)]
    text_document: Option<TextDocumentIdentifier>,
}

/// `trust-lsp.hmiFaceplateInstances`: `{ faceplate, root_uri?, text_document? }` to `{ ok,
/// instances }`, the paths of the function block instances the faceplate fits.
pub(super) fn instances_value_with_context<C: ServerContext>(
    context: &C,
    args: Vec<Value>,
) -> Value {
    let args: InstancesArgs = match single_argument(args, HMI_FACEPLATE_INSTANCES_COMMAND) {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let selection = HmiBindingsCommandArgs {
        open: args.open.clone(),
        root_uri: args.root_uri,
        text_document: args.text_document,
    };
    let project = match project_inputs(context, &selection, HMI_FACEPLATE_INSTANCES_COMMAND) {
        Ok(project) => project,
        Err(message) => return refused(message),
    };
    let bundle = match preview_bundle(&project.root, args.open) {
        Ok(bundle) => bundle,
        Err(error) => return refused(format!("the HMI files cannot be read: {error}")),
    };
    let faceplate = match super::hmi_authoring::bundle::faceplate(&bundle, &args.faceplate) {
        Some(Ok(faceplate)) => faceplate,
        Some(Err(_)) => {
            return refused(format!(
                "the faceplate \"{}\" does not load",
                args.faceplate
            ))
        }
        None => {
            return refused(format!(
                "the faceplate \"{}\" is not in the project or its libraries",
                args.faceplate
            ))
        }
    };
    let index = super::hmi_authoring::declarations::Declarations::read(&project.sources);
    let pairs = super::hmi_authoring::bundle::pairs_of(&bundle, &args.faceplate);
    let matches = |node: &trust_runtime::hmi::variable_tree::VariableNode| {
        if node.kind != trust_runtime::hmi::variable_tree::VariableKind::FunctionBlock {
            return false;
        }
        let pair = trust_runtime::hmi::faceplate_fit::choose_pair(&pairs, Some(&node.type_name));
        faceplate.fields.iter().all(|(field, definition)| {
            let member = pair
                .and_then(|pair| pair.fields.get(field))
                .map_or(field.as_str(), String::as_str);
            index
                .resolve(&trust_runtime::hmi::page_files::pair_member_path(
                    &node.path, member,
                ))
                .is_some_and(|value| {
                    trust_runtime::hmi::faceplate_fit::accepts_declared_kinds(
                        definition,
                        &value.kinds,
                    )
                })
        })
    };
    let instances = match super::hmi_authoring::declaration_walk::matching(
        &index.variables,
        MAX_INSTANCES,
        &matches,
    ) {
        Ok(nodes) => nodes.into_iter().map(|node| node.path).collect::<Vec<_>>(),
        Err(error) => return refused(error),
    };
    json!({ "ok": true, "instances": instances })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacedArgs {
    #[serde(default)]
    open: std::collections::BTreeMap<String, String>,
    text: String,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    root_uri: Option<Url>,
    #[serde(default)]
    text_document: Option<TextDocumentIdentifier>,
}

/// `trust-lsp.hmiPlacedFaceplates`: `{ text, file?, root_uri?, text_document? }`, a page's text,
/// to `{ ok, placed }`: each placed faceplate by object id, `{ model }` bound to its instance as
/// the display binds it, `null` where the project lacks the faceplate; one whose faceplate does
/// not load is left out.
pub(super) fn placed_value_with_context<C: ServerContext>(context: &C, args: Vec<Value>) -> Value {
    let args: PlacedArgs = match single_argument(args, HMI_PLACED_FACEPLATES_COMMAND) {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let file = args.file.as_deref().unwrap_or("(unsaved)");
    let parsed = if file.replace('\\', "/").contains("hmi/panels/") {
        super::hmi_authoring::documents::faceplate(file, &args.text).map(|panel| panel.popup.page)
    } else {
        super::hmi_authoring::documents::page(file, &args.text, false).map(|(page, _)| page)
    };
    let page = match parsed {
        Ok(page) => page,
        Err(errors) => return file_errors(&errors),
    };
    if !page
        .objects
        .iter()
        .any(|object| object.kind == PLACED_FACEPLATE)
    {
        return json!({ "ok": true, "placed": {} });
    }
    let selection = HmiBindingsCommandArgs {
        open: args.open.clone(),
        root_uri: args.root_uri,
        text_document: args.text_document,
    };
    let project = match project_inputs(context, &selection, HMI_PLACED_FACEPLATES_COMMAND) {
        Ok(project) => project,
        Err(message) => return refused(message),
    };
    let bundle = match preview_bundle(&project.root, args.open) {
        Ok(bundle) => bundle,
        Err(error) => return refused(format!("the HMI files cannot be read: {error}")),
    };
    let index = super::hmi_authoring::declarations::Declarations::read(&project.sources);
    let mut placed = serde_json::Map::new();
    for object in &page.objects {
        if object.kind != PLACED_FACEPLATE {
            continue;
        }
        let Some(name) = &object.faceplate else {
            continue;
        };
        let instance = object.instance.as_deref().unwrap_or("");
        let faceplate = match super::hmi_authoring::bundle::faceplate(&bundle, name) {
            Some(Ok(faceplate)) => faceplate,
            // The editor draws it as a faceplate the project lacks.
            None => {
                placed.insert(object.id.clone(), Value::Null);
                continue;
            }
            Some(Err(_)) => continue,
        };
        let Some((file, bytes)) = super::hmi_authoring::bundle::faceplate_text(&bundle, name)
        else {
            continue;
        };
        let text = String::from_utf8_lossy(bytes);
        let model = match super::hmi_authoring::documents::parse("faceplate", file, &text) {
            Ok(model) => model,
            Err(errors) => return file_errors(&errors),
        };
        if object.instance.is_none() {
            if super::hmi_authoring::documents::requires_instance(&model) {
                return refused(format!("Select an equipment instance for panel {name}"));
            }
            placed.insert(object.id.clone(), json!({"model":model}));
            continue;
        }
        let pairs = super::hmi_authoring::bundle::pairs_of(&bundle, name);
        // The runtime binds the panel's fields, commands and settings to the instance (§8.9).
        if let Some(bound) = placed_faceplate(
            faceplate,
            &pairs,
            object,
            index.resolve(instance).map(|node| node.type_name.as_str()),
        ) {
            placed.insert(
                object.id.clone(),
                json!({"model": super::hmi_authoring::documents::faceplate_model(&bound)}),
            );
        }
    }
    json!({ "ok": true, "placed": placed })
}

/// Overlay editor text in memory; these keys are never used for filesystem access.
pub(super) fn preview_bundle(
    root: &std::path::Path,
    open: std::collections::BTreeMap<String, String>,
) -> std::io::Result<HmiBundle> {
    let snapshot = super::hmi_snapshot::paths(root, &open).map_err(std::io::Error::other)?;
    super::hmi_authoring::bundle::read_edited(root, &snapshot)
}
