//! The visual-program commands of the language server (specification 25 §11.4, specification 17):
//! the Ladder, Blockly, Statechart and SFC editors reach every rule about their files through these
//! commands, and draw what they answer.
//!
//! - `trust-lsp.visualCompanion` `{ file, text }`: the Structured Text companion and runtime entry.
//! - `trust-lsp.visualSources` `{}`: the glob of every visual program, for the file search.
//! - `trust-lsp.visualValidate` `{ file? | kind?, text }`: the editor's Validate.
//! - `trust-lsp.visualParse` `{ kind, text, now? }`: a file as the editor's model (Ladder
//!   `programText`, Statechart and SFC `graphText`).
//! - `trust-lsp.visualFormat` `{ kind, programText | graphText, previousText?, now? }`: a model as
//!   file text; a Statechart or SFC saved over `previousText` keeps that file's id, name and
//!   metadata.
//! - `trust-lsp.visualEdit` `{ kind, op, … }`: an edit the editor stores in the file, or the value
//!   it stores for text typed into a field.
//! - `trust-lsp.visualNew` `{ kind, name, created?, checkOnly? }`: a new program.
//! - `trust-lsp.visualImport` `{ kind, file, text }`: the target of an import.
//!
//! Models travel as JSON text (`programText`, `graphText`), so the member order of the file
//! survives the round trip. This module decodes the arguments and encodes the answers; every rule
//! is in `trust-visual`.

use serde_json::{json, Value};
use trust_visual::companion::{companion, CompanionError};
use trust_visual::ladder::edit as ladder_edit;
use trust_visual::{kind, Json, Problem, VisualKind};

/// The companion and runtime entry of a visual program.
pub const VISUAL_COMPANION_COMMAND: &str = "trust-lsp.visualCompanion";
/// The glob of every visual program.
pub const VISUAL_SOURCES_COMMAND: &str = "trust-lsp.visualSources";
/// The editor's Validate.
pub const VISUAL_VALIDATE_COMMAND: &str = "trust-lsp.visualValidate";
/// A file as the editor's model.
pub const VISUAL_PARSE_COMMAND: &str = "trust-lsp.visualParse";
/// A model as file text.
pub const VISUAL_FORMAT_COMMAND: &str = "trust-lsp.visualFormat";
/// An edit the editor stores in the file.
pub const VISUAL_EDIT_COMMAND: &str = "trust-lsp.visualEdit";
/// A new program.
pub const VISUAL_NEW_COMMAND: &str = "trust-lsp.visualNew";
/// The target of an import.
pub const VISUAL_IMPORT_COMMAND: &str = "trust-lsp.visualImport";

/// The visual-program commands, in the order the server advertises them. Each name is one
/// camel-case segment after `trust-lsp.`, so it never collides with an extension command such as
/// `trust-lsp.visual.syncCompanionSt`.
pub const VISUAL_COMMANDS: [&str; 8] = [
    VISUAL_COMPANION_COMMAND,
    VISUAL_SOURCES_COMMAND,
    VISUAL_VALIDATE_COMMAND,
    VISUAL_PARSE_COMMAND,
    VISUAL_FORMAT_COMMAND,
    VISUAL_EDIT_COMMAND,
    VISUAL_NEW_COMMAND,
    VISUAL_IMPORT_COMMAND,
];

/// Whether `command` is a visual-program command.
pub(super) fn is_command(command: &str) -> bool {
    VISUAL_COMMANDS.contains(&command)
}

/// The answer of the visual-program command `command` to `arguments` (one object).
pub(super) fn execute(command: &str, arguments: Vec<Value>) -> Value {
    let args = match single_object(command, arguments) {
        Ok(args) => args,
        Err(message) => return refused(&message),
    };
    let answer = match command {
        VISUAL_COMPANION_COMMAND => companion_answer(&args),
        VISUAL_SOURCES_COMMAND => Ok(json!({ "ok": true, "glob": kind::source_glob() })),
        VISUAL_VALIDATE_COMMAND => validate_answer(&args),
        VISUAL_PARSE_COMMAND => parse_answer(&args),
        VISUAL_FORMAT_COMMAND => format_answer(&args),
        VISUAL_EDIT_COMMAND => edit_answer(&args),
        VISUAL_NEW_COMMAND => new_answer(&args),
        VISUAL_IMPORT_COMMAND => import_answer(&args),
        _ => Err(format!("{command} is not a visual-program command.")),
    };
    answer.unwrap_or_else(|message| refused(&message))
}

