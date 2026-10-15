//! The sentences of Keep in project (specification 35, B6.5 and §0.1b; A11.1: each names the values involved). Each sentence
//! replaces one of `keepInProject.ts`, `equipmentEdits.ts` or `parameterSession.ts`; the rules that decide them are in `keep`.

use std::fmt::Write as _;

/// The loop binds no parameter record (`keepAppliedParameters`, keepInProject.ts; B6.5).
#[must_use]
pub fn no_record() -> String {
    "Keep in project writes a parameter record, such as the one Control.PID is configured from; this loop binds none.".to_owned()
}

/// A gain of a program's own law is not a variable of a program instance (`<instance>.<variable>`), so Keep in project cannot write
/// its initial value (C2, B6.5).
#[must_use]
pub fn gain_not_program_variable(symbol: &str) -> String {
    format!("{symbol} is not a variable of a program instance, so Keep in project cannot write its initial value. Copy the value into the program by hand.")
}

/// The gains of a program's own law belong to two program instances, and Keep in project writes the variables of one (C2, B6.5).
#[must_use]
pub fn gains_of_two_instances() -> String {
    "The gains of this program's law belong to different program instances, and Keep in project writes the variables of one. Copy the values into the program by hand.".to_owned()
}

/// The runtime reports none of the record's fields (`keepAppliedParameters`, keepInProject.ts).
#[must_use]
pub fn no_values(record: &str) -> String {
    format!("The runtime reports no values of {record}.")
}

/// A file the keep would change has unsaved changes in an editor: Keep writes the saved file, so the user saves or discards first
/// (B6.5).
#[must_use]
pub fn unsaved_changes(file: &str) -> String {
    format!("Save or discard your unsaved changes to {file} first: Keep in project writes the saved file.")
}

/// A file no longer holds the text the edit was prepared from; it is written only while it does (`applyProjectEdit`,
/// equipmentEdits.ts; B6.5).
#[must_use]
pub fn project_changed() -> String {
    "The project changed while preparing this edit. Your changes were preserved.".to_owned()
}

/// The confirmation's identity is not the one a new review gives: the values or a file changed since the review (§0.1b).
#[must_use]
pub fn review_expired() -> String {
    "This review expired. Review the exact values again.".to_owned()
}

/// The language tool refuses to keep the record without a sentence of its own (`keepAppliedParameters`, keepInProject.ts).
#[must_use]
pub fn not_kept(record: &str) -> String {
    format!("{record} could not be kept in the project.")
}

/// The equipment instance is declared in PLC code, so its values are copied there by hand (`keepEquipmentSettings`,
/// equipmentEdits.ts; Specification 39 §21.4 item 6).
#[must_use]
pub fn declared_in_plc(instance: &str) -> String {
    format!("{instance} is declared in your PLC code: copy the values there.")
}

/// The equipment's configuration cannot be described (`keepEquipmentSettings`, equipmentEdits.ts).
#[must_use]
pub fn equipment_unavailable() -> String {
    "Equipment configuration is unavailable.".to_owned()
}

/// The equipment refuses its project settings (`applyEquipmentChanges`, equipmentEdits.ts).
#[must_use]
pub fn equipment_rejected() -> String {
    "The equipment rejected these project settings.".to_owned()
}

/// A language tool's answer does not have the form the protocol gives (Specification 14 §7.8).
#[must_use]
pub fn tool_answer_invalid(command: &str) -> String {
    format!("The language tool's answer to {command} does not match the protocol.")
}

/// An edit names a file URI that is not a file path on this computer.
#[must_use]
pub fn file_uri_invalid(uri: &str) -> String {
    format!("{uri} is not a file path on this computer.")
}

/// A text edit lies outside the text it edits (LSP positions: line, and UTF-16 character).
#[must_use]
pub fn edit_outside_file() -> String {
    "An edit lies outside its file.".to_owned()
}

/// Two text edits of one file overlap.
#[must_use]
pub fn edits_overlap() -> String {
    "Two edits of one file overlap.".to_owned()
}

/// A file cannot be read.
#[must_use]
pub fn file_unreadable(path: &str, reason: &str) -> String {
    format!("{path} cannot be read: {reason}.")
}

/// A file cannot be written.
#[must_use]
pub fn file_unwritable(path: &str, reason: &str) -> String {
    format!("{path} cannot be written: {reason}.")
}

/// The report of a kept record: the fields kept or the fact that the project already holds them, and the equipment's fields it has
/// no setting for (`keptReason`, keepInProject.ts).
#[must_use]
pub fn kept_reason(record: &str, kept: &[String], unknown: &[String]) -> String {
    let mut skipped = String::new();
    if !unknown.is_empty() {
        let _ = write!(
            skipped,
            " The equipment has no setting for {}; copy it into the PLC code by hand.",
            unknown.join(", ")
        );
    }
    if kept.is_empty() {
        format!("The project already has these values of {record}.{skipped}")
    } else {
        format!(
            "Kept in the project: {} of {record}. A restart or a new download keeps them.{skipped}",
            kept.join(", ")
        )
    }
}
