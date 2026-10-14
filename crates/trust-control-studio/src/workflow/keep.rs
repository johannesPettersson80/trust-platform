//! Keep in project's workflow (specification 35, B6.5 and §0.1b `keep`): the engine asks the language tool for the edit that keeps a
//! loop's parameter record, checks each file the edit changes, and writes the files once the review is confirmed. What is kept is
//! decided in `trust_control::commissioning::keep`; this module runs the tool, reads and writes the files and sequences the checks.
//! The engine's method (`controlStudio/keep`) and its Apply-section precondition are the caller's.

#![allow(clippy::module_name_repetitions)] // `KeepReview` and `KeepOutcome` name the Keep action that this module runs.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use trust_control::commissioning::{keep as rules, keep_texts};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;

use crate::language_tool::ProjectTool;

/// The command that answers a Keep of a record (specification 14 §7.8).
const CONTROL_KEEP: &str = "trust-lsp.controlKeepParameters";
/// The command that describes an equipment instance and proposes an edit of its choices (specification 39 §21.3).
const HMI_EQUIPMENT: &str = "trust-lsp.hmiEquipment";
/// The project file that holds the equipment design (specification 39 §10.4).
const PLANT_FILE: &str = "plant.toml";

/// A file that a Keep changes: its path, the text it holds now (`None` when the Keep creates it) and the text the Keep gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// The file's path.
    pub path: PathBuf,
    /// The text the file holds now, or `None` when it does not exist yet.
    pub before: Option<String>,
    /// The text the file holds once the Keep is written.
    pub after: String,
}

/// What a Keep would write, shown before it is confirmed (§0.1b): the record, the values to keep, the fields it writes, the files it
/// changes, and the review identity that a confirmation must give back.
#[derive(Debug, Clone, PartialEq)]
pub struct KeepReview {
    /// The record, for example `Main.TIC101_Parameters`.
    pub record: String,
    /// The values to keep, by field.
    pub values: BTreeMap<String, f64>,
    /// The fields the Keep writes.
    pub kept: Vec<String>,
    /// The fields that the equipment has no setting for (a record Create PLC code declared).
    pub unknown: Vec<String>,
    /// The files the Keep changes.
    pub files: Vec<FileChange>,
    /// The identity of this review (`review_identity`).
    pub identity: String,
}

/// What a confirmed Keep wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeepOutcome {
    /// The record.
    pub record: String,
    /// The fields the Keep wrote.
    pub kept: Vec<String>,
    /// The fields that the equipment has no setting for.
    pub unknown: Vec<String>,
}

/// Reviews a Keep in project of the loop's parameter record: the values the running PLC holds, the edit the language tool proposes
/// for them, and the identity of that review. It writes nothing.
///
/// `unsaved_files` are the files with unsaved changes in an editor, project-relative or absolute; a Keep that would change one of them
/// is refused (B6.5).
///
/// # Errors
/// The loop binds no record, the runtime reports none of its values, the tool refuses or answers out of protocol, a file the Keep
/// changes has unsaved changes, or a file no longer holds the text the edit was prepared from.
pub fn review_keep(
    project: &Path,
    loop_definition: &LoopDefinition,
    running_values: &BTreeMap<String, f64>,
    unsaved_files: &[PathBuf],
    tool: &impl ProjectTool,
) -> Result<KeepReview, Failure> {
    let record = rules::keepable_record(loop_definition)?;
    let values = rules::kept_values(&record, running_values)?;
    let plan = plan_keep(project, &record.record, &values, tool)?;
    for file in &plan.files {
        refuse_unsaved(project, file, unsaved_files)?;
    }
    let files: Vec<(String, Option<String>)> = plan
        .files
        .iter()
        .map(|file| (file.path.display().to_string(), file.before.clone()))
        .collect();
    let identity = rules::review_identity(&record.record, &values, &files);
    Ok(KeepReview {
        record: record.record,
        values,
        kept: plan.kept,
        unknown: plan.unknown,
        files: plan.files,
        identity,
    })
}