/// The one argument object of a command.
fn single_object(command: &str, arguments: Vec<Value>) -> Result<Json, String> {
    let [argument] = <[Value; 1]>::try_from(arguments)
        .map_err(|_| format!("{command} expects one argument object."))?;
    let argument = Json::from_serde(&argument);
    if argument.is_object() {
        Ok(argument)
    } else {
        Err(format!("{command} expects one argument object."))
    }
}

fn text_arg<'a>(args: &'a Json, key: &str) -> Result<&'a str, String> {
    args.str_at(key)
        .ok_or_else(|| format!("The argument `{key}` must be a string."))
}

fn number_arg(args: &Json, key: &str) -> Result<f64, String> {
    args.get(key)
        .and_then(Json::as_finite)
        .ok_or_else(|| format!("The argument `{key}` must be a finite number."))
}

fn index_arg(args: &Json, key: &str) -> Result<usize, String> {
    let number = number_arg(args, key)?;
    if number < 0.0 || number.fract() != 0.0 {
        return Err(format!(
            "The argument `{key}` must be a whole number from 0."
        ));
    }
    format!("{number}")
        .parse::<usize>()
        .map_err(|_| format!("The argument `{key}` is too large."))
}

/// The kind named by `kind`, or else the kind of the file `file`.
fn kind_arg(args: &Json) -> Result<VisualKind, String> {
    if let Some(name) = args.str_at("kind") {
        return VisualKind::from_name(name).ok_or_else(|| {
            format!("Unknown visual program kind \"{name}\"; expected ladder, blockly, statechart or sfc.")
        });
    }
    let file = text_arg(args, "file")?;
    VisualKind::of_file(file).ok_or_else(|| not_visual_message(file))
}

fn not_visual_message(file: &str) -> String {
    format!(
        "{} is not a visual program (*.ladder.json, *.blockly.json, *.statechart.json, *.sfc.json, *.sfc).",
        kind::file_name(file)
    )
}

fn parsed(text: &str) -> Result<Json, String> {
    Json::parse(text)
}

/// `{ ok: false, errors: [{ message }] }`.
fn refused(message: &str) -> Value {
    json!({ "ok": false, "errors": [{ "message": message }] })
}

fn problems_json(problems: &[Problem]) -> Value {
    json!(problems)
}

fn companion_answer(args: &Json) -> Result<Value, String> {
    let file = text_arg(args, "file")?;
    let text = text_arg(args, "text")?;
    Ok(match companion(file, text) {
        Ok(files) => json!({
            "ok": true,
            "kind": files.kind.name(),
            "companion": { "file": files.companion_file, "text": files.companion_text },
            "runtimeEntry": { "file": files.runtime_entry_file, "text": files.runtime_entry_text },
            "warnings": files.warnings,
        }),
        Err(CompanionError::NotVisual) => json!({
            "ok": false,
            "notVisual": true,
            "errors": [{ "message": not_visual_message(file) }],
        }),
        Err(CompanionError::Invalid(kind, problems)) => json!({
            "ok": false,
            "kind": kind.name(),
            "errors": problems_json(&problems),
        }),
    })
}

