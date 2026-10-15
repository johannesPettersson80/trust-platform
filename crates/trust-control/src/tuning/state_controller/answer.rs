//! The answer of a state-law Tune that the page reads (specification 35, C4 and B4.5; D-B12): the family of the design, its
//! closed-loop poles, its finite-horizon stages, and the fields of the design form that the family shows, with their labels.
//! The engine owns each of them; the page shows them and keeps no copy of the rules.

use schemars::JsonSchema;
use serde::Serialize;
use trust_control_math::linalg::ComplexValue;

use super::design::{StateControllerDesign, StateHorizon};
use super::preferences::{form_fields, FormField};
use super::StateControllerFamily;

/// What a state-law Tune answers: the family, the closed-loop poles of the candidate, the horizon stages when the design has a
/// finite horizon, and the design-form fields of the family.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StateAnswer {
    /// The family of the design.
    pub family: StateControllerFamily,
    /// The closed-loop poles of the candidate's mapped coefficients, on the sampled model (N7.11).
    pub closed_loop_poles: Vec<ComplexValue>,
    /// The finite-horizon stages (the gains and the Riccati matrices), when the design has a finite horizon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon: Option<StateHorizon>,
    /// The fields the design form shows for the family, with their labels (D-B12).
    pub fields: Vec<FormField>,
}

impl StateAnswer {
    /// The answer of `design`: its family, its poles and stages, and the design-form fields of its family.
    #[must_use]
    pub fn of(design: &StateControllerDesign) -> Self {
        Self {
            family: design.candidate.structure,
            closed_loop_poles: design.closed_loop_poles.clone(),
            horizon: design.horizon.clone(),
            // The design's own horizon exists exactly when the stored solver policy is the finite horizon (C4).
            fields: form_fields(design.candidate.structure, design.horizon.is_some()),
        }
    }
}
