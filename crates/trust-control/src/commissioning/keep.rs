//! Keep in project (specification 35, B6.5 and §0.1b `keep`; specification 39 §21.4 item 6): the values a loop's parameter
//! record holds in the running PLC become the record's initial values in the project. These are the rules that
//! `keepInProject.ts` and the Keep part of `equipmentEdits.ts` held: the record a controller is configured from, the values to
//! keep, the reading of the language tool's replies, the settings an equipment keep writes, the application of a text edit, the
//! file URIs an edit names and the review identity. The rules do no I/O: `trust-control-studio` runs the language tool, reads the
//! files and writes them.

#![allow(clippy::module_name_repetitions)] // `KeepableRecord` and `KeepEquipment` name the Keep action this module decides.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::keep_texts;
use super::project_law::{gain_symbols, served_law};
use crate::failure::{Cause, Failure, NextAction};
use crate::json;
use crate::loop_definition::{ControllerLaw, LoopDefinition};

/// The parameter record Keep in project writes, and the fields of it that the loop binds. For a program's own law, the record is
/// the program instance that declares its gain variables, and the fields are those variables (C2, B6.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeepableRecord {
    /// The record's symbol as the controller is configured from it, for example `Main.Parameters`; or a program instance, `P1`.
    pub record: String,
    /// The bound fields of the record, by their names within it, in the binding's order; or a program's gain variables.
    pub fields: Vec<String>,
}

/// The record a `Control.PID` law is configured from, and the fields of it that the parameter binding names (`keepableRecord`,
/// keepInProject.ts). A program's own law keeps its gain variables, which the record names by their program instance (C2, B6.5).
///
/// # Errors
/// The loop has no PID law, no parameter binding, or binds no field of that record; or a gain of a program's own law is not a
/// variable of one program instance.
pub fn keepable_record(loop_definition: &LoopDefinition) -> Result<KeepableRecord, Failure> {
    if served_law(loop_definition).is_some() {
        return program_law_record(loop_definition);
    }
    let (Some(ControllerLaw::Pid(law)), Some(binding)) = (
        loop_definition.controller.law.as_ref(),
        loop_definition.parameter_binding.as_ref(),
    ) else {
        return Err(no_record());
    };
    let prefix = format!("{}.", law.parameter_record_symbol);
    let fields: Vec<String> = binding
        .parameters
        .iter()
        .filter_map(|parameter| parameter.symbol.strip_prefix(&prefix))
        .filter(|field| !field.contains('.'))
        .map(str::to_owned)
        .collect();
    if fields.is_empty() {
        return Err(no_record());
    }
    Ok(KeepableRecord {
        record: law.parameter_record_symbol.clone(),
        fields,
    })
}

fn no_record() -> Failure {
    Failure::new(
        &keep_texts::no_record(),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    )
}

/// The record of a program's own law (C2): the program instance that declares its gain variables, and the variables by name. Each
/// gain is `<instance>.<variable>`, and all of them belong to one instance.
///
/// # Errors
/// A gain is not a variable of a program instance, or the gains belong to two instances.
fn program_law_record(loop_definition: &LoopDefinition) -> Result<KeepableRecord, Failure> {
    let gains = gain_symbols(loop_definition).ok_or_else(no_record)?;
    let mut instance: Option<&str> = None;
    let mut fields = Vec::new();
    for symbol in [Some(gains.proportional), gains.integral, gains.derivative]
        .into_iter()
        .flatten()
    {
        let Some((root, field)) = program_variable(symbol) else {
            return Err(Failure::new(
                &keep_texts::gain_not_program_variable(symbol),
                Cause::ControllerCapability,
                NextAction::OpenLoopSetup,
            ));
        };
        if instance.is_some_and(|known| known != root) {
            return Err(Failure::new(
                &keep_texts::gains_of_two_instances(),
                Cause::ControllerCapability,
                NextAction::OpenLoopSetup,
            ));
        }
        instance = Some(root);
        fields.push(field.to_owned());
    }
    let record = instance.ok_or_else(no_record)?;
    Ok(KeepableRecord {
        record: record.to_owned(),
        fields,
    })
}