fn validate_answer(args: &Json) -> Result<Value, String> {
    let kind = kind_arg(args)?;
    let program = match parsed(text_arg(args, "text")?) {
        Ok(program) => program,
        Err(message) => {
            return Ok(
                json!({ "ok": false, "kind": kind.name(), "errors": [{ "message": message }] }),
            )
        }
    };
    let base = args
        .str_at("file")
        .map_or_else(|| kind.title().to_owned(), kind::base_name);
    Ok(match kind {
        VisualKind::Ladder => verdict(kind, trust_visual::ladder::validate(&program)),
        VisualKind::Statechart => verdict(kind, trust_visual::statechart::validate(&program)),
        VisualKind::Sfc => match trust_visual::sfc::validate(&program) {
            Ok(issues) if issues.is_empty() => json!({ "ok": true, "kind": kind.name() }),
            Ok(issues) => {
                let errors: Vec<Value> = issues
                    .iter()
                    .map(|issue| json!({ "message": issue.message, "element": issue.element }))
                    .collect();
                json!({ "ok": false, "kind": kind.name(), "errors": errors, "issues": issues })
            }
            Err(message) => {
                json!({ "ok": false, "kind": kind.name(), "errors": [{ "message": message }] })
            }
        },
        VisualKind::Blockly => match trust_visual::blockly::generate(&program, &base) {
            Ok(generated) => json!({
                "ok": true,
                "kind": kind.name(),
                "warnings": generated.warnings,
                "preview": generated.function_block,
            }),
            Err(problems) => {
                json!({ "ok": false, "kind": kind.name(), "errors": problems_json(&problems) })
            }
        },
    })
}

fn verdict(kind: VisualKind, result: Result<(), String>) -> Value {
    match result {
        Ok(()) => json!({ "ok": true, "kind": kind.name() }),
        Err(message) => {
            json!({ "ok": false, "kind": kind.name(), "errors": [{ "message": message }] })
        }
    }
}

fn parse_answer(args: &Json) -> Result<Value, String> {
    let kind = kind_arg(args)?;
    let text = text_arg(args, "text")?;
    let answer = match kind {
        VisualKind::Ladder => {
            // A file that cannot be read opens as an empty program the editor names so.
            let now = text_arg(args, "now")?;
            return Ok(match trust_visual::ladder::load(text, now) {
                Ok(program) => {
                    json!({ "ok": true, "kind": kind.name(), "programText": program.to_compact() })
                }
                Err(message) => json!({
                    "ok": false,
                    "kind": kind.name(),
                    "errors": [{ "message": message }],
                    "programText": trust_visual::ladder::empty_program(
                        trust_visual::ladder::INVALID_PROGRAM_NAME,
                        now,
                    )
                    .to_compact(),
                }),
            });
        }
        // The SFC graph travels as text, so the member order of its actions and variables survives
        // the round trip to the file.
        VisualKind::Sfc => parsed(text)
            .and_then(|file| trust_visual::sfc::graph(&file))
            .map(
                |graph| json!({ "ok": true, "kind": kind.name(), "graphText": graph.to_compact() }),
            ),
        // The statechart graph travels as text too: its action mappings keep their member order.
        VisualKind::Statechart => parsed(text)
            .and_then(|file| trust_visual::statechart::graph(&file))
            .map(
                |graph| json!({ "ok": true, "kind": kind.name(), "graphText": graph.to_compact() }),
            ),
        VisualKind::Blockly => Err("The Blockly editor reads its file itself.".to_owned()),
    };
    Ok(answer.unwrap_or_else(
        |message| json!({ "ok": false, "kind": kind.name(), "errors": [{ "message": message }] }),
    ))
}

/// The file a save replaces (`previousText`), when the editor sent one that is JSON.
fn previous_file(args: &Json) -> Option<Json> {
    args.str_at("previousText")
        .and_then(|text| Json::parse(text).ok())
}

fn format_answer(args: &Json) -> Result<Value, String> {
    let kind = kind_arg(args)?;
    let answer = match kind {
        VisualKind::Ladder => {
            let now = text_arg(args, "now")?;
            parsed(text_arg(args, "programText")?)
                .and_then(|program| trust_visual::ladder::save_text(&program, now))
        }
        VisualKind::Sfc => {
            let now = text_arg(args, "now")?;
            parsed(text_arg(args, "graphText")?)
                .and_then(|graph| trust_visual::sfc::file(&graph, now))
                .map(|mut file| {
                    if let Some(previous) = previous_file(args) {
                        trust_visual::sfc::keep_identity(&mut file, &previous, now);
                    }
                    file.to_pretty()
                })
        }
        VisualKind::Statechart => parsed(text_arg(args, "graphText")?)
            .and_then(|graph| trust_visual::statechart::file(&graph))
            .map(|mut file| {
                if let Some(previous) = previous_file(args) {
                    trust_visual::statechart::keep_identity(&mut file, &previous);
                }
                file.to_pretty()
            }),
        VisualKind::Blockly => Err("The Blockly editor writes its file itself.".to_owned()),
    };
    Ok(match answer {
        Ok(text) => json!({ "ok": true, "kind": kind.name(), "text": text }),
        Err(message) => {
            json!({ "ok": false, "kind": kind.name(), "errors": [{ "message": message }] })
        }
    })
}

