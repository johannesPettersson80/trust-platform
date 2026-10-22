//! The structure a Tune designs for (specification 35, B4.2 and B4.4). The page names a structure only when the engineer chose one in
//! the Controller selector; every other case is the engine's rule, so the page keeps no copy of it and sends no word of its own.

use crate::messages::TuneMethod;
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::structure::Structure;

/// The structure a Tune designs for (B4.2, B4.4): the structure the request names, else the one the study's inputs record (A8.1).
///
/// The manual method is the one method that cannot design a structure of its own, because its entered settings belong to a family. With
/// no structure named or recorded it takes the family the Controller selector shows (`default_family`): the family of the study's
/// candidate, else the family of its recorded pins. Every other method, and Automatic, leaves the structure to its own design, so a
/// candidate of an earlier Tune never fixes the structure of the next one.
#[must_use]
pub fn structure_of_request(
    named: Option<Structure>,
    method: Option<TuneMethod>,
    recorded: Option<Structure>,
    candidate: Option<Structure>,
    recorded_pins: Option<&ControllerSettingsPins>,
) -> Option<Structure> {
    let chosen = named.or(recorded);
    if method == Some(TuneMethod::Manual) {
        chosen
            .or(candidate)
            .or(recorded_pins.map(|pins| pins.family))
    } else {
        chosen
    }
}

#[cfg(test)]
#[path = "structure_choice_tests.rs"]
mod tests;
