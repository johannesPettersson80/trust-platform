//! The sentences of a program's own law (C1, C2 and C13; A11.1: each names the values involved), re-exported by `commissioning::texts`.

/// Adapters (`projectController.ts`, `materialize` of the project law; C2): the program's own law needs its proportional gain.
#[must_use]
pub fn project_proportional_gain_required() -> String {
    "The program's own law needs a proportional gain.".to_owned()
}

/// Adapters (`projectController.ts`, `materialize` of the project law; C2): the seed names no variable for the term, so a nonzero
/// gain of that term has nowhere to be written.
#[must_use]
pub fn project_term_missing(term: &str) -> String {
    format!(
        "The program's own law names no {term} variable, so it cannot write a nonzero {term} gain."
    )
}

/// Adapters (C13, C1): a law state of a program's own P law, which a P law cannot start. The connected P entry writes the application
/// states and the reference only; an integral or a derivative term needs a PI, PD or PID law.
#[must_use]
pub fn project_p_law_state(symbol: &str) -> String {
    format!("The program's own P law declares {symbol}, a law state that the connected P entry cannot start. Only application states are written with the P entry.")
}

/// Adapters (C13, C1): an application state of a program's own P law takes a formula, which the Auto entry cannot compute without the
/// model. The startup-done flag takes a constant.
#[must_use]
pub fn project_p_application_formula(symbol: &str) -> String {
    format!("The application state {symbol} of the program's own P law must take a constant initial value.")
}

/// C11: a trajectory-following law writes its rate and acceleration feedforward gains with its PID gains, and the candidate holds neither.
#[must_use]
pub fn project_feedforward_required() -> String {
    "The trajectory feedforward needs its rate and acceleration gains, and the candidate holds them not. Tune the law again.".to_owned()
}

/// C8: a measured-disturbance feedforward writes its gain with its PI gains, and its lead and lag times when the program has both; the
/// candidate holds one of them not.
#[must_use]
pub fn project_feedforward_gain_required() -> String {
    "The feedforward needs its gain, and its lead and lag times when the program has them, and the candidate holds one of them not. Tune the law again.".to_owned()
}