fn edit_answer(args: &Json) -> Result<Value, String> {
    let kind = kind_arg(args)?;
    let op = text_arg(args, "op")?;
    match kind {
        VisualKind::Ladder => ladder_edit_answer(args, op),
        VisualKind::Sfc => sfc_edit_answer(args, op),
        VisualKind::Statechart => statechart_edit_answer(args, op),
        VisualKind::Blockly => Err(format!("The {} editor has no edit \"{op}\".", kind.title())),
    }
}

/// A Statechart request: `actionReport` (`graphText`), the actions named without a mapping and
/// the mappings nothing names; `afterDelay` (`text`), the delay typed into a transition's timer
/// field (`after`, `null` for none); `mappingValue` (`text`), the value typed into an action
/// mapping (`value`).
fn statechart_edit_answer(args: &Json, op: &str) -> Result<Value, String> {
    match op {
        "actionReport" => {
            let graph = parsed(text_arg(args, "graphText")?)?;
            let report = trust_visual::statechart::action_report(&graph);
            Ok(json!({ "ok": true, "unmapped": report.unmapped, "unused": report.unused }))
        }
        "afterDelay" => Ok(
            match trust_visual::statechart::after_delay(text_arg(args, "text")?) {
                Ok(after) => {
                    json!({ "ok": true, "after": after.map(|ms| Json::Number(ms).to_serde()) })
                }
                Err(message) => edit_refused("refused", &message),
            },
        ),
        "mappingValue" => {
            let value = trust_visual::statechart::mapping_value(text_arg(args, "text")?);
            Ok(json!({ "ok": true, "value": value.to_serde() }))
        }
        _ => Err(format!("The Statechart editor has no edit \"{op}\".")),
    }
}

/// A refused edit: `{ ok: false, error: <code>, errors: [{ message }] }`.
fn edit_refused(code: &str, message: &str) -> Value {
    json!({ "ok": false, "error": code, "errors": [{ "message": message }] })
}

