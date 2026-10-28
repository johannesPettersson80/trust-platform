//! The controller offer of a state-feedback law (specification 35, C4): the families the law is offered as on the Tune page, the
//! parameter symbols that each family takes, and the family the page marks recommended. The rule is the state adapter's: an LQG
//! law is offered as LQG alone, because its estimator fixes the structure; a law of one input is offered as state feedback and as
//! LQR, which share the law without an estimator; a law of several inputs is offered as LQR, because state feedback takes one
//! input. The engine answers the offer, and the page keeps no copy of the rule.

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::tuning::state_controller::{
    decode_state_controller_binding, texts, StateControllerFamily,
};

/// The offer of a state-feedback law (C4): its families by name, the parameter symbols that each family takes, and the family
/// the page marks recommended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateOffer {
    /// The families the law is offered as, by name, in the order the choice lists them.
    pub families: Vec<String>,
    /// The parameter symbols that each family's settings and pins take: the gains, the reference gains and, for LQG, the
    /// observer gains.
    pub family_settings: BTreeMap<String, Vec<String>>,
    /// The family the page marks recommended: the declared family when the offer takes it, otherwise the first offered family.
    pub recommended: String,
}

/// The name of a family as the page names it in the Controller choice: `state-feedback`, `LQR` or `LQG`.
#[must_use]
pub fn family_name(family: StateControllerFamily) -> &'static str {
    match family {
        StateControllerFamily::StateFeedback => "state-feedback",
        StateControllerFamily::Lqr => "LQR",
        StateControllerFamily::Lqg => "LQG",
    }
}

/// The families that a law declared as `family`, with `inputs` plant inputs, is offered as (C4): an LQG law as LQG alone; a law
/// of one input as state feedback and LQR; a law of several inputs as LQR alone.
#[must_use]
pub fn offered_families(
    family: StateControllerFamily,
    inputs: usize,
) -> Vec<StateControllerFamily> {
    match (family, inputs) {
        (StateControllerFamily::Lqg, _) => vec![StateControllerFamily::Lqg],
        (_, 1) => vec![
            StateControllerFamily::StateFeedback,
            StateControllerFamily::Lqr,
        ],
        _ => vec![StateControllerFamily::Lqr],
    }
}

/// The offer of the loop's state-feedback law (C4).
///
/// # Errors
/// Returns the failure of a law that does not decode against the loop's parameter record.
pub fn offer(loop_definition: &LoopDefinition) -> Result<StateOffer, Failure> {
    let binding = loop_definition
        .controller
        .state_feedback
        .as_ref()
        .and_then(|value| {
            decode_state_controller_binding(value, loop_definition.parameter_binding.as_ref())
        })
        .ok_or_else(|| {
            Failure::new(
                texts::NEEDS_BINDING,
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    let families = offered_families(binding.family, binding.gain_symbols.len());
    let symbols: Vec<String> = binding
        .gain_symbols
        .iter()
        .chain(&binding.reference_gain_symbols)
        .chain(binding.observer_gain_symbols.iter().flatten())
        .flatten()
        .cloned()
        .collect();
    let recommended = if families.contains(&binding.family) {
        binding.family
    } else {
        families.first().copied().unwrap_or(binding.family)
    };
    Ok(StateOffer {
        families: families
            .iter()
            .map(|family| family_name(*family).to_owned())
            .collect(),
        family_settings: families
            .iter()
            .map(|family| (family_name(*family).to_owned(), symbols.clone()))
            .collect(),
        recommended: family_name(recommended).to_owned(),
    })
}