/// Confirms a reviewed Keep: the review is made again, and it is written only when its identity is the one given. Each file is
/// written only while it still holds the text the review read.
///
/// # Errors
/// As `review_keep`; the review's identity is not `identity` (the values or a file changed since it); or a file changed or cannot be
/// written.
pub fn confirm_keep(
    project: &Path,
    loop_definition: &LoopDefinition,
    running_values: &BTreeMap<String, f64>,
    unsaved_files: &[PathBuf],
    tool: &impl ProjectTool,
    identity: &str,
) -> Result<KeepOutcome, Failure> {
    let review = review_keep(
        project,
        loop_definition,
        running_values,
        unsaved_files,
        tool,
    )?;
    if review.identity != identity {
        return Err(refusal(&keep_texts::review_expired()));
    }
    for file in &review.files {
        if read_text(&file.path)? != file.before {
            return Err(refusal(&keep_texts::project_changed()));
        }
    }
    for file in &review.files {
        write_change(file)?;
    }
    Ok(KeepOutcome {
        record: review.record,
        kept: review.kept,
        unknown: review.unknown,
    })
}

/// The edit of a Keep before it is reviewed.
struct Plan {
    kept: Vec<String>,
    unknown: Vec<String>,
    files: Vec<FileChange>,
}

/// Asks the tool for the edit of the record: a record written by hand is edited in its declaration; a record Create PLC code declared
/// is kept through its equipment's settings (B6.5).
fn plan_keep(
    project: &Path,
    record: &str,
    values: &BTreeMap<String, f64>,
    tool: &impl ProjectTool,
) -> Result<Plan, Failure> {
    let argument = json!({ "record": record, "values": values });
    let answer = tool.answer(CONTROL_KEEP, project, &argument)?;
    match rules::decode_reply(answer, CONTROL_KEEP, &keep_texts::not_kept(record))? {
        rules::Reply::Edit(edit) => {
            let files = edit_files(CONTROL_KEEP, &edit)?;
            Ok(Plan {
                kept: edit
                    .kept
                    .unwrap_or_else(|| values.keys().cloned().collect()),
                unknown: Vec::new(),
                files,
            })
        }
        rules::Reply::Generated { instance, settings } => {
            plan_equipment(project, &instance, &settings, tool)
        }
        rules::Reply::Refused(text) => Err(refusal(&text)),
        rules::Reply::Described(_) => Err(malformed(CONTROL_KEEP)),
    }
}

/// The Keep of a record Create PLC code declared: the equipment's own settings decide what differs, and the HMI Builder's edit
/// writes it with the regions the generator owns (`keepEquipmentSettings`, equipmentEdits.ts; Specification 39 §21.4 item 6).
fn plan_equipment(
    project: &Path,
    instance: &str,
    settings: &BTreeMap<String, f64>,
    tool: &impl ProjectTool,
) -> Result<Plan, Failure> {
    let manifest = read_text(&project.join(PLANT_FILE))?;
    let describe = json!({ "instance": instance });
    let answer = tool.answer(
        HMI_EQUIPMENT,
        project,
        &with_manifest(describe, manifest.as_deref()),
    )?;
    let equipment =
        match rules::decode_reply(answer, HMI_EQUIPMENT, &keep_texts::equipment_unavailable())? {
            rules::Reply::Described(equipment) => equipment,
            rules::Reply::Refused(text) => return Err(refusal(&text)),
            _ => return Err(malformed(HMI_EQUIPMENT)),
        };
    if !equipment.owned {
        return Err(refusal(&keep_texts::declared_in_plc(instance)));
    }
    let keep = rules::equipment_keep(&equipment, settings);
    let files = if keep.settings.is_empty() {
        Vec::new()
    } else {
        let change = json!({ "instance": instance, "changes": { "settings": keep.settings } });
        let answer = tool.answer(
            HMI_EQUIPMENT,
            project,
            &with_manifest(change, manifest.as_deref()),
        )?;
        match rules::decode_reply(answer, HMI_EQUIPMENT, &keep_texts::equipment_rejected())? {
            rules::Reply::Edit(edit) => edit_files(HMI_EQUIPMENT, &edit)?,
            rules::Reply::Refused(text) => return Err(refusal(&text)),
            _ => return Err(malformed(HMI_EQUIPMENT)),
        }
    };
    Ok(Plan {
        kept: keep
            .kept
            .iter()
            .map(|member| rules::member_field(member).to_owned())
            .collect(),
        unknown: keep
            .unknown
            .iter()
            .map(|member| rules::member_field(member).to_owned())
            .collect(),
        files,
    })
}

