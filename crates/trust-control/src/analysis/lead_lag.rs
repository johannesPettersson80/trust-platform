//! Lead and lag design of a selected model (the lead/lag tool of Control Studio). The design is the classical
//! compensator at one crossover (`trust_control_math::lead_lag`); this module reads the selected model as a
//! transfer function, keeps the result with the model it was designed for, and refuses a result that does not
//! belong to the selected model.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::classical::Transfer;
use trust_control_math::lead_lag::{design_lead_lag, LeadLagDesign, LeadLagMode as Mode};

use crate::analysis::texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::model::{linear_transfer_model, GeneralModel};

/// Which compensator is designed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LeadLagMode {
    /// A lead: phase added at the crossover, in degrees.
    Lead,
    /// A lag: low-frequency gain ratio.
    Lag,
}

/// The status of a designed compensator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LeadLagStatus {
    /// The design qualified.
    Qualified,
}

/// The inputs of a lead/lag design.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LeadLagRequest {
    /// Which compensator.
    pub mode: LeadLagMode,
    /// The crossover in rad/s; positive.
    pub crossover_rad_per_second: f64,
    /// The phase in degrees for a lead, or the low-frequency gain ratio for a lag.
    pub value: f64,
}

/// The designed compensator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LeadLagResult {
    /// The status of the design.
    pub status: LeadLagStatus,
    /// The gain of the compensator.
    pub gain: f64,
    /// The zero in rad/s.
    pub zero_rad_per_second: f64,
    /// The pole in rad/s.
    pub pole_rad_per_second: f64,
    /// The phase the compensator adds at the crossover, in degrees.
    pub phase_contribution_degrees: f64,
    /// The loop magnitude at the crossover.
    pub open_loop_magnitude_at_crossover: f64,
    /// The low-frequency gain ratio of a lag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_frequency_gain_ratio: Option<f64>,
}

/// A designed compensator with the model and the request it was made for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedLeadLagAnalysis {
    /// The model that was selected.
    pub model: GeneralModel,
    /// Which compensator.
    pub mode: LeadLagMode,
    /// The crossover in rad/s.
    pub crossover_rad_per_second: f64,
    /// The phase or the ratio.
    pub value: f64,
    /// The design.
    pub result: LeadLagResult,
}

/// Designs the compensator for the selected model.
///
/// # Errors
/// Returns the lead/lag input sentence for a crossover or value that is not finite and positive, the
/// linear-model refusal when the model is not a valid linear model, and the rejection sentence when the design
/// is refused.
pub fn run_lead_lag(
    model: &GeneralModel,
    request: LeadLagRequest,
) -> Result<SavedLeadLagAnalysis, Failure> {
    if !request.crossover_rad_per_second.is_finite()
        || request.crossover_rad_per_second <= 0.0
        || !request.value.is_finite()
    {
        return Err(Failure::new(
            texts::LEAD_LAG_INPUT,
            Cause::Settings,
            NextAction::None,
        ));
    }
    let transfer = transfer_of(model)?;
    let design = design_lead_lag(
        &transfer,
        math_mode(request.mode),
        request.crossover_rad_per_second,
        request.value,
    )
    .map_err(|error| {
        Failure::new(
            &texts::lead_lag_rejected(texts::classical_reason(error)),
            Cause::ModelOrData,
            NextAction::None,
        )
    })?;
    let saved = SavedLeadLagAnalysis {
        model: model.clone(),
        mode: request.mode,
        crossover_rad_per_second: request.crossover_rad_per_second,
        value: request.value,
        result: result_of(design),
    };
    check_saved_lead_lag(&saved)?;
    Ok(saved)
}

