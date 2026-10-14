//! The declared state-feedback law of a loop: its `controller.stateFeedback` record, decoded with the rules that make it
//! usable (specification 35, C0 and C4; `decodeStateControllerBinding` of the TypeScript). The record is read through
//! `trust_control::json`, so its numbers are the same whatever `serde_json`'s features are, and each rule of the record is
//! checked here, after the structure is read.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::Value;
use trust_control_math::linalg::DenseMatrix;

use super::{StateControllerFamily, MAXIMUM_INPUTS, MAXIMUM_STATES};
use crate::commissioning::initial_state::ControllerInitialization;
use crate::json;
use crate::loop_definition::{InitializedState, ParameterBinding, StateType};

/// The estimator's fixed model of an LQG law: `A`, `B` and `C`, and the sample period the program runs it at (C4).
#[derive(Debug, Clone, PartialEq)]
pub struct EstimatorModel {
    /// The state matrix `A`.
    pub a: DenseMatrix,
    /// The input matrix `B`.
    pub b: DenseMatrix,
    /// The output matrix `C`.
    pub c: DenseMatrix,
    /// The sample period in seconds: finite and positive.
    pub sample_time_seconds: f64,
}

/// A decoded state-feedback law (C4). Its symbols name parameters of the loop's parameter record.
#[derive(Debug, Clone, PartialEq)]
pub struct StateControllerBinding {
    /// The law's family.
    pub family: StateControllerFamily,
    /// The parameter symbols of the gain `K`: one row per input, one column per state.
    pub gain_symbols: Vec<Vec<String>>,
    /// The parameter symbols of the reference gain `N`: one row per input, one column per controlled objective.
    pub reference_gain_symbols: Vec<Vec<String>>,
    /// The parameter symbols of the observer gain `L` (LQG only): one row per state.
    pub observer_gain_symbols: Option<Vec<Vec<String>>>,
    /// The desired discrete poles, when the designer places them.
    pub desired_poles: Option<Vec<f64>>,
    /// The state weight `Q`, when it is declared.
    pub state_weights: Option<DenseMatrix>,
    /// The input weight `R`, when it is declared.
    pub input_weights: Option<DenseMatrix>,
    /// The process noise covariance (LQG), when it is declared.
    pub process_noise: Option<DenseMatrix>,
    /// The measurement noise covariance (LQG), when it is declared.
    pub measurement_noise: Option<DenseMatrix>,
    /// The estimator's fixed model (LQG only).
    pub estimator_model: Option<EstimatorModel>,
    /// The declared initialization of the controller's states at an Auto entry, when there is one.
    pub initialization: Option<ControllerInitialization>,
    /// The estimate states that the initialization starts (LQG with an initialization), one per observer row.
    pub estimator_state_symbols: Option<Vec<String>>,
}

/// The record as it is written, before its rules are checked.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawBinding {
    family: StateControllerFamily,
    convention: Convention,
    gain_symbols: Vec<Vec<String>>,
    reference_gain_symbols: Vec<Vec<String>>,
    observer_gain_symbols: Option<Vec<Vec<String>>>,
    desired_poles: Option<Vec<f64>>,
    state_weights: Option<DenseMatrix>,
    input_weights: Option<DenseMatrix>,
    process_noise: Option<DenseMatrix>,
    measurement_noise: Option<DenseMatrix>,
    estimator_model: Option<RawEstimatorModel>,
    initialization: Option<RawInitialization>,
    estimator_state_symbols: Option<Vec<String>>,
}

