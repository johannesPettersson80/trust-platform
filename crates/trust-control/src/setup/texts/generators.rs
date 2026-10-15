//! The sentences of the compiled catalog and the loop generators (A11.1): each names the values involved and keeps the
//! TypeScript sentence it replaces (`hostBindingDiscovery.ts`, `compiledCatalog.ts`, `loopBindings.ts`, `pidLoopGenerator.ts`,
//! `ownLawLoopGenerator.ts`, `identifyLoopGenerator.ts`), and the startup-done sentence of `loopSetupForm.ts`.

use crate::limits::{MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS};

/// The compiled catalog is unavailable (`normalizeCatalog`).
#[must_use]
pub fn catalog_unavailable() -> &'static str {
    "Compiled control catalog is unavailable."
}

/// The project did not compile, so its control symbols are missing (`openCommand.ts`).
#[must_use]
pub fn catalog_not_compiled() -> &'static str {
    "Compile the project successfully before configuring its loop."
}

/// The process-image lists exceed the bounded entry limit (`normalizeCatalog`).
#[must_use]
pub fn process_image_limit() -> &'static str {
    "Compiled process-image catalog exceeds its bounded entry limit."
}

/// The control-symbol lists exceed the bounded entry limit (`normalizeCatalog`).
#[must_use]
pub fn control_symbol_limit() -> &'static str {
    "Compiled control-symbol catalog exceeds its bounded entry limit."
}

/// One process-image signal has two identities (`normalizeCatalog`).
#[must_use]
pub fn signal_identity_conflict(symbol: &str) -> String {
    format!("Compiled signal {symbol} has conflicting identities.")
}

/// One control symbol has two types (`normalizeCatalog`).
#[must_use]
pub fn symbol_type_conflict(symbol: &str) -> String {
    format!("Compiled symbol {symbol} has conflicting types.")
}

/// One controller instance has two types (`normalizeCatalog`).
#[must_use]
pub fn controller_type_conflict(instance: &str) -> String {
    format!("Compiled controller {instance} has conflicting types.")
}

/// The symbol is not compiled in this project (`compiled`, `loopBindings.ts`).
#[must_use]
pub fn not_compiled_symbol(symbol: &str) -> String {
    format!("{symbol} is not a compiled symbol of this project.")
}

/// The variable is not REAL or LREAL (`realType`, `loopBindings.ts`).
#[must_use]
pub fn must_be_real(symbol: &str) -> String {
    format!("{symbol} must be REAL or LREAL.")
}

/// The program computes the measurement, so a simulated plant would overwrite it (`measurementBinding`, B5 and §13.2).
#[must_use]
pub fn computed_measurement(symbol: &str) -> String {
    format!(
        "{symbol} is computed by the program, so a simulated measurement would be overwritten every scan. Choose the measurement it is computed from."
    )
}

/// The identification duration is missing or outside 1 s to 24 h (`identificationDuration`, A7).
#[must_use]
pub fn identification_duration(name: &str) -> String {
    let minimum = MINIMUM_EXPERIMENT_DURATION_MS / 1_000;
    let maximum = MAXIMUM_EXPERIMENT_DURATION_MS / 3_600_000;
    format!("{name} needs an identification duration between {minimum} s and {maximum} h.")
}

/// Only one of the output step and the identification duration is set (`explicitExperiment`, B1.3).
#[must_use]
pub fn output_step_and_duration(name: &str) -> String {
    format!(
        "{name} needs both the output step and the identification duration, or neither: Identify chooses the one that is left."
    )
}

/// The controller does not run in exactly one cyclic task (`pidLoop`, `ownLawLoop`, `identifyLoop`).
#[must_use]
pub fn not_one_cyclic_task(instance: &str) -> String {
    format!("{instance} must run in exactly one cyclic task.")
}

/// The instance is not a compiled `Control.PID` (`pidLoopGenerator.ts`).
#[must_use]
pub fn not_compiled_pid(instance: &str) -> String {
    format!("{instance} is not a compiled Control.PID instance.")
}

