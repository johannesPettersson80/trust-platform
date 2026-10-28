//! The tuning methods and the settings each controller family takes, as the setup page offers them (specification 35, B4.2, C0, C2, C3, C4,
//! C6 to C10, N5.R): the named methods of a loop, the methods a study offers the Tune page with their titles, and the settings of each
//! family. `discovery` joins them into a loop's controller compatibility and re-exports the public items.

use std::collections::BTreeMap;

use super::discovery::{adapter_compatibility, family_name, ControllerCompatibility};
use crate::loop_definition::LoopDefinition;
use crate::tuning::cascade_candidate::served_cascade;
use crate::tuning::project_candidate::{served_feedforward, served_smith};
use crate::tuning::relay_rule::RelayCycleEvidence;
use crate::tuning::settings_pins::family_setting_symbols;
use crate::tuning::structure::Structure;

/// The parameter symbols that each offered family's manual settings and pins take, by the family's name (C0, C2): the same rule
/// that Tune applies to the settings and pins it accepts (`family_setting_symbols`).
pub(super) fn family_settings_of(
    loop_definition: &LoopDefinition,
    families: &[Structure],
) -> BTreeMap<String, Vec<String>> {
    families
        .iter()
        .map(|structure| {
            (
                family_name(*structure).to_string(),
                family_setting_symbols(loop_definition, *structure),
            )
        })
        .collect()
}

/// The named methods of a `Control.PID`, proportional or program's own law (B4.2, C4): SIMC, AMIGO and Manual, in the order the page lists
/// them. The relay rule is not among them, because it needs a qualified relay record of the study (N5.R), which setup does not read. A
/// state law and a finite search take their own lists, which `adapter_compatibility` gives before it reaches this one.
fn named_methods() -> Vec<crate::messages::TuneMethod> {
    use crate::messages::TuneMethod;
    vec![TuneMethod::Simc, TuneMethod::Amigo, TuneMethod::Manual]
}

/// The named methods that a loop takes: `named_methods` where Tune takes them, and none where Tune designs the loop by Automatic alone.
pub(super) fn named_methods_of(
    loop_definition: &LoopDefinition,
) -> Vec<crate::messages::TuneMethod> {
    if automatic_only(loop_definition) {
        Vec::new()
    } else {
        named_methods()
    }
}

/// Whether Tune designs the loop by Automatic alone (C3, C6, C7, C8, C9, C10): a feedforward, a Smith predictor, a cascade, a gain
/// schedule, a switching controller and a PID group. Each refuses every named method before its design, Manual included, as
/// `study/tune/design/feedforward.rs`, `smith.rs`, `cascade.rs`, `gain_schedule.rs` and `pid_group.rs`, and `study/tune/on_off.rs` give.
/// The seeds are read as `design::run` reads them, so the offer and the design take the same loops.
fn automatic_only(loop_definition: &LoopDefinition) -> bool {
    let controller = &loop_definition.controller;
    controller.on_off.is_some()
        || controller.group.is_some()
        || controller
            .project
            .as_ref()
            .is_some_and(|project| project.schedule.is_some())
        || served_feedforward(loop_definition).is_some()
        || served_smith(loop_definition).is_some()
        || served_cascade(loop_definition).is_some()
}

/// The tuning methods a study offers the Tune page, in the page's order, and their titles keyed by method (see
/// [`study_tune_methods`]).
pub type TuneMethodOffer = (
    Vec<crate::messages::TuneMethod>,
    Option<BTreeMap<String, String>>,
);

/// The tuning methods that a study offers the Tune page, in the page's order, with their titles (C4, N5.R): the loop's named methods,
/// and the relay rule beside Manual when the study holds a qualified Feedback relay record and the loop declares a `Control.PID` law,
/// the only law the page offered the relay rule for. `None` when the loop's controller is not accepted. Setup cannot read the study, so
/// the open answer carries this offer, and `setupDiscover` keeps the loop's own list.
#[must_use]
pub fn study_tune_methods(
    loop_definition: &LoopDefinition,
    relay: Option<&RelayCycleEvidence>,
) -> Option<TuneMethodOffer> {
    use crate::messages::TuneMethod;
    let ControllerCompatibility::Accepted { methods, .. } = adapter_compatibility(loop_definition)
    else {
        return None;
    };
    let mut methods = methods.unwrap_or_default();
    let qualified = matches!(relay, Some(RelayCycleEvidence::Qualified { .. }));
    let pid_law = matches!(
        loop_definition.controller.law,
        Some(crate::loop_definition::ControllerLaw::Pid(_))
    );
    if qualified && pid_law && !methods.is_empty() {
        let manual = methods
            .iter()
            .position(|method| *method == TuneMethod::Manual)
            .unwrap_or(methods.len());
        methods.insert(manual, TuneMethod::Relay);
    }
    let titles = method_titles_of(&methods);
    Some((methods, titles))
}

/// The title of each method, keyed by the method's word, as the engine gives it (`TuneMethod::title`, C4); absent for no method.
pub(super) fn method_titles_of(
    methods: &[crate::messages::TuneMethod],
) -> Option<BTreeMap<String, String>> {
    (!methods.is_empty()).then(|| {
        methods
            .iter()
            .map(|method| (method_word(*method).to_owned(), method.title().to_owned()))
            .collect()
    })
}

/// The word of a tuning method as the answers name it: the name that the method has in its JSON (`TuneMethod`).
fn method_word(method: crate::messages::TuneMethod) -> &'static str {
    use crate::messages::TuneMethod;
    match method {
        TuneMethod::Simc => "SIMC",
        TuneMethod::Amigo => "AMIGO",
        TuneMethod::Relay => "relay",
        TuneMethod::Manual => "manual",
        TuneMethod::StateSpace => "state-space",
    }
}

#[cfg(test)]
#[path = "tune_method_offers_tests.rs"]
mod tests;
