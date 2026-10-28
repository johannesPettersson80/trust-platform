use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Axis {
    pub symbol: String,
    pub unit: String,
    pub minimum: f64,
    pub maximum: f64,
    pub values: Vec<f64>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Channel {
    pub symbol: String,
    pub unit: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Event {
    pub at_ms: u64,
    pub symbol: String,
    pub value: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Bound {
    pub id: String,
    pub symbol: String,
    pub minimum: f64,
    pub maximum: f64,
    pub start_ms: u64,
    pub end_ms: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Objective {
    pub reference_symbol: String,
    pub feedback_symbol: String,
    pub error_unit: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub expected_project_revision: String,
    pub execution: String,
    pub grid: Vec<Axis>,
    pub trial_count: usize,
    pub step_ms: u64,
    pub duration_ms: u64,
    pub seed: u64,
    pub max_total_cycles: u64,
    pub wall_budget_ms: u64,
    pub per_trial_wall_ms: u64,
    pub initial_values: BTreeMap<String, f64>,
    pub events: Vec<Event>,
    pub observations: Vec<Channel>,
    pub requirements: Vec<Bound>,
    pub objective: Objective,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Identity {
    pub include_results: Option<bool>,
    pub job_id: String,
    pub expected_project_revision: String,
}
impl Request {
    pub fn settings(&self) -> super::execution_settings::ExecutionSettings {
        super::execution_settings::ExecutionSettings {
            execution: self.execution.clone(),
            step_ms: self.step_ms,
            duration_ms: self.duration_ms,
            seed: self.seed,
            max_total_cycles: self.max_total_cycles,
            wall_budget_ms: self.wall_budget_ms,
            per_trial_wall_ms: self.per_trial_wall_ms,
            // Search trials keep every grid point; only an isolated run may average them.
            record_every: 1,
            initial_values: self.initial_values.clone(),
            events: self.events.clone(),
            observations: self.observations.clone(),
        }
    }

    pub fn validate(&self) -> Result<Vec<BTreeMap<String, f64>>, String> {
        if !(1..=4).contains(&self.grid.len())
            || !(1..=64).contains(&self.trial_count)
            || self.requirements.len() > 64
        {
            return Err("invalid explicit grid/count bounds".into());
        }
        let mut symbols = HashSet::new();
        let mut candidates = vec![BTreeMap::new()];
        for axis in &self.grid {
            if axis.symbol.is_empty()
                || axis.symbol.len() > 256
                || axis.unit.trim().is_empty()
                || !symbols.insert(axis.symbol.to_ascii_uppercase())
                || !(1..=16).contains(&axis.values.len())
                || !axis.minimum.is_finite()
                || !axis.maximum.is_finite()
                || axis.minimum > axis.maximum
            {
                return Err("invalid or duplicate grid axis".into());
            }
            let mut unique = Vec::new();
            for value in &axis.values {
                if !value.is_finite()
                    || *value < axis.minimum
                    || *value > axis.maximum
                    || unique.contains(value)
                {
                    return Err(
                        "grid values must be finite, unique and within declared bounds".into(),
                    );
                }
                unique.push(*value);
            }
            if candidates.len() * axis.values.len() > 64 {
                return Err("Cartesian grid exceeds 64 trials".into());
            }
            candidates = candidates
                .iter()
                .flat_map(|current| {
                    axis.values.iter().map(move |value| {
                        let mut next = current.clone();
                        next.insert(axis.symbol.clone(), *value);
                        next
                    })
                })
                .collect();
        }
        if candidates.len() != self.trial_count {
            return Err("trial_count must equal complete Cartesian grid".into());
        }
        self.settings().validate(self.trial_count, &symbols)?;
        let observed: HashSet<_> = self
            .observations
            .iter()
            .map(|channel| channel.symbol.as_str())
            .collect();
        for symbol in [
            &self.objective.reference_symbol,
            &self.objective.feedback_symbol,
        ] {
            if !observed.contains(symbol.as_str()) || symbols.contains(&symbol.to_ascii_uppercase())
            {
                return Err(
                    "objective channels must be observed and cannot be swept parameters".into(),
                );
            }
            if self
                .observations
                .iter()
                .find(|c| &c.symbol == symbol)
                .unwrap()
                .unit
                != self.objective.error_unit
            {
                return Err("objective channel units must match declared error unit".into());
            }
        }
        let mut ids = HashSet::new();
        for bound in &self.requirements {
            if bound.id.is_empty()
                || !ids.insert(&bound.id)
                || !observed.contains(bound.symbol.as_str())
                || !bound.minimum.is_finite()
                || !bound.maximum.is_finite()
                || bound.minimum > bound.maximum
                || bound.start_ms >= bound.end_ms
                || bound.end_ms > self.duration_ms
                || bound.start_ms % self.step_ms != 0
                || bound.end_ms % self.step_ms != 0
            {
                return Err("invalid observed requirement window or bounds".into());
            }
        }
        Ok(candidates)
    }
}
