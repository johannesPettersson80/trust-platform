//! The cascade's candidate and its parameter map (specification 35, C6 and C0): the two PIs that `design_cascade` chose, held as a
//! `ProjectCandidate` whose `values` hold the four gain variables of the two controllers, and the six gain parameters of the loop's
//! binding, which the candidate's values and the Validate runs write. The derivative variables are not in `values`: Apply writes them
//! as zero, as it does for every PI (C0, C2).

use crate::loop_definition::{LoopDefinition, ParameterBinding, RealType};
use crate::tuning::candidate::CandidateSource;
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::project_seed::CascadePidSeed;
use crate::tuning::proportional::GainBounds;
use crate::tuning::settings_pins::{GainKey, MappedParameter};
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::structures::cascade::CascadeDesign;

use std::collections::BTreeMap;

/// The cascade seed of a loop whose program law is one (C6). A loop with a controller law has none.
#[must_use]
pub fn served_cascade(loop_definition: &LoopDefinition) -> Option<&CascadePidSeed> {
    if loop_definition.controller.law.is_some() {
        return None;
    }
    loop_definition.controller.project.as_ref()?.cascade()
}

/// The candidate of a cascade (C6): both PIs, the inner's gains as `gains` and both controllers' proportional and integral variables in
/// `values`. The structure is PI for both controllers, whatever the gains are.
#[must_use]
pub fn cascade_candidate(
    seed: &CascadePidSeed,
    source: CandidateSource,
    design: CascadeDesign,
) -> ProjectCandidate {
    let mut values = BTreeMap::new();
    for (gains, designed) in [(&seed.inner, design.inner), (&seed.outer, design.outer)] {
        values.insert(
            gains.proportional_symbol.clone(),
            designed.proportional_gain,
        );
        values.insert(gains.integral_symbol.clone(), designed.integral_gain);
    }
    ProjectCandidate {
        applied: false,
        source,
        structure: Structure::PI,
        gains: design.inner,
        values,
    }
}

/// The outer controller's PI of a cascade candidate, read from its variables (C6). `None` when the candidate holds no value for one of
/// them. The outer derivative is zero, as the design writes it.
#[must_use]
pub fn cascade_outer_gains(
    seed: &CascadePidSeed,
    candidate: &ProjectCandidate,
) -> Option<ParallelGains> {
    Some(ParallelGains {
        proportional_gain: *candidate.values.get(&seed.outer.proportional_symbol)?,
        integral_gain: *candidate.values.get(&seed.outer.integral_symbol)?,
        derivative_gain: 0.0,
    })
}

/// The six gain parameters of a cascade, by record field (C0): each controller's proportional, integral and derivative gain, in the
/// binding's order of symbols. A symbol the binding does not hold maps nothing.
#[must_use]
pub fn cascade_mapped_parameters(
    seed: &CascadePidSeed,
    binding: &ParameterBinding,
) -> Vec<MappedParameter> {
    [&seed.inner, &seed.outer]
        .into_iter()
        .flat_map(|gains| {
            [
                (&gains.proportional_symbol, GainKey::ProportionalGain),
                (&gains.integral_symbol, GainKey::IntegralGain),
                (&gains.derivative_symbol, GainKey::DerivativeGain),
            ]
        })
        .filter_map(|(symbol, key)| mapped(binding, symbol, key))
        .collect()
}

/// The bound parameter with `symbol`, as the record field `key` holds it: its bounds and whether it is `REAL` (C0).
fn mapped(binding: &ParameterBinding, symbol: &str, key: GainKey) -> Option<MappedParameter> {
    let parameter = binding
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == symbol)?;
    Some(MappedParameter {
        symbol: symbol.to_owned(),
        key,
        bounds: GainBounds {
            minimum: parameter.minimum,
            maximum: parameter.maximum,
        },
        real: matches!(parameter.iec_type, RealType::Real),
    })
}

#[cfg(test)]
#[path = "cascade_candidate_tests.rs"]
mod tests;
