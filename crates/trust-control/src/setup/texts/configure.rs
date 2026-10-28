//! The sentences of the loop build (specification 35, B1.3, C0, C1, C12 and C13; A11.1): the refusals of
//! `configureLoop.ts`, `configureControllerAdapter.ts` and `configureLoopChannels.ts`, the refusals of the parts that are
//! parked for checklist V7, and the summary that Save shows. Each sentence names the value it is about.

use crate::loop_definition::LoopDefinition;
use crate::setup::prompts::SetupWorkflow;

/// The catalog has no compiled process-image signal, so no loop can be set up (`configureControlLoop`).
#[must_use]
pub fn no_process_signals() -> &'static str {
    "No compiled process-image signals are available. Symbolic-only setup is not available yet."
}

/// The loop's bindings fail the loop contract; `reason` is the contract's own sentence (`configureControlLoop`).
#[must_use]
pub fn incompatible_bindings(reason: &str) -> String {
    format!("The selected bindings are incompatible: {reason}.")
}

/// A question of the step-by-step setup has no answer yet, so the loop cannot be built.
#[must_use]
pub fn unanswered(title: &str) -> String {
    format!("The loop setup still needs an answer to {title}.")
}

/// The controller output is the process variable's signal (`configureLoop.ts`, the output list excludes it).
#[must_use]
pub fn output_repeats_process(symbol: &str) -> String {
    format!("{symbol} is the process variable, so it cannot also be the controller output.")
}

/// The setpoint is the process variable's or the output's signal (`configureLoop.ts`, the setpoint list excludes both).
#[must_use]
pub fn setpoint_repeats(symbol: &str) -> String {
    format!("{symbol} is already the process variable or the controller output, so it cannot be the setpoint.")
}

/// The chosen workflow needs a setpoint, which the loop does not declare (`selectWorkflow`).
#[must_use]
pub fn workflow_needs_setpoint(label: &str) -> String {
    format!("{label} needs a setpoint: choose a setpoint, or observe the program without one.")
}

/// The chosen symbol is not a compiled REAL or LREAL (`proportionalSetup`, the choice is filtered by type).
#[must_use]
pub fn not_compiled_numeric(symbol: &str) -> String {
    format!("{symbol} is not a compiled REAL or LREAL symbol.")
}

/// The chosen symbol is not a compiled BOOL (`proportionalSetup`, the mode choice is filtered by type).
#[must_use]
pub fn not_compiled_bool(symbol: &str) -> String {
    format!("{symbol} is not a compiled BOOL symbol.")
}

/// The multiple-channel setup is not available in the Rust engine yet (`configureLoopChannels.ts`, parked).
#[must_use]
pub fn multiple_channels_not_available() -> &'static str {
    "Multiple channels are not available in this setup yet. Use a single channel."
}

/// The ON/OFF controller setup is not available in the Rust engine yet (`configureOnOffController.ts`, parked).
#[must_use]
pub fn on_off_controller_not_available() -> &'static str {
    "An ON/OFF controller is not available in this setup yet. Choose the observation or the proportional workflow."
}

/// The ON/OFF actuator setup is not available in the Rust engine yet (`configureLoopChannels.ts`, parked).
#[must_use]
pub fn on_off_actuator_not_available() -> &'static str {
    "An ON/OFF actuator is not available in this setup yet. Use a continuous output."
}

/// The chosen feedback cannot report the applied action (`configureLoopChannels.ts`, the feedback list is filtered).
#[must_use]
pub fn not_applied_feedback(symbol: &str) -> String {
    format!("{symbol} cannot report the applied action: choose an input (%I) of type REAL, LREAL or BOOL that the loop does not use.")
}

/// The adapter mapping file is larger than its bound (`configureControllerAdapter.ts`, 256 kB).
#[must_use]
pub fn adapter_limit() -> &'static str {
    "Controller adapter mappings are limited to 256 kB."
}

/// The adapter mapping file is not JSON: `reason` is the parser's sentence (`configureControllerAdapter.ts`).
#[must_use]
pub fn adapter_not_json(reason: &str) -> String {
    format!("The controller adapter mapping is not valid JSON: {reason}.")
}

/// The adapter mapping lacks a `controller` or a `parameterBinding` object (`configureControllerAdapter.ts`).
#[must_use]
pub fn adapter_objects() -> &'static str {
    "Provide controller and parameterBinding objects in the adapter mapping."
}

/// The mapping's controller object does not fit the loop's controller: `reason` is the decoder's sentence.
#[must_use]
pub fn adapter_controller(reason: &str) -> String {
    format!("The controller adapter does not match the loop's controller: {reason}.")
}

/// The adapter does not match the selected task or the parameter contract (`configureControllerAdapter.ts`).
#[must_use]
pub fn adapter_task_mismatch() -> &'static str {
    "The controller adapter does not match the selected task/parameter contract."
}

/// A field of the adapter is not a compiled member of the expected type (`configureControllerAdapter.ts`).
#[must_use]
pub fn adapter_symbol(symbol: &str, type_word: &str) -> String {
    format!("Adapter field {symbol} must be a compiled scalar member of type {type_word}.")
}

/// The sentence of Save that summarises the loop (`configureControlLoop`, the modal confirmation). The engine shows it;
/// the sentence about what Save writes depends on the workflow.
#[must_use]
pub fn save_summary(loop_definition: &LoopDefinition, workflow: SetupWorkflow) -> String {
    let tail = match workflow {
        SetupWorkflow::Proportional => {
            "This declares the existing proportional controller; it does not write the runtime or edit ST."
        }
        SetupWorkflow::Adapter | SetupWorkflow::OnOff => {
            "This declares the existing controller and its typed adapter; it does not edit ST or write runtime values."
        }
        SetupWorkflow::Observation => "This loop observes the existing program.",
    };
    let process = &loop_definition.signals.process_variable;
    let output = &loop_definition.signals.manipulated_variable;
    format!(
        "Save {} for {}? PV: {} ({}); output: {} ({}); {} ms. {tail}",
        loop_definition.name,
        loop_definition.runtime_id,
        process.symbol,
        process.unit,
        output.symbol,
        output.unit,
        loop_definition.controller.task_period_ms,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_adapter_refusal_names_the_field_and_its_type() {
        assert_eq!(
            adapter_symbol("P1.Controller.StateValue.NominalOutput", "LREAL"),
            "Adapter field P1.Controller.StateValue.NominalOutput must be a compiled scalar member of type LREAL."
        );
    }

    #[test]
    fn the_incompatibility_names_the_contract_reason() {
        assert_eq!(
            incompatible_bindings("the setpoint and the measurement have different units"),
            "The selected bindings are incompatible: the setpoint and the measurement have different units."
        );
    }
}
