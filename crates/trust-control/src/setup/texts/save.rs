//! The sentences of the setup save (specification 35, B1.3, §0.1b and A11; `loopSetup/loopSetupSave.ts` and the id check of
//! `projectLoop.ts`). Each sentence names the values involved.

/// A planned loop's file already exists in the project: an existing definition is never overwritten (B1.3).
#[must_use]
pub fn already_exists(name: &str) -> String {
    format!("A loop named {name} already exists in this project.")
}

/// A planned loop's id is already declared by another loop file (B1.2, B1.3).
#[must_use]
pub fn duplicate_id(id: &str, file: &str) -> String {
    format!("Duplicate control loop ID {id} in {file}. Choose a different loop name.")
}

/// A loop file cannot be read as a loop definition, so a new loop is not added before it is repaired (B1.3). The file's JSON
/// is invalid, or the file could not be read; the save does not tell the two apart.
#[must_use]
pub fn repair_before_adding(file: &str) -> String {
    format!("Repair {file} before adding a control loop: its JSON is invalid or it cannot be read.")
}

/// A generated loop fails the checks that need no runtime (B1.3). `reason_code` is the rule's name, shown in words.
#[must_use]
pub fn incompatible(reason_code: &str) -> String {
    format!(
        "The selected bindings are incompatible: {}.",
        reason_code.replace('_', " ")
    )
}

/// The structure is one the Rust engine does not run yet (stage V7). `label` names it.
#[must_use]
pub fn parked_structure(label: &str) -> String {
    format!("{label} is not yet supported by the Rust engine.")
}

/// The name of a structure the Rust engine does not run yet (stage V7), or `None` for a structure it knows.
#[must_use]
pub fn parked_label(structure: &str) -> Option<&'static str> {
    match structure {
        "cascade" => Some("A cascade"),
        "group" => Some("A PID group"),
        "onoff" => Some("An ON/OFF controller"),
        "state-feedback" => Some("State feedback"),
        "search" => Some("A search law"),
        "smith" => Some("A Smith predictor"),
        "schedule" => Some("A gain schedule"),
        _ => None,
    }
}

/// The draft names no structure this setup knows (§0.1b).
#[must_use]
pub fn unknown_structure(structure: &str) -> String {
    format!("{structure} is not a loop setup structure this version knows.")
}

/// The draft's structure is not a text (§0.1b).
#[must_use]
pub fn structure_not_text() -> &'static str {
    "The setup draft's structure must be a text."
}

/// The draft is not a JSON object (§0.1b).
#[must_use]
pub fn draft_not_object() -> &'static str {
    "The setup draft must be a JSON object."
}

/// A trajectory feedforward is added to a PID law, because its seed names the three PID gains (C11): the program's own law must be PID.
#[must_use]
pub fn trajectory_law_not_pid() -> &'static str {
    "The trajectory feedforward is added to a PID law: the program's own law must be PID."
}

/// The draft of a structure is not complete, or has a field the structure does not declare (§0.1b). `error` names the field.
#[must_use]
pub fn draft_not_complete(structure: &str, error: &str) -> String {
    format!("The {structure} setup draft is not complete: {error}.")
}
