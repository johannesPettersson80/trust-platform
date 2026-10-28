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
pub fn parked_label(_structure: &str) -> Option<&'static str> {
    None
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

// The state-feedback (C4), LQG (C4) and finite-search (C5) drafts: the sentences of `loopSetupForm.ts` for the choices the form
// checks, and of `stateFeedbackLoopGenerator.ts`, `lqgLoopGenerator.ts`, `estimatorModelFiles.ts` and `searchLawLoopGenerator.ts`
// for the rest. Each names the values involved.

/// The loop has no name (`loopSetupForm.ts`).
#[must_use]
pub fn name_needed() -> &'static str {
    "Enter a loop name."
}

/// The instance that runs a state-feedback law is not chosen (`loopSetupForm.ts`).
#[must_use]
pub fn state_feedback_instance_needed() -> &'static str {
    "Choose the instance that runs the state-feedback law."
}

/// The instance that computes a searched law is not chosen (`loopSetupForm.ts`).
#[must_use]
pub fn search_instance_needed() -> &'static str {
    "Choose the instance that computes the law."
}

/// A signal of the setup has no symbol (`loopSetupForm.ts`, `signal`). `label` names the signal's role.
#[must_use]
pub fn signal_symbol_needed(label: &str) -> String {
    format!("Choose the {label}.")
}

/// A signal of the setup has no unit (`loopSetupForm.ts`, `signal`).
#[must_use]
pub fn signal_unit_needed(label: &str) -> String {
    format!("Enter the {label} unit.")
}

/// A signal of the setup has no finite minimum and maximum (`loopSetupForm.ts`, `signal`).
#[must_use]
pub fn signal_range_needed(label: &str) -> String {
    format!("Enter the {label} minimum and maximum.")
}

/// A signal's maximum is not above its minimum (`loopSetupForm.ts`, `signal`).
#[must_use]
pub fn signal_range_order(label: &str) -> String {
    format!("The {label} maximum must be above its minimum.")
}

/// The setpoint is not chosen (`loopSetupForm.ts`).
#[must_use]
pub fn setpoint_needed() -> &'static str {
    "Choose the setpoint."
}

/// The Auto/Manual signal is not chosen (`loopSetupForm.ts`).
#[must_use]
pub fn mode_needed() -> &'static str {
    "Choose the Auto/Manual signal."
}

/// The manual output is not chosen (`loopSetupForm.ts`).
#[must_use]
pub fn manual_output_needed() -> &'static str {
    "Choose the manual output."
}

/// The variable that holds the gain on a measured output is not chosen (`loopSetupForm.ts`). `symbol` names the output.
#[must_use]
pub fn gain_needed(symbol: &str) -> String {
    format!("Choose the variable holding the gain on {symbol}.")
}

/// The variable that holds the reference gain is not chosen (`loopSetupForm.ts`).
#[must_use]
pub fn reference_gain_needed() -> &'static str {
    "Choose the variable holding the reference gain."
}

/// No further measured output is chosen for a state-feedback law (`loopSetupForm.ts`).
#[must_use]
pub fn further_output_needed() -> &'static str {
    "Choose at least one further measured output: state feedback needs every state it feeds back."
}

/// The law names more further measured outputs than the setup holds (`stateFeedbackLoopGenerator.ts`; D-B4 in C4).
#[must_use]
pub fn further_outputs_limit() -> &'static str {
    "State feedback needs one to three measured outputs besides the controlled output."
}

/// The LQG estimator's model file is not chosen (`loopSetupForm.ts`).
#[must_use]
pub fn estimator_model_needed() -> &'static str {
    "Choose the model file the program's estimator implements."
}

/// The model file is not inside the project (`loopSetupSave.ts`, `estimatorModel`).
#[must_use]
pub fn model_outside_project() -> &'static str {
    "Choose a model file inside the project."
}

/// The model file does not decode as a model (`loopSetupSave.ts`, `estimatorModel`).
#[must_use]
pub fn model_not_valid(file: &str) -> String {
    format!("{file} is not a valid model.")
}

/// The model file cannot be read. The `TypeScript` read failed with the file system's own message.
#[must_use]
pub fn model_unreadable(file: &str) -> String {
    format!("{file} cannot be read.")
}

/// The estimator's model is not a state-space model with one input and the controlled output as its one output
/// (`lqgLoopGenerator.ts`).
#[must_use]
pub fn estimator_model_shape() -> &'static str {
    "The estimator's model must be a state-space model with one input and the controlled output as its one output."
}

/// The estimator has no estimate, gain and observer gain for each state of its model (`lqgLoopGenerator.ts`).
#[must_use]
pub fn estimator_rows() -> &'static str {
    "Give one estimate, gain and observer gain for each of the model's (at most three) states."
}

/// One state of the estimator is missing its estimate, gain or observer gain (`loopSetupForm.ts`). `part` names the missing item.
#[must_use]
pub fn estimator_state_needed(part: &str, index: usize) -> String {
    format!("Choose the variable holding the {part} of model state {index}.")
}

/// One state of the estimator has no positive process-noise variance (`loopSetupForm.ts`).
#[must_use]
pub fn process_noise_needed(index: usize) -> String {
    format!("Enter the positive process-noise variance of model state {index}.")
}

/// The measurement-noise variance is missing or not positive (`loopSetupForm.ts`).
#[must_use]
pub fn measurement_noise_needed() -> &'static str {
    "Enter the positive measurement-noise variance."
}

/// The law the program computes is not one the search tunes (`searchLawLoopGenerator.ts`).
#[must_use]
pub fn search_family_needed() -> &'static str {
    "Choose which kind of law the program computes."
}

/// No coefficient is chosen for the search (`loopSetupForm.ts` and `searchLawLoopGenerator.ts`).
#[must_use]
pub fn search_coefficient_needed() -> &'static str {
    "Choose at least one coefficient Tune may search."
}

/// The search names more coefficients than it tries (`searchLawLoopGenerator.ts`).
#[must_use]
pub fn search_coefficient_limit() -> &'static str {
    "Tune searches at most four coefficients."
}

/// The search keeps more values from scan to scan than it declares (`searchLawLoopGenerator.ts`).
#[must_use]
pub fn search_state_limit() -> &'static str {
    "Declare at most three values the program keeps from scan to scan."
}

/// Two searched variables, or a searched and a kept variable, are the same (`searchLawLoopGenerator.ts`).
#[must_use]
pub fn search_variables_distinct() -> &'static str {
    "Choose a different variable for each coefficient and each value the program keeps."
}

/// A searched coefficient or a kept value has no unit (`loopSetupForm.ts`).
#[must_use]
pub fn search_unit_needed(symbol: &str) -> String {
    format!("Enter the unit of {symbol}.")
}

/// The values Tune may try for a coefficient cannot be read (`loopSetupForm.ts`).
#[must_use]
pub fn search_values_unreadable(symbol: &str) -> String {
    format!("Enter the values of {symbol} Tune may try, separated by commas.")
}

/// A coefficient's values are empty or not finite (`searchLawLoopGenerator.ts`).
#[must_use]
pub fn search_values_needed(symbol: &str) -> String {
    format!("Enter the values of {symbol} Tune may try.")
}

/// The search's combinations exceed the limit (`loopSetupForm.ts` and `searchLawLoopGenerator.ts`).
#[must_use]
pub fn search_combinations(count: usize, limit: usize) -> String {
    format!("Tune tries at most {limit} combinations; these values make {count}.")
}