/// The instance and the variable of a program's own variable's symbol, `<instance>.<variable>`: two names joined by one `.`.
fn program_variable(symbol: &str) -> Option<(&str, &str)> {
    let (root, field) = symbol.split_once('.')?;
    let is_name = |name: &str| !name.is_empty() && !name.contains('.');
    (is_name(root) && is_name(field)).then_some((root, field))
}

/// The values Keep in project writes: each bound field whose running value is finite, by field name
/// (`keepAppliedParameters`, keepInProject.ts).
///
/// # Errors
/// The runtime reports no finite value of the record's fields.
pub fn kept_values(
    record: &KeepableRecord,
    running: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let values: BTreeMap<String, f64> = record
        .fields
        .iter()
        .filter_map(|field| {
            let value = *running.get(&format!("{}.{field}", record.record))?;
            value.is_finite().then(|| (field.clone(), value))
        })
        .collect();
    if values.is_empty() {
        return Err(Failure::new(
            &keep_texts::no_values(&record.record),
            Cause::Runtime,
            NextAction::ConnectRuntime,
        ));
    }
    Ok(values)
}

/// A language tool's reply to one of the commands that Keep in project asks (Specification 14 §7.8).
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    /// An edit of project files: the edits of each file, the text each file held when they were prepared, and the files it creates.
    Edit(ProjectEdit),
    /// `trust-lsp.controlKeepParameters` for a record that Create PLC code declared: the loop instance that is configured from it, and
    /// the equipment settings to keep, by member.
    Generated {
        /// The loop's instance, for example `Main.TIC101`.
        instance: String,
        /// The values to keep, by the equipment's member names.
        settings: BTreeMap<String, f64>,
    },
    /// `trust-lsp.hmiEquipment` describing an equipment instance.
    Described(KeepEquipment),
    /// A refusal, with the sentence to show.
    Refused(String),
}

/// A project edit as the language tool proposes it. The caller writes it only while each file still holds its `expected` text.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectEdit {
    /// The text edits of each changed file, by file URI.
    pub changes: BTreeMap<String, Vec<TextEdit>>,
    /// The text each changed file held when the edit was prepared, by file URI.
    pub expected: BTreeMap<String, String>,
    /// The files the edit creates, by file URI, with their text.
    pub creates: BTreeMap<String, String>,
    /// The fields the edit keeps, when the reply names them.
    pub kept: Option<Vec<String>>,
}

/// An equipment instance as the inspector describes it: whether its choices are the builder's, and its settings by member.
#[derive(Debug, Clone, PartialEq)]
pub struct KeepEquipment {
    /// Whether the choices are the HMI Builder's, not declared by hand in PLC code.
    pub owned: bool,
    /// The settings' current values, by member.
    pub settings: BTreeMap<String, Value>,
}

/// A text edit as the language tool proposes it (LSP `TextEdit`): a range and the text that replaces it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    /// The replaced range.
    pub range: Range,
    /// The text that replaces the range.
    pub new_text: String,
}

/// An LSP range: its start and end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Range {
    /// The first position.
    pub start: Position,
    /// The position after the last character.
    pub end: Position,
}

/// An LSP position: the zero-based line and the character as UTF-16 code units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Position {
    /// The zero-based line.
    pub line: u32,
    /// The UTF-16 code unit within the line.
    pub character: u32,
}