/// The argument with `plant.toml` as the project has it, when it has one: the tool then reads the design the engine read.
fn with_manifest(mut argument: Value, manifest: Option<&str>) -> Value {
    if let Some(text) = manifest {
        argument["manifest"] = json!(text);
    }
    argument
}

/// The files an edit changes and creates. A changed file must still hold the text the edit was prepared from (B6.5).
fn edit_files(command: &str, edit: &rules::ProjectEdit) -> Result<Vec<FileChange>, Failure> {
    let mut files = Vec::with_capacity(edit.changes.len() + edit.creates.len());
    for (uri, edits) in &edit.changes {
        let path = rules::file_uri_path(uri)?;
        let expected = edit.expected.get(uri).ok_or_else(|| malformed(command))?;
        if read_text(&path)?.as_deref() != Some(expected.as_str()) {
            return Err(refusal(&keep_texts::project_changed()));
        }
        let after = rules::apply_text_edits(expected, edits)?;
        files.push(FileChange {
            path,
            before: Some(expected.clone()),
            after,
        });
    }
    for (uri, text) in &edit.creates {
        let path = rules::file_uri_path(uri)?;
        if read_text(&path)?.is_some() {
            return Err(refusal(&keep_texts::project_changed()));
        }
        files.push(FileChange {
            path,
            before: None,
            after: text.clone(),
        });
    }
    Ok(files)
}

/// Refuses a Keep that would change a file with unsaved changes in an editor: Keep writes the saved file (B6.5).
fn refuse_unsaved(
    project: &Path,
    file: &FileChange,
    unsaved_files: &[PathBuf],
) -> Result<(), Failure> {
    let changed = comparable(&file.path);
    for unsaved in unsaved_files {
        // `join` keeps an absolute path as it is.
        if comparable(&project.join(unsaved)) == changed {
            let shown = file.path.strip_prefix(project).unwrap_or(&file.path);
            return Err(refusal(&keep_texts::unsaved_changes(
                &shown.display().to_string(),
            )));
        }
    }
    Ok(())
}

/// A path spelled as the file system has it, so that one file has one spelling.
fn comparable(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// A file's text, or `None` when it does not exist.
fn read_text(path: &Path) -> Result<Option<String>, Failure> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(refusal(&keep_texts::file_unreadable(
            &path.display().to_string(),
            &error.to_string(),
        ))),
    }
}

/// Writes one changed file: a created file must not exist yet.
fn write_change(file: &FileChange) -> Result<(), Failure> {
    let written = if file.before.is_some() {
        fs::write(&file.path, &file.after)
    } else {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file.path)
            .and_then(|mut created| created.write_all(file.after.as_bytes()))
    };
    written.map_err(|error| {
        refusal(&keep_texts::file_unwritable(
            &file.path.display().to_string(),
            &error.to_string(),
        ))
    })
}

/// A refusal of the workflow, or a sentence the language tool gave.
fn refusal(text: &str) -> Failure {
    Failure::new(text, Cause::Unclassified, NextAction::None)
}

/// A reply of the language tool that does not have the form it should have here.
fn malformed(command: &str) -> Failure {
    Failure::new(
        &keep_texts::tool_answer_invalid(command),
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "keep_tests.rs"]
mod tests;
