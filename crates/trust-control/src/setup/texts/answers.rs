//! The sentences of the step-by-step answers (specification 35, B1.3; `step_answers.rs`). Each sentence names the question or the
//! identity it refuses.

/// The draft's answers are not a JSON object.
#[must_use]
pub fn not_object() -> &'static str {
    "The step-by-step answers must be a JSON object of the setup's questions."
}

/// A key is not a question of the step-by-step setup.
#[must_use]
pub fn unknown_question(key: &str) -> String {
    format!("The step-by-step setup asks no question named {key}.")
}

/// An answer is not text, as its question needs.
#[must_use]
pub fn not_text(key: &str) -> String {
    format!("The answer to {key} must be text.")
}

/// An answer is not a number, as its question needs.
#[must_use]
pub fn not_number(key: &str) -> String {
    format!("The answer to {key} must be a number.")
}

/// An answer is not true or false, as its question needs.
#[must_use]
pub fn not_boolean(key: &str) -> String {
    format!("The answer to {key} must be true or false.")
}

/// An answer names none of the choices its question offers.
#[must_use]
pub fn not_offered(key: &str, value: &str) -> String {
    format!("{value} is not an answer that {key} offers.")
}

/// A controller instance the compiled project does not hold.
#[must_use]
pub fn unknown_controller(instance: &str) -> String {
    format!(
        "The compiled project has no controller {instance}. Choose a controller of the project."
    )
}

/// A signal address the compiled project does not hold.
#[must_use]
pub fn unknown_signal(address: &str) -> String {
    format!("The compiled project has no signal at {address}. Choose a signal of the project.")
}

/// A symbol the compiled project does not hold.
#[must_use]
pub fn unknown_symbol(symbol: &str) -> String {
    format!("The compiled project has no symbol {symbol}. Choose a symbol of the project.")
}