/// The language tool's answers, as the protocol gives them (Specification 14 §7.8).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wire {
    ok: bool,
    #[serde(default)]
    errors: Vec<WireMessage>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    generated: Option<WireGenerated>,
    #[serde(default)]
    owned: Option<bool>,
    #[serde(default)]
    settings: Vec<WireSetting>,
    #[serde(default)]
    edit: Option<WireChanges>,
    #[serde(default)]
    expected: BTreeMap<String, String>,
    #[serde(default)]
    creates: Vec<WireCreate>,
    #[serde(default)]
    kept: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct WireMessage {
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireGenerated {
    instance: String,
    settings: BTreeMap<String, f64>,
}

#[derive(Deserialize)]
struct WireSetting {
    member: String,
    value: Value,
}

#[derive(Deserialize)]
struct WireChanges {
    #[serde(default)]
    changes: BTreeMap<String, Vec<TextEdit>>,
}

#[derive(Deserialize)]
struct WireCreate {
    uri: String,
    text: String,
}

/// Reads a language tool's reply to `command`. A refusal without a sentence of its own gets `fallback`.
///
/// # Errors
/// The reply does not have the form the protocol gives.
pub fn decode_reply(answer: Value, command: &str, fallback: &str) -> Result<Reply, Failure> {
    let wire: Wire = json::decode_value(answer).map_err(|_| invalid_answer(command))?;
    if let Some(generated) = wire.generated {
        return Ok(Reply::Generated {
            instance: generated.instance,
            settings: generated.settings,
        });
    }
    if !wire.ok {
        let message = wire
            .errors
            .into_iter()
            .next()
            .map(|error| error.message)
            .or(wire.error);
        return Ok(Reply::Refused(
            message.unwrap_or_else(|| fallback.to_owned()),
        ));
    }
    // The inspector's description always names whether the instance is owned; an edit's reply does not.
    if let Some(owned) = wire.owned {
        let settings = wire
            .settings
            .into_iter()
            .map(|setting| (setting.member, setting.value))
            .collect();
        return Ok(Reply::Described(KeepEquipment { owned, settings }));
    }
    let changes = wire.edit.map_or_else(BTreeMap::new, |edit| edit.changes);
    let creates = wire
        .creates
        .into_iter()
        .map(|create| (create.uri, create.text))
        .collect();
    Ok(Reply::Edit(ProjectEdit {
        changes,
        expected: wire.expected,
        creates,
        kept: wire.kept,
    }))
}

fn invalid_answer(command: &str) -> Failure {
    Failure::new(
        &keep_texts::tool_answer_invalid(command),
        Cause::Implementation,
        NextAction::None,
    )
}

/// The settings an equipment keep writes (`keepEquipmentSettings`, equipmentEdits.ts): the values that differ from the
/// equipment's own, the members they keep, and the members the equipment has no setting for.
#[derive(Debug, Clone, PartialEq)]
pub struct EquipmentKeep {
    /// The values to write, by member.
    pub settings: BTreeMap<String, f64>,
    /// The members the write keeps, in order.
    pub kept: Vec<String>,
    /// The members the equipment has no setting for, in order.
    pub unknown: Vec<String>,
}

/// The equipment settings a keep writes: each value that the equipment has as a setting and that differs from it (`sameSetting`).
#[must_use]
pub fn equipment_keep(equipment: &KeepEquipment, values: &BTreeMap<String, f64>) -> EquipmentKeep {
    let mut settings = BTreeMap::new();
    let mut unknown = Vec::new();
    for (member, value) in values {
        match equipment.settings.get(member) {
            Some(own) if !same_setting(own, *value) => {
                settings.insert(member.clone(), *value);
            }
            Some(_) => {}
            None => unknown.push(member.clone()),
        }
    }
    let kept = settings.keys().cloned().collect();
    EquipmentKeep {
        settings,
        kept,
        unknown,
    }
}

/// The field a member names: the part after its last `.` (`{instance}_Parameters.ProportionalGain` is `ProportionalGain`).
#[must_use]
pub fn member_field(member: &str) -> &str {
    member.rsplit('.').next().unwrap_or(member)
}

/// Whether an equipment's setting and a value from the PLC are the same: numbers within a rounding difference, anything else not
/// (`sameSetting`, equipmentEdits.ts).
#[must_use]
pub fn same_setting(own: &Value, running: f64) -> bool {
    own.as_f64().is_some_and(|setting| {
        (setting - running).abs() <= 1e-9 * 1.0_f64.max(setting.abs()).max(running.abs())
    })
}

/// The text with each LSP text edit applied. Positions are a line and a UTF-16 character; the edits of one file must not overlap,
/// and a position outside the text is refused (`WorkspaceEdit` semantics, as `applyProjectEdit` applies them).
///
/// # Errors
/// A position lies outside the text, or two edits overlap.
pub fn apply_text_edits(text: &str, edits: &[TextEdit]) -> Result<String, Failure> {
    let mut spans = Vec::with_capacity(edits.len());
    for edit in edits {
        let start = offset(text, edit.range.start).ok_or_else(outside_edit)?;
        let end = offset(text, edit.range.end).ok_or_else(outside_edit)?;
        if start > end {
            return Err(outside_edit());
        }
        spans.push((start, end, edit.new_text.as_str()));
    }
    // A stable sort keeps the order of several insertions at one position.
    spans.sort_by_key(|&(start, end, _)| (start, end));
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;
    for (start, end, new_text) in spans {
        if start < cursor {
            return Err(Failure::new(
                &keep_texts::edits_overlap(),
                Cause::Implementation,
                NextAction::None,
            ));
        }
        result.push_str(&text[cursor..start]);
        result.push_str(new_text);
        cursor = end;
    }
    result.push_str(&text[cursor..]);
    Ok(result)
}

fn outside_edit() -> Failure {
    Failure::new(
        &keep_texts::edit_outside_file(),
        Cause::Implementation,
        NextAction::None,
    )
}

/// The byte offset of an LSP position in `text`, or `None` when the position lies outside it or inside a character.
fn offset(text: &str, position: Position) -> Option<usize> {
    let mut line_start = 0;
    for _ in 0..position.line {
        line_start += text[line_start..].find('\n')? + 1;
    }
    let rest = &text[line_start..];
    let line = rest.find('\n').map_or(rest, |end| &rest[..end]);
    let line = line.strip_suffix('\r').unwrap_or(line);
    let mut units: u32 = 0;
    for (index, character) in line.char_indices() {
        if units == position.character {
            return Some(line_start + index);
        }
        let width = u32::try_from(character.len_utf16()).ok()?;
        if units + width > position.character {
            return None;
        }
        units += width;
    }
    (units == position.character).then_some(line_start + line.len())
}

/// The path of a `file://` URI, percent-decoded. A Windows drive letter loses the slash before it.
///
/// # Errors
/// The URI is not a `file://` URI of this computer, or its escapes are not UTF-8.
pub fn file_uri_path(uri: &str) -> Result<PathBuf, Failure> {
    let rest = uri
        .strip_prefix("file://")
        .map(|rest| rest.strip_prefix("localhost").unwrap_or(rest))
        .filter(|rest| rest.starts_with('/'))
        .ok_or_else(|| invalid_uri(uri))?;
    let decoded = percent_decode(rest).ok_or_else(|| invalid_uri(uri))?;
    let path = if cfg!(windows) && has_drive_letter(&decoded) {
        &decoded[1..]
    } else {
        decoded.as_str()
    };
    Ok(PathBuf::from(path))
}

fn invalid_uri(uri: &str) -> Failure {
    Failure::new(
        &keep_texts::file_uri_invalid(uri),
        Cause::Implementation,
        NextAction::None,
    )
}

fn has_drive_letter(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':'
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let digits = text.get(index + 1..index + 3)?;
            if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return None;
            }
            decoded.push(u8::from_str_radix(digits, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

/// The review identity of a Keep (§0.1b): the SHA-256 digest, as `sha256:` and its hex, of the record, the values to keep and,
/// for each file the edit changes, its path and the text it holds now (`None` when it does not exist). The same inputs give the
/// same identity.
#[must_use]
pub fn review_identity(
    record: &str,
    values: &BTreeMap<String, f64>,
    files: &[(String, Option<String>)],
) -> String {
    let digest = Sha256::digest(json!([record, values, files]).to_string().as_bytes());
    let mut identity = String::from("sha256:");
    for byte in &digest {
        let _ = write!(identity, "{byte:02x}");
    }
    identity
}

#[cfg(test)]
#[path = "keep_tests.rs"]
mod tests;
