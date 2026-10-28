//! The sentences of the Manual-to-Auto entry case (specification 35, B5.3 and B9.9): the names of its rows and the refusal of a channel
//! that the loop declares no unit for. They sit beside `validation::texts`, which names the rows of the other cases.

/// The row of the first controller output after the Auto entry: the first-output continuity under the Auto-entry allowance (A6.4).
pub const FIRST_OUTPUT_AFTER_ENTRY: &str = "First output after Auto entry";
/// The row of the controller mode after the Auto entry (A6.2, B5.3).
pub const MODE_AFTER_ENTRY: &str = "Mode after Auto entry";
/// The row of the integral state at the Auto entry (C1).
pub const INTEGRAL_AT_ENTRY: &str = "Integral state at Auto entry";
/// The row of the derivative state at the Auto entry (C1).
pub const DERIVATIVE_AT_ENTRY: &str = "Derivative state at Auto entry";

/// The refusal of a channel of the entry run that no declared signal, parameter, mode, Manual command or state gives a unit for.
#[must_use]
pub fn unit_not_declared(symbol: &str) -> String {
    format!("The signal {symbol} has no declared unit.")
}
