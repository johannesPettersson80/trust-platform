//! Create pages from program (specification 39, §9): an overview page with the symbol of each
//! instance whose block has an equipment pair, its symbol's properties bound to the instance's
//! declared members. Existing views are preserved, On click defaults to Nothing, and the PLC
//! program is neither compiled nor changed by Create HMI.

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;
use trust_hir::types::Type;
use trust_runtime::hmi::page_files::{parse_page, write_page};
use trust_runtime::program_model::VarDef;
use trust_runtime::RetainPolicy;

use super::hmi_files::{refused, single_argument};
use super::{
    compile_project, HmiBindingsCommandArgs, HMI_PROPOSE_PAGE_COMMAND,
    HMI_PROPOSE_SETTINGS_PAGE_COMMAND,
};
use crate::handlers::context::ServerContext;

/// Deepest nesting searched for equipment, and the deepest base chain followed.
const MAX_DEPTH: usize = 8;
/// The page's width the symbols are laid out in, its margin and the gap between symbols.
const MARGIN: u32 = 80;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposeArgs {
    #[serde(default)]
    open: std::collections::BTreeMap<String, String>,
    root_uri: Url,
}

/// `trust-lsp.hmiProposePage`: `{ root_uri }` to `{ ok, text, count }`, the proposed page's file
/// text and the number of symbols on it.
pub(super) fn propose_page_value<C: ServerContext>(context: &C, args: Vec<Value>) -> Value {
    let args: ProposeArgs = match single_argument(args, HMI_PROPOSE_PAGE_COMMAND) {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let selection = HmiBindingsCommandArgs {
        open: args.open.clone(),
        root_uri: Some(args.root_uri),
        text_document: None,
    };
    match super::project_inputs(context, &selection, HMI_PROPOSE_PAGE_COMMAND)
        .and_then(|inputs| super::hmi_authoring::propose::page(&inputs))
    {
        Ok(value) => value,
        Err(error) => refused(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsArgs {
    #[serde(default)]
    open: std::collections::BTreeMap<String, String>,
    root_uri: Url,
    #[serde(default)]
    program: Option<String>,
}

/// `trust-lsp.hmiProposeSettingsPage`: `{ root_uri, program? }` to `{ ok, text, count, skipped }`,
/// or `{ ok, choose }`, the program instances with settings, when several have some and none is
/// named (§9 item 4).
pub(super) fn propose_settings_page_value<C: ServerContext>(
    context: &C,
    args: Vec<Value>,
) -> Value {
    let args: SettingsArgs = match single_argument(args, HMI_PROPOSE_SETTINGS_PAGE_COMMAND) {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let selection = HmiBindingsCommandArgs {
        open: args.open.clone(),
        root_uri: Some(args.root_uri),
        text_document: None,
    };
    let project = match compile_project(context, &selection, HMI_PROPOSE_SETTINGS_PAGE_COMMAND) {
        Ok(project) => project,
        Err(message) => return refused(message),
    };
    let runtime = &project.runtime;
    // Each configured program instance with the settings its program declares retained (the
    // project's declarations, specification 44 §6.7).
    let with_settings: Vec<(String, Vec<&VarDef>)> = runtime
        .metadata()
        .programs()
        .iter()
        .filter_map(|(name, program)| {
            let settings: Vec<&VarDef> = program
                .vars
                .iter()
                .filter(|var| matches!(var.retain, RetainPolicy::Retain | RetainPolicy::Persistent))
                .collect();
            (!settings.is_empty()).then(|| (name.to_string(), settings))
        })
        .collect();
    let chosen = match &args.program {
        Some(program) => with_settings
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(program)),
        None if with_settings.len() == 1 => with_settings.first(),
        None if with_settings.is_empty() => None,
        None => {
            let names: Vec<&str> = with_settings
                .iter()
                .map(|(name, _)| name.as_str())
                .collect();
            return json!({ "ok": true, "choose": names });
        }
    };
    let Some((program, settings)) = chosen else {
        return json!({ "ok": true, "count": 0, "skipped": [] });
    };
    let mut text = String::from("format = 1\ntitle = \"Settings\"\nicon = \"settings\"\n");
    let mut skipped = Vec::new();
    let mut count = 0u32;
    for var in settings {
        let path = format!("{program}.{}", var.name);
        let (kind, options) = match setting_control(runtime.metadata().registry(), var.type_id) {
            Ok(control) => control,
            Err(reason) => {
                skipped.push(json!({ "variable": path, "message": reason }));
                continue;
            }
        };
        // Two columns of entries, filled row by row.
        let (x, y) = (MARGIN + (count % 2) * 480, MARGIN + 40 + (count / 2) * 80);
        count += 1;
        let id: String = var
            .name
            .chars()
            .flat_map(|character| {
                let lower = character.to_ascii_lowercase();
                let dash = character.is_ascii_uppercase().then_some('-');
                dash.into_iter().chain(std::iter::once(lower))
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string();
        text.push_str(&format!(
            "\n[[object]]\nid = \"{id}\"\ntype = \"{kind}\"\nat = [{x}, {y}]\nsize = [440, 56]\nlabel = \"{}\"\n",
            words_of(&var.name)
        ));
        if let Some(options) = options {
            let quoted: Vec<String> = options
                .iter()
                .map(|option| format!("\"{option}\""))
                .collect();
            text.push_str(&format!("options = [{}]\n", quoted.join(", ")));
        }
        text.push_str(&format!(
            "role = \"engineer\"\n\n[object.bind]\nvalue = {{ path = \"{path}\", direction = \"read-write\" }}\n"
        ));
    }
    match parse_page("hmi/pages/settings.toml", &text) {
        Ok(page) => {
            json!({ "ok": true, "text": write_page(&page), "count": count, "skipped": skipped })
        }
        Err(errors) => refused(format!(
            "{HMI_PROPOSE_SETTINGS_PAGE_COMMAND}: the proposed page does not load: {}",
            errors
                .first()
                .map(|error| error.message.clone())
                .unwrap_or_default()
        )),
    }
}

/// The control that sets a variable of a type, with an enumeration's options, or why none does.
fn setting_control(
    registry: &trust_hir::types::TypeRegistry,
    type_id: trust_hir::TypeId,
) -> Result<(&'static str, Option<Vec<String>>), &'static str> {
    let mut current = registry.get(type_id);
    for _ in 0..MAX_DEPTH {
        match current {
            Some(Type::Alias { target, .. }) => current = registry.get(*target),
            Some(Type::Bool) => return Ok(("switch", None)),
            Some(Type::Enum { values, .. }) => {
                return Ok((
                    "selector",
                    Some(values.iter().map(|(name, _)| name.to_string()).collect()),
                ))
            }
            Some(kind) if kind.is_numeric() => return Ok(("numeric-input", None)),
            Some(
                Type::Time
                | Type::LTime
                | Type::Date
                | Type::LDate
                | Type::Tod
                | Type::LTod
                | Type::Dt
                | Type::Ldt,
            ) => return Err("no control sets a time yet"),
            Some(Type::String { .. } | Type::WString { .. } | Type::Char | Type::WChar) => {
                return Err("no control sets a text yet")
            }
            _ => return Err("no control sets a value of this type"),
        }
    }
    Err("no control sets a value of this type")
}

/// `PumpEnabled` reads "Pump enabled", as the display shows option names (specification 39, §5.8).
pub(super) fn words_of(name: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let characters: Vec<char> = name.chars().collect();
    let mut current = String::new();
    for (index, character) in characters.iter().enumerate() {
        let boundary = character.is_ascii_uppercase()
            && !current.is_empty()
            && (characters.get(index - 1).is_some_and(|previous| {
                previous.is_ascii_lowercase() || previous.is_ascii_digit()
            }) || characters
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_lowercase())
                && current.chars().all(|c| c.is_ascii_uppercase()));
        if boundary || *character == '_' {
            words.push(std::mem::take(&mut current));
        }
        if *character != '_' {
            current.push(*character);
        }
    }
    words.push(current);
    words
        .into_iter()
        .filter(|word| !word.is_empty())
        .enumerate()
        .map(|(index, word)| {
            if index == 0 || (word.len() > 1 && word.chars().all(|c| c.is_ascii_uppercase())) {
                word
            } else {
                word.to_ascii_lowercase()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
