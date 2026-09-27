//! Shared immutable execution bounds, independent of search/ordinary Test purpose.
use super::contracts::{Channel, Event};
use std::collections::{BTreeMap, HashSet};
pub(super) const MAX_OBSERVATIONS: usize = 32;
#[derive(Clone, Debug)]
pub(super) struct ExecutionSettings {
    pub execution: String,
    pub step_ms: u64,
    pub duration_ms: u64,
    pub seed: u64,
    pub max_total_cycles: u64,
    pub wall_budget_ms: u64,
    pub per_trial_wall_ms: u64,
    pub initial_values: BTreeMap<String, f64>,
    pub events: Vec<Event>,
    pub observations: Vec<Channel>,
}
impl ExecutionSettings {
    pub fn validate(&self, count: usize, parameters: &HashSet<String>) -> Result<(), String> {
        if self.execution != "isolated-simulation" {
            return Err("only isolated-simulation execution is supported".into());
        }
        if !(1..=1000).contains(&self.step_ms)
            || self.duration_ms == 0
            || self.duration_ms > 600_000
            || self.duration_ms % self.step_ms != 0
            || !(1..=60_000).contains(&self.wall_budget_ms)
            || !(1..=10_000).contains(&self.per_trial_wall_ms)
            || self.per_trial_wall_ms > self.wall_budget_ms
        {
            return Err("invalid explicit execution time/budget bounds".into());
        }
        let cycles = (self.duration_ms / self.step_ms + 1) * count as u64;
        if self.seed > 9_007_199_254_740_991
            || self.duration_ms / self.step_ms + 1 > 12_000
            || self.max_total_cycles > 50_000
            || cycles > self.max_total_cycles
            || self.observations.is_empty()
            || self.observations.len() > MAX_OBSERVATIONS
            || cycles * self.observations.len() as u64 > 250_000
            || self.initial_values.len() > 64
            || self.events.len() > 1024
        {
            return Err("execution exceeds cycle, sample or declaration bounds".into());
        }
        let mut observed = HashSet::new();
        for channel in &self.observations {
            if channel.symbol.is_empty()
                || channel.symbol.len() > 256
                || channel.unit.is_empty()
                || !observed.insert(channel.symbol.as_str())
            {
                return Err("invalid or duplicate observation channel".into());
            }
        }
        let mut initials = HashSet::new();
        for (symbol, value) in &self.initial_values {
            if symbol.is_empty()
                || symbol.len() > 256
                || !initials.insert(symbol.to_ascii_uppercase())
                || parameters.contains(&symbol.to_ascii_uppercase())
                || !value.is_finite()
            {
                return Err("initial values cannot override controller parameters".into());
            }
        }
        let mut targets = HashSet::new();
        let mut previous = 0;
        for event in &self.events {
            if event.symbol.is_empty()
                || event.symbol.len() > 256
                || event.at_ms < previous
                || event.at_ms > self.duration_ms
                || event.at_ms % self.step_ms != 0
                || !event.value.is_finite()
                || parameters.contains(&event.symbol.to_ascii_uppercase())
                || !targets.insert((event.at_ms, event.symbol.to_ascii_uppercase()))
            {
                return Err("events must be ordered, unique, scan-aligned and cannot override controller parameters".into());
            }
            previous = event.at_ms;
        }
        Ok(())
    }
}
