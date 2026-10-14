//! The sentences of the open path's missing and invalid loop reasons (specification 35, B1.2; `controlStudio/projectLoop.ts`). The
//! invalid-JSON, not-a-definition and runtime sentences are the discovery's (`texts::discovery`), with the same wording.

/// The project declares no loop definition, so there is nothing to bind (B1.2, "Configure control loop" is offered).
#[must_use]
pub fn missing_loop_definition() -> &'static str {
    "This project has no loop definition in control-loop.json or control-loops/. Configure a loop to bind its signals."
}

/// A loop file fails a rule that needs the project's signals. `fields` names the signals and `reason_code` the rule.
#[must_use]
pub fn validation_failed(file: &str, fields: &str, reason_code: &str) -> String {
    format!("{file}: {fields}: {}.", reason_code.replace('_', " "))
}

/// The setpoint and the measurement of a loop file have different units (B1.4).
#[must_use]
pub fn units_mismatch(file: &str, fields: &str) -> String {
    format!("{file}: {fields} must use matching engineering units.")
}

/// Two loop files of one project declare one id (B1.2).
#[must_use]
pub fn duplicate_in_project(id: &str) -> String {
    format!("Duplicate control loop ID {id} in this project. Give each loop a unique ID.")
}