/// The instance is not a compiled program or function block (`ownLawLoopGenerator.ts`).
#[must_use]
pub fn not_compiled_program(instance: &str) -> String {
    format!("{instance} is not a compiled program or function block instance.")
}

/// The manipulated variable does not belong to a compiled program (`identifyLoopGenerator.ts`).
#[must_use]
pub fn not_compiled_owner(symbol: &str) -> String {
    format!("{symbol} does not belong to a compiled program instance.")
}

/// A law that integrates needs its integral gain variable (`ownLawLoopGenerator.ts`).
#[must_use]
pub fn integral_gain_needed(law: &str) -> String {
    format!("A {law} law needs the variable holding its integral gain.")
}

/// A law that integrates needs its integral term variable (`ownLawLoopGenerator.ts`).
#[must_use]
pub fn integral_term_needed(law: &str) -> String {
    format!("A {law} law needs the variable holding its integral term.")
}

/// A PID law needs its derivative gain variable (`ownLawLoopGenerator.ts`).
#[must_use]
pub fn derivative_gain_needed() -> &'static str {
    "A PID law needs the variable holding its derivative gain."
}

/// A PID law needs the variable its derivative is taken from (`ownLawLoopGenerator.ts`).
#[must_use]
pub fn derivative_state_needed() -> &'static str {
    "A PID law needs the variable holding the previous error or measurement its derivative is taken from."
}

/// The gains and the integral term are not distinct variables (`ownLawLoopGenerator.ts`).
#[must_use]
pub fn distinct_variables() -> &'static str {
    "Choose a different variable for each gain and for the integral term."
}

/// A gain's range is not a finite interval (`ownLawLoopGenerator.ts`).
#[must_use]
pub fn gain_range(symbol: &str) -> String {
    format!("The range of {symbol} needs a minimum below its maximum.")
}

/// The program writes the symbol in its own code (C1 Setup; `loopSetupForm.ts`, `seededWrite`).
#[must_use]
pub fn program_writes(symbol: &str) -> String {
    format!("The program writes {symbol} in its own code.")
}

/// The program initializes the PID in its own code (C1 Setup; `loopSetupForm.ts`, `seededWrite`).
#[must_use]
pub fn program_initializes(instance: &str) -> String {
    format!("The program initializes {instance} in its own code.")
}

/// Setup needs the startup-done flag for the reason given (C1 Setup; `loopSetupForm.ts`, `startupFlag`).
#[must_use]
pub fn startup_flag_needed(reason: &str) -> String {
    format!(
        "{reason} Choose the flag that marks its startup as done, so validation keeps the values it starts from."
    )
}

/// A feedforward's lead time, lag time and filtered disturbance are chosen together, or none of them (C8, `feedforwardGenerator.ts`).
#[must_use]
pub fn feedforward_gain_group() -> &'static str {
    "Choose the lead time, the lag time and the filtered disturbance together, or none of them."
}

/// A feedforward's variables differ from each other and from the loop's signals (C8, `feedforwardGenerator.ts`).
#[must_use]
pub fn feedforward_variables_distinct() -> &'static str {
    "The feedforward's variables must differ from each other and from the loop's signals."
}

/// The disturbance step is positive and at most the disturbance's span (C8, `feedforwardGenerator.ts`).
#[must_use]
pub fn feedforward_step_span(span: f64, unit: &str) -> String {
    format!("The disturbance step must be positive and at most {span} {unit}.")
}

/// The variable Identify steps to move the measured disturbance is not an explicitly actuated simulation or test input: Setup refuses it
/// before any write (C8, D-5). Such an input is a REAL or LREAL variable that the program only reads, at a memory address `%M`.
#[must_use]
pub fn feedforward_command_not_actuated(symbol: &str) -> String {
    format!(
        "{symbol} cannot move the measured disturbance: a disturbance command must be a simulation or test input, a REAL or LREAL variable the program only reads, at a memory address (%M)."
    )
}