/// Checks a stored compensator: its crossover and value are finite and positive, and its figures are finite.
///
/// # Errors
/// Returns the saved-invalid sentence when a rule fails.
pub fn check_saved_lead_lag(saved: &SavedLeadLagAnalysis) -> Result<(), Failure> {
    let result = &saved.result;
    let finite = [
        result.gain,
        result.zero_rad_per_second,
        result.pole_rad_per_second,
        result.phase_contribution_degrees,
        result.open_loop_magnitude_at_crossover,
    ]
    .iter()
    .all(|value| value.is_finite())
        && result.low_frequency_gain_ratio.is_none_or(f64::is_finite);
    if finite && saved.crossover_rad_per_second > 0.0 && saved.value.is_finite() {
        Ok(())
    } else {
        Err(Failure::new(
            texts::LEAD_LAG_SAVED_INVALID,
            Cause::ModelOrData,
            NextAction::None,
        ))
    }
}

/// The lead/lag state of one selected model: the saved design, read-only.
#[derive(Debug, Default)]
pub struct LeadLagSession {
    saved: Option<SavedLeadLagAnalysis>,
}

impl LeadLagSession {
    /// A session with no design.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Designs for `model` and keeps the design.
    ///
    /// # Errors
    /// Returns the failure of [`run_lead_lag`]; the previous design is kept.
    pub fn run(&mut self, model: &GeneralModel, request: LeadLagRequest) -> Result<(), Failure> {
        self.saved = Some(run_lead_lag(model, request)?);
        Ok(())
    }

    /// The saved design, when it belongs to the selected model.
    #[must_use]
    pub fn snapshot(&self, selected: &GeneralModel) -> Option<SavedLeadLagAnalysis> {
        self.saved
            .as_ref()
            .filter(|saved| saved.model == *selected)
            .cloned()
    }

    /// Restores a stored design for the selected model, or clears the design when `value` is `None`.
    ///
    /// # Errors
    /// Returns the saved-invalid sentence for a design that fails its checks, and the other-model sentence for a
    /// design made for another model.
    pub fn restore(
        &mut self,
        value: Option<SavedLeadLagAnalysis>,
        selected: Option<&GeneralModel>,
    ) -> Result<(), Failure> {
        if let Some(saved) = &value {
            check_saved_lead_lag(saved)?;
            if selected.is_some_and(|model| model != &saved.model) {
                return Err(Failure::new(
                    texts::LEAD_LAG_OTHER_MODEL,
                    Cause::ModelOrData,
                    NextAction::None,
                ));
            }
        }
        self.saved = value;
        Ok(())
    }

    /// Clears the design.
    pub fn dispose(&mut self) {
        self.saved = None;
    }
}

/// The selected model as a transfer function, through the same linear-model conversion as the model page.
fn transfer_of(model: &GeneralModel) -> Result<Transfer, Failure> {
    let value = serde_json::to_value(model)
        .map_err(|_| Failure::new(texts::LEAD_LAG_INPUT, Cause::ModelOrData, NextAction::None))?;
    match linear_transfer_model(&value) {
        Ok(GeneralModel::TransferFunction { identity, transfer }) => Ok(Transfer {
            numerator: transfer.numerator,
            denominator: transfer.denominator,
            delay_seconds: identity.delay_seconds,
        }),
        Ok(_) => Err(Failure::new(
            texts::LEAD_LAG_INPUT,
            Cause::ModelOrData,
            NextAction::None,
        )),
        Err(refusal) => Err(Failure::new(
            &refusal.to_string(),
            Cause::ModelOrData,
            NextAction::None,
        )),
    }
}

fn math_mode(mode: LeadLagMode) -> Mode {
    match mode {
        LeadLagMode::Lead => Mode::Lead,
        LeadLagMode::Lag => Mode::Lag,
    }
}

fn result_of(design: LeadLagDesign) -> LeadLagResult {
    LeadLagResult {
        status: LeadLagStatus::Qualified,
        gain: design.gain,
        zero_rad_per_second: design.zero_rad_per_second,
        pole_rad_per_second: design.pole_rad_per_second,
        phase_contribution_degrees: design.phase_contribution_degrees,
        open_loop_magnitude_at_crossover: design.open_loop_magnitude_at_crossover,
        low_frequency_gain_ratio: design.low_frequency_gain_ratio,
    }
}

#[cfg(test)]
#[path = "lead_lag_tests.rs"]
mod tests;
