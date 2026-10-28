//! The sentences of the loop discovery, the binding selection and the three-page binding route (specification 35, B1.2, B1.3 and
//! A11; `hostBindingDiscovery.ts`, `hostBindingSession.ts`, `threePageBindingRoute.ts`). Each sentence names the values involved.

/// The project declares more loop files than the discovery reads (B1.2).
#[must_use]
pub fn too_many_loop_files(maximum: usize) -> String {
    format!("The project declares more than {maximum} loop definitions.")
}

/// The `control-loops` directory cannot be listed, so nothing is discovered (B1.2).
#[must_use]
pub fn control_loops_unreadable() -> &'static str {
    "The control-loops directory could not be read."
}

/// A loop file is larger than the discovery reads (B1.2).
#[must_use]
pub fn exceeds_byte_bound(file: &str, maximum: u64) -> String {
    format!("{file} exceeds the {maximum} byte loop-definition bound.")
}

/// A loop file is not JSON, or could not be read (B1.2). The save and the open path use the same sentence.
#[must_use]
pub fn not_valid_json(file: &str) -> String {
    format!("{file} could not be read as valid JSON.")
}

/// A loop file is JSON that is not a complete loop definition of this format (B1.2, B1.4).
#[must_use]
pub fn not_loop_definition(file: &str) -> String {
    format!("{file} does not contain a complete Control Studio loop definition.")
}

/// A loop file belongs to another runtime than the one selected (B1.2).
#[must_use]
pub fn runtime_mismatch(loop_runtime: &str, selected: &str) -> String {
    format!("The configured loop uses {loop_runtime}, but {selected} is the selected target.")
}

/// A loop file fails the checks that need no runtime (B1.4). `reason_code` is the rule's name, shown in words.
#[must_use]
pub fn invalid_loop(reason_code: &str) -> String {
    format!(
        "Loop definition is invalid: {}.",
        reason_code.replace('_', " ")
    )
}

/// Two loop files declare one id, so no target is selectable (B1.2).
#[must_use]
pub fn duplicate_loop_id(id: &str) -> String {
    format!("Duplicate control loop ID {id}; the target is ambiguous.")
}

/// The loop's controller is not in the compiled program (B1.2, the compiled catalog).
#[must_use]
pub fn controller_unavailable(instance: &str) -> String {
    format!("Compiled controller {instance} is unavailable.")
}

/// The compiled controller has another type than the loop declares.
#[must_use]
pub fn controller_type(instance: &str, compiled: &str, expected: &str) -> String {
    format!("Compiled controller {instance} has type {compiled}, expected {expected}.")
}

/// The compiled controller is not bound to the loop's task period (B1.4, the task comes from the compiled program).
#[must_use]
pub fn controller_task(instance: &str, period_ms: f64) -> String {
    format!("Compiled controller {instance} is not bound to the declared {period_ms} ms task.")
}

/// A member of a PID group is not in the compiled program in the declared task (C7).
#[must_use]
pub fn group_member_unavailable(instance: &str) -> String {
    format!("Compiled PID group member {instance} is unavailable in the declared task.")
}

/// A signal without an address is not a compiled variable of the declared type (§17.1).
#[must_use]
pub fn signal_not_variable(symbol: &str, iec_type: &str) -> String {
    format!("Declared signal {symbol} is not a compiled {iec_type} variable.")
}

/// A signal with an address does not match the compiled process image (B1.2).
#[must_use]
pub fn signal_identity(symbol: &str) -> String {
    format!("Declared signal {symbol} does not match the compiled process-image identity.")
}

/// A controller symbol of the loop is not in the compiled catalog (C0).
#[must_use]
pub fn controller_symbol_unavailable(symbol: &str) -> String {
    format!("Declared controller symbol {symbol} is unavailable in the compiled catalog.")
}

/// A controller symbol has another type than the loop declares (C0).
#[must_use]
pub fn controller_symbol_type(symbol: &str, compiled: &str, expected: &str) -> String {
    format!("Compiled controller symbol {symbol} has type {compiled}, expected {expected}.")
}

/// The loop has no input or no output to select (B1.4).
#[must_use]
pub fn needs_input_and_output() -> &'static str {
    "The loop must declare at least one input and one output channel."
}

/// The loop uses channels, which the Rust engine does not run yet (stage V7, Specification 35 C0).
#[must_use]
pub fn channels_not_supported() -> &'static str {
    "This loop uses channels, which are not yet supported by the Rust engine."
}

/// A selection names a loop that the scope does not list (B1.2, the session's list).
#[must_use]
pub fn no_loop_with_id(id: &str) -> String {
    format!("No discovered loop has ID {id}.")
}

/// A selection names another controller than the loop's compiled target (B1.3).
#[must_use]
pub fn controller_not_declared(selected: &str, declared: &str) -> String {
    format!("Controller {selected} is not the declared compiled target {declared}.")
}

/// A selection names no channel of one side (B1.3).
#[must_use]
pub fn choose_at_least_one(label: &str) -> String {
    format!("Choose at least one {label} channel.")
}

/// A selection leaves out a declared channel of one side: a partial mapping never replaces the loop (B1.3).
#[must_use]
pub fn select_every(label: &str) -> String {
    format!("Select every declared {label} channel before opening this loop.")
}

/// A selection names a channel identity that the loop does not declare (B1.3).
#[must_use]
pub fn channel_not_declared(label: &str) -> String {
    format!("The selected {label} channel identity is not declared by the loop.")
}

/// A binding selection is not shaped as the selector sends it (B1.3, `threePageBindingRoute.ts`).
#[must_use]
pub fn selection_shape() -> &'static str {
    "Choose a declared controller and its complete signal mapping."
}

/// The chosen controller family is not one the selected loop's adapter supports (C0).
#[must_use]
pub fn family_not_supported() -> &'static str {
    "Choose a configuration supported by that controller's declared adapter."
}

/// No single configured project holds the loop of the selector (B1.2, multi-root workspaces).
#[must_use]
pub fn project_not_identified() -> &'static str {
    "Open Control Studio from the configured project to identify its controller bindings."
}