fn ladder_edit_answer(args: &Json, op: &str) -> Result<Value, String> {
    let mut program = parsed(text_arg(args, "programText")?)?;
    let mut extra = serde_json::Map::new();
    match op {
        "reconcileDeclarations" => ladder_edit::reconcile_declarations(&mut program),
        "autoRoute" => {
            let rung = if args.get("rungIndex").is_some() {
                Some(index_arg(args, "rungIndex")?)
            } else {
                None
            };
            ladder_edit::auto_route(&mut program, rung);
        }
        "replaceSymbol" => {
            let count = ladder_edit::replace_symbol(
                &mut program,
                text_arg(args, "find")?,
                text_arg(args, "replace")?,
            );
            extra.insert("replacements".to_owned(), json!(count));
        }
        "addParallelContact" => {
            let rung = index_arg(args, "rungIndex")?;
            match ladder_edit::add_parallel_contact(
                &mut program,
                rung,
                text_arg(args, "contactId")?,
            ) {
                Ok(leg) => {
                    extra.insert("selectedNodeId".to_owned(), json!(leg.selected_node_id));
                    extra.insert("rungShiftPx".to_owned(), json!(leg.rung_shift_px));
                    extra.insert(
                        "addedVerticalSpanPx".to_owned(),
                        json!(leg.added_vertical_span_px),
                    );
                    extra.insert(
                        "totalParallelLegs".to_owned(),
                        json!(leg.total_parallel_legs),
                    );
                }
                Err(error) => return Ok(edit_refused(error.code(), &error.to_string())),
            }
        }
        "pasteElement" => {
            let element = args
                .get("element")
                .ok_or("The argument `element` is missing.")?;
            let index =
                ladder_edit::paste_element(&mut program, index_arg(args, "rungIndex")?, element)?;
            extra.insert("insertedIndex".to_owned(), json!(index));
        }
        "pasteRung" => {
            let rung = args.get("rung").ok_or("The argument `rung` is missing.")?;
            let after = number_arg(args, "afterIndex")?;
            let after = format!("{}", after.trunc()).parse::<i64>().unwrap_or(-1);
            let index = ladder_edit::paste_rung(&mut program, rung, after);
            extra.insert("insertedIndex".to_owned(), json!(index));
        }
        "insertElement" => {
            let index = ladder_edit::insert_element(
                &mut program,
                index_arg(args, "rungIndex")?,
                text_arg(args, "tool")?,
                number_arg(args, "x")?,
                number_arg(args, "y")?,
            )?;
            extra.insert("insertedIndex".to_owned(), json!(index));
        }
        "addRung" => {
            let index = ladder_edit::add_rung(&mut program, index_arg(args, "index")?);
            extra.insert("insertedIndex".to_owned(), json!(index));
        }
        "removeRung" => ladder_edit::remove_rung(&mut program, index_arg(args, "index")?)?,
        "removeElement" => ladder_edit::remove_element(
            &mut program,
            index_arg(args, "rungIndex")?,
            index_arg(args, "elementIndex")?,
        )?,
        "clearWiring" => ladder_edit::clear_wiring(&mut program, index_arg(args, "rungIndex")?)?,
        "toggleContactType" => {
            let contact_type = ladder_edit::toggle_contact_type(
                &mut program,
                index_arg(args, "rungIndex")?,
                index_arg(args, "elementIndex")?,
            )?;
            extra.insert("contactType".to_owned(), json!(contact_type));
        }
        "connect" => {
            let wire = ladder_edit::connect(
                &mut program,
                index_arg(args, "rungIndex")?,
                text_arg(args, "fromNodeId")?,
                text_arg(args, "toNodeId")?,
            );
            extra.insert("wire".to_owned(), json!(wire.name()));
        }
        "setNumber" => return ladder_number_answer(&mut program, args),
        "setField" => return ladder_field_answer(&mut program, args),
        _ => return Err(format!("The Ladder editor has no edit \"{op}\".")),
    }
    let mut answer = serde_json::Map::new();
    answer.insert("ok".to_owned(), json!(true));
    answer.insert("programText".to_owned(), json!(program.to_compact()));
    answer.extend(extra);
    Ok(Value::Object(answer))
}

/// The Ladder `setField` edit: the text or choice `value` of the member `field` of the element
/// `elementIndex` of the rung `rungIndex`, typed or chosen in the properties panel, answered with
/// the program. A value the field does not take is refused with the reason, and nothing is stored.
fn ladder_field_answer(program: &mut Json, args: &Json) -> Result<Value, String> {
    Ok(
        match ladder_edit::set_field(
            program,
            index_arg(args, "rungIndex")?,
            index_arg(args, "elementIndex")?,
            text_arg(args, "field")?,
            text_arg(args, "value")?,
        ) {
            Ok(()) => json!({ "ok": true, "programText": program.to_compact() }),
            Err(message) => edit_refused("refused", &message),
        },
    )
}

/// The Ladder `setNumber` edit: the text typed into a number field of the properties panel
/// (`field`: `presetMs`, `preset`, `x` or `y`; `text`) stored in the element `elementIndex` of the
/// rung `rungIndex`, answered with the program and the stored `value`. Text that is no number is
/// refused with the reason, and nothing is stored.
fn ladder_number_answer(program: &mut Json, args: &Json) -> Result<Value, String> {
    let name = text_arg(args, "field")?;
    let field = ladder_edit::NumberField::from_name(name)
        .ok_or_else(|| format!("The Ladder properties panel has no number field \"{name}\"."))?;
    Ok(
        match ladder_edit::set_number(
            program,
            index_arg(args, "rungIndex")?,
            index_arg(args, "elementIndex")?,
            field,
            text_arg(args, "text")?,
        ) {
            Ok(value) => json!({
                "ok": true,
                "programText": program.to_compact(),
                "value": Json::Number(value).to_serde(),
            }),
            Err(message) => edit_refused("refused", &message),
        },
    )
}