#[derive(Deserialize)]
enum Convention {
    #[serde(rename = "sampled-deviation-negative-feedback")]
    SampledDeviationNegativeFeedback,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawEstimatorModel {
    a: DenseMatrix,
    b: DenseMatrix,
    c: DenseMatrix,
    sample_time_seconds: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawInitialization {
    kind: DeclaredStateKind,
    states: Vec<InitializedState>,
}

#[derive(Deserialize)]
enum DeclaredStateKind {
    #[serde(rename = "declared-state-v1")]
    DeclaredStateV1,
}

/// Decodes the stored law of a loop against its parameter record. `None` when the record does not decode, when a symbol is
/// not a parameter of the record, when the LQG estimator is not declared, or when a rule of the record fails.
#[must_use]
pub fn decode_state_controller_binding(
    value: &Value,
    parameter_binding: Option<&ParameterBinding>,
) -> Option<StateControllerBinding> {
    let parameters = &parameter_binding?.parameters;
    let raw: RawBinding = json::decode_value(value.clone()).ok()?;
    let Convention::SampledDeviationNegativeFeedback = raw.convention;
    let known = |symbol: &str| {
        parameters
            .iter()
            .any(|parameter| parameter.symbol == symbol)
    };

    if !symbol_rows_fit(&raw.gain_symbols, &known)
        || !symbol_rows_fit(&raw.reference_gain_symbols, &known)
        || !symbols_distinct(&raw)
    {
        return None;
    }
    let lqg = raw.family == StateControllerFamily::Lqg;
    match (&raw.observer_gain_symbols, lqg) {
        (Some(observer), true) if symbol_rows_fit(observer, &known) => {}
        (None, false) => {}
        _ => return None,
    }
    let initialization = raw
        .initialization
        .map(|raw_initialization| {
            let DeclaredStateKind::DeclaredStateV1 = raw_initialization.kind;
            ControllerInitialization::new(raw_initialization.states)
        })
        .transpose()
        .ok()?;
    if lqg {
        if let (Some(initialization), Some(observer)) =
            (&initialization, &raw.observer_gain_symbols)
        {
            let names = raw.estimator_state_symbols.as_deref()?;
            if names.len() != observer.len()
                || !names
                    .iter()
                    .all(|name| has_real_state(initialization, name))
            {
                return None;
            }
        }
    }
    let estimator_model = if lqg {
        Some(decode_estimator_model(raw.estimator_model.as_ref()?)?)
    } else {
        None
    };
    let matrices = [
        &raw.state_weights,
        &raw.input_weights,
        &raw.process_noise,
        &raw.measurement_noise,
    ];
    if matrices
        .into_iter()
        .flatten()
        .any(|matrix| !is_square_within_limit(matrix))
    {
        return None;
    }
    if raw.desired_poles.as_ref().is_some_and(Vec::is_empty) {
        return None;
    }
    Some(StateControllerBinding {
        family: raw.family,
        gain_symbols: raw.gain_symbols,
        reference_gain_symbols: raw.reference_gain_symbols,
        observer_gain_symbols: raw.observer_gain_symbols,
        desired_poles: raw.desired_poles,
        state_weights: raw.state_weights,
        input_weights: raw.input_weights,
        process_noise: raw.process_noise,
        measurement_noise: raw.measurement_noise,
        estimator_model,
        initialization,
        estimator_state_symbols: raw.estimator_state_symbols,
    })
}

/// One to 16 rows of one to 32 symbols each, every symbol a parameter of the record.
fn symbol_rows_fit(rows: &[Vec<String>], known: &impl Fn(&str) -> bool) -> bool {
    (1..=MAXIMUM_INPUTS).contains(&rows.len())
        && rows.iter().all(|row| {
            (1..=MAXIMUM_STATES).contains(&row.len())
                && row.iter().all(|symbol| known(symbol.as_str()))
        })
}

/// Whether every symbol of the gain, the reference gain and the observer gain is named once.
fn symbols_distinct(raw: &RawBinding) -> bool {
    let mut seen = HashSet::new();
    raw.gain_symbols
        .iter()
        .chain(&raw.reference_gain_symbols)
        .chain(raw.observer_gain_symbols.iter().flatten())
        .flatten()
        .all(|symbol| seen.insert(symbol.as_str()))
}

/// A square matrix of one to 32 rows.
fn is_square_within_limit(matrix: &DenseMatrix) -> bool {
    (1..=MAXIMUM_STATES).contains(&matrix.len())
        && matrix.iter().all(|row| row.len() == matrix.len())
}

/// The estimator's model: a finite positive sample period, and matrices of one to 32 rows of one to 32 entries.
fn decode_estimator_model(raw: &RawEstimatorModel) -> Option<EstimatorModel> {
    let period_fits = raw.sample_time_seconds.is_finite() && raw.sample_time_seconds > 0.0;
    let matrix_fits = |matrix: &DenseMatrix| {
        (1..=MAXIMUM_STATES).contains(&matrix.len())
            && matrix
                .iter()
                .all(|row| (1..=MAXIMUM_STATES).contains(&row.len()))
    };
    (period_fits && matrix_fits(&raw.a) && matrix_fits(&raw.b) && matrix_fits(&raw.c)).then(|| {
        EstimatorModel {
            a: raw.a.clone(),
            b: raw.b.clone(),
            c: raw.c.clone(),
            sample_time_seconds: raw.sample_time_seconds,
        }
    })
}

/// Whether the initialization declares `symbol` as a state that is not a `BOOL`.
fn has_real_state(initialization: &ControllerInitialization, symbol: &str) -> bool {
    initialization
        .states()
        .iter()
        .any(|state| state.symbol == symbol && state.iec_type != StateType::Bool)
}

#[cfg(test)]
#[path = "binding_tests.rs"]
mod tests;
