use super::{
    contracts::{Channel, Event},
    execution_settings::ExecutionSettings,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ParameterOverride {
    pub symbol: String,
    pub iec_type: String,
    pub unit: String,
    pub convention: String,
    pub minimum: f64,
    pub maximum: f64,
    pub value: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SimulationOverride {
    pub model_revision: String,
    pub model_provenance: String,
    pub simulation_toml: String,
}
impl SimulationOverride {
    fn validate(&self) -> Result<(), String> {
        if self.model_revision.is_empty()
            || self.model_revision.len() > 256
            || self.model_provenance.is_empty()
            || self.model_provenance.len() > 4_096
            || self.simulation_toml.is_empty()
            || self.simulation_toml.len() > 262_144
        {
            return Err(
                "invalid selected model metadata or simulation configuration bounds".into(),
            );
        }
        let config = crate::simulation::SimulationConfig::from_toml_text(
            &self.simulation_toml,
            "selected model simulation",
        )
        .map_err(|error| error.to_string())?;
        if !config.enabled || config.plant_system.plants.is_empty() {
            return Err("selected model must materialize an enabled simulated plant".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunRequest {
    pub expected_project_revision: String,
    #[serde(default)]
    pub request_id: Option<String>,
    pub execution: String,
    pub step_ms: u64,
    pub duration_ms: u64,
    pub seed: u64,
    pub max_total_cycles: u64,
    pub wall_budget_ms: u64,
    pub initial_values: BTreeMap<String, f64>,
    pub events: Vec<Event>,
    pub observations: Vec<Channel>,
    #[serde(default)]
    pub parameter_overrides: Vec<ParameterOverride>,
    #[serde(default)]
    pub simulation_override: Option<SimulationOverride>,
    #[serde(default)]
    pub feedback_relay: Option<super::feedback_relay::FeedbackRelay>,
}
impl RunRequest {
    pub fn settings(&self) -> ExecutionSettings {
        ExecutionSettings {
            execution: self.execution.clone(),
            step_ms: self.step_ms,
            duration_ms: self.duration_ms,
            seed: self.seed,
            max_total_cycles: self.max_total_cycles,
            wall_budget_ms: self.wall_budget_ms,
            per_trial_wall_ms: self.wall_budget_ms,
            initial_values: self.initial_values.clone(),
            events: self.events.clone(),
            observations: self.observations.clone(),
        }
    }
    pub fn validate(&self) -> Result<BTreeMap<String, f64>, String> {
        if self.request_id.as_ref().is_some_and(|id| {
            id.is_empty()
                || id.len() > 128
                || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        }) {
            return Err("invalid isolated operation identity".into());
        }
        if let Some(simulation_override) = &self.simulation_override {
            simulation_override.validate()?;
        }
        if self.parameter_overrides.len() > 64 {
            return Err("at most64 scalar parameter overrides are supported".into());
        }
        let mut symbols = HashSet::new();
        let mut parameters = BTreeMap::new();
        for p in &self.parameter_overrides {
            if p.symbol.is_empty()
                || p.symbol.len() > 256
                || !symbols.insert(p.symbol.to_ascii_uppercase())
                || !matches!(p.iec_type.as_str(), "REAL" | "LREAL" | "BOOL")
                || p.unit.is_empty()
                || p.convention.is_empty()
                || !p.minimum.is_finite()
                || !p.maximum.is_finite()
                || p.minimum > p.maximum
                || !p.value.is_finite()
                || p.value < p.minimum
                || p.value > p.maximum
                || (p.iec_type == "BOOL"
                    && (p.unit != "1"
                        || p.minimum < 0.0
                        || p.maximum > 1.0
                        || (p.value != 0.0 && p.value != 1.0)))
            {
                return Err("invalid typed parameter override or bounds".into());
            }
            parameters.insert(p.symbol.clone(), p.value);
        }
        self.settings().validate(1, &symbols)?;
        if let Some(relay) = &self.feedback_relay {
            relay.validate(self.step_ms, self.duration_ms)?;
            if symbols.contains(&relay.command.to_ascii_uppercase())
                || self
                    .events
                    .iter()
                    .any(|event| event.symbol.eq_ignore_ascii_case(&relay.command))
                || !self
                    .observations
                    .iter()
                    .any(|channel| channel.symbol == relay.process_variable)
                || !self
                    .observations
                    .iter()
                    .any(|channel| channel.symbol == relay.command)
            {
                return Err("relay must observe its measurement/command and cannot compete with frozen parameters or events".into());
            }
        }
        Ok(parameters)
    }
}