/// An SFC edit on the canvas graph `graphText`: `connect` (`connection`, `fallback`, `stamp`),
/// `setBranchCount` (`nodeId`, `branchCount`) or `addNode` (`nodeType`, `position`, `wire`,
/// `stamp`).
fn sfc_edit_answer(args: &Json, op: &str) -> Result<Value, String> {
    let graph = parsed(text_arg(args, "graphText")?)?;
    match op {
        "connect" => {
            let connection = args
                .get("connection")
                .ok_or("The argument `connection` is missing.")?;
            let fallback = args.get("fallback").cloned().unwrap_or_else(Json::object);
            let stamp = text_arg(args, "stamp")?;
            Ok(
                match trust_visual::sfc::connect(&graph, connection, &fallback, stamp) {
                    Ok(edge) => json!({ "ok": true, "edge": edge.to_serde() }),
                    Err(reason) => edit_refused("refused", &reason),
                },
            )
        }
        "setBranchCount" => {
            let count = trust_visual::sfc::set_branch_count(
                &graph,
                text_arg(args, "nodeId")?,
                args.get("branchCount"),
            )?;
            let removed: Vec<Value> = count.removed_edges.iter().map(Json::to_serde).collect();
            Ok(json!({
                "ok": true,
                "branchCount": Json::Number(count.branch_count).to_serde(),
                "removedEdgeIds": removed,
            }))
        }
        "addNode" => {
            let name = text_arg(args, "nodeType")?;
            let node_type = trust_visual::sfc::NodeType::from_name(name)
                .ok_or_else(|| format!("Unknown SFC node type \"{name}\"."))?;
            let position = args
                .get("position")
                .ok_or("The argument `position` is missing.")?;
            let wire = args.get("wire").and_then(Json::as_bool) == Some(true);
            let stamp = text_arg(args, "stamp")?;
            let added = trust_visual::sfc::add_node(&graph, node_type, position, wire, stamp)?;
            Ok(json!({
                "ok": true,
                "node": added.node.to_serde(),
                "edge": added.edge.as_ref().map(Json::to_serde),
            }))
        }
        _ => Err(format!("The SFC editor has no edit \"{op}\".")),
    }
}

fn new_answer(args: &Json) -> Result<Value, String> {
    let kind = kind_arg(args)?;
    let name = text_arg(args, "name")?;
    if args.get("checkOnly").and_then(Json::as_bool) == Some(true) {
        return Ok(match trust_visual::new_program::check_name(kind, name) {
            None => json!({ "ok": true }),
            Some(message) => json!({ "ok": false, "errors": [{ "message": message }] }),
        });
    }
    let created = args.str_at("created").unwrap_or_default();
    Ok(
        match trust_visual::new_program::new_program(kind, name, created) {
            Ok(program) => {
                json!({ "ok": true, "kind": kind.name(), "fileName": program.file_name, "text": program.text })
            }
            Err(message) => json!({ "ok": false, "errors": [{ "message": message }] }),
        },
    )
}

fn import_answer(args: &Json) -> Result<Value, String> {
    let kind = kind_arg(args)?;
    let file = text_arg(args, "file")?;
    let text = text_arg(args, "text")?;
    // The workspace folder, when there is one: a file inside it may be opened where it is.
    let workspace = args.str_at("workspace").unwrap_or_default();
    Ok(
        match trust_visual::import::import_target(kind, file, text) {
            Ok(target) => json!({
                "ok": true,
                "kind": kind.name(),
                "targetFile": target,
                "inWorkspace": trust_visual::import::inside_workspace(file, workspace),
            }),
            Err(message) => {
                json!({ "ok": false, "kind": kind.name(), "errors": [{ "message": message }] })
            }
        },
    )
}

#[cfg(test)]
#[path = "visual_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "visual_sfc_tests.rs"]
mod sfc_tests;

#[cfg(test)]
#[path = "visual_sfc_run_tests.rs"]
mod sfc_run_tests;

#[cfg(test)]
#[path = "visual_statechart_run_tests.rs"]
mod statechart_run_tests;
