//! The constrained comparison of a SIMC request (numerical annex N5.10; specification 35, C1 "Constrained comparison"): the
//! fixed neighbourhood around the SIMC formula PI, each candidate predicted on the selected model with the same predictor
//! and checked against the mandatory requirements. The comparison creates no candidate, the formula PI stays the method's
//! candidate, and nothing is ranked: a ranking needs an explicitly chosen objective (N8.6), which Validate owns.

use crate::messages::ComparisonEntry;
use crate::study::tune::prediction::candidate_outcome;
use crate::tuning::formula::tune_identified;
use crate::tuning::objective::TuningMethod;
use crate::tuning::plant::TuningPlant;
use crate::tuning::prediction::SelectedModelPredictor;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::{predicted_outcome, DesignContext};
use crate::tuning::typed_pid::{constrained_comparison, ComparisonLabel, TypedDesign};

/// The label of the formula's own PI, which keeps the method's name (N5.10).
const FORMULA_LABEL: &str = "SIMC";
/// The label of every other candidate of the comparison: never SIMC or AMIGO (N5.10).
const CONSTRAINED_LABEL: &str = "Constrained comparison";

/// The entries of the constrained comparison around the SIMC PI of the requirements' response (N5.10): the formula PI, the
/// less aggressive PI and P, and for a first-order model PD and PID, each with its gains and its outcome on the predictor's
/// loop. It is empty when SIMC gives the model no PI, which the checks before the start already refuse.
pub(crate) fn simc_comparison(
    plant: &TuningPlant,
    requirements: &TuningRequirements,
    context: &DesignContext<'_>,
    predictor: &SelectedModelPredictor<'_>,
    gain_unit: &str,
) -> Vec<ComparisonEntry> {
    let Ok(formula) = tune_identified(
        predictor.model,
        TuningMethod::Simc,
        requirements.formula_objective(),
        context.action,
        gain_unit,
    ) else {
        return Vec::new();
    };
    constrained_comparison(plant, &formula)
        .into_iter()
        .map(|candidate| {
            let design = TypedDesign {
                structure: candidate.structure,
                gains: candidate.gains,
            };
            let outcome = predicted_outcome(&design, requirements, context);
            let outcome = candidate_outcome(outcome, predictor, candidate.gains, requirements);
            ComparisonEntry {
                label: label_of(candidate.label).to_string(),
                structure: candidate.structure,
                gains: candidate.gains,
                outcome_text: outcome.sentence(),
                outcome,
            }
        })
        .collect()
}

/// The label a candidate of the comparison carries (N5.10).
fn label_of(label: ComparisonLabel) -> &'static str {
    match label {
        ComparisonLabel::FormulaPi => FORMULA_LABEL,
        ComparisonLabel::LessAggressivePi
        | ComparisonLabel::P
        | ComparisonLabel::Pd
        | ComparisonLabel::Pid => CONSTRAINED_LABEL,
    }
}
