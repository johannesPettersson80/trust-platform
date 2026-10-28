//! Shared immutable execution bounds, independent of search/ordinary Test purpose.
use super::contracts::{Channel, Event};
use std::collections::{BTreeMap, HashSet};

/// Most observed channels one run or trial records (specification 35, A7).
pub(super) const MAX_OBSERVATIONS: usize = 64;
/// Longest simulated duration of one isolated run (A7): 24 h.
const MAX_DURATION_MS: u64 = 86_400_000;
/// Most samples one run keeps (A7). A sample is one record period (A4.2).
pub(super) const MAX_RUN_SAMPLES: u64 = 100_000;
/// Most channel samples one isolated run or search job holds (A7). The worker's byte budgets are
/// derived from it in `worker_checkpoint.rs` and `worker.rs`: the largest valid run at this limit
/// writes about 57 MiB, below the 64 MiB ceiling.
pub(super) const MAX_CHANNEL_SAMPLES: u64 = 1_600_000;
/// Longest unit of an observed channel, in bytes. Units are written into every run's header, so the
/// header bound of `worker_checkpoint.rs` depends on it.
const MAX_UNIT_BYTES: usize = 256;
/// Most scans one job executes (A7).
const MAX_SCANS: u64 = 10_000_000;
/// Wall time one run may take (A7): 10 min.
const MAX_RUN_WALL_MS: u64 = 600_000;
/// Wall time one job may take (A7): 60 min.
const MAX_JOB_WALL_MS: u64 = 3_600_000;
/// Executions one sample may average (A4.2).
const MAX_RECORD_EVERY: u64 = 100_000;

#[derive(Clone, Debug)]
pub(super) struct ExecutionSettings {
    pub execution: String,
    pub step_ms: u64,
    pub duration_ms: u64,
    pub seed: u64,
    pub max_total_cycles: u64,
    pub wall_budget_ms: u64,
    pub per_trial_wall_ms: u64,
    /// Executions averaged into one sample (A4.2); one keeps every grid point.
    pub record_every: u64,
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
            || !self.duration_ms.is_multiple_of(self.step_ms)
            || self.wall_budget_ms == 0
            || self.per_trial_wall_ms == 0
            || self.per_trial_wall_ms > self.wall_budget_ms
        {
            return Err("invalid explicit execution time/budget bounds".into());
        }
        if self.duration_ms > MAX_DURATION_MS {
            return Err(format!(
                "simulated duration {} ms exceeds the limit of {MAX_DURATION_MS} ms (24 h)",
                self.duration_ms
            ));
        }
        if self.wall_budget_ms > MAX_JOB_WALL_MS {
            return Err(format!(
                "job wall budget {} ms exceeds the limit of {MAX_JOB_WALL_MS} ms (60 min)",
                self.wall_budget_ms
            ));
        }
        if self.per_trial_wall_ms > MAX_RUN_WALL_MS {
            return Err(format!(
                "per-run wall budget {} ms exceeds the limit of {MAX_RUN_WALL_MS} ms (10 min)",
                self.per_trial_wall_ms
            ));
        }
        let channels = self.observations.len();
        if channels > MAX_OBSERVATIONS {
            return Err(format!(
                "{channels} observed channels exceed the limit of {MAX_OBSERVATIONS}"
            ));
        }
        if !(1..=MAX_RECORD_EVERY).contains(&self.record_every) {
            return Err(format!(
                "record_every must be 1 to {MAX_RECORD_EVERY} executions"
            ));
        }
        let trials = count as u64;
        let points = self.duration_ms / self.step_ms + 1;
        let samples = points.div_ceil(self.record_every);
        if samples > MAX_RUN_SAMPLES {
            return Err(format!(
                "{samples} samples per run exceed the limit of {MAX_RUN_SAMPLES}; raise record_every"
            ));
        }
        let total_samples = samples * trials;
        if total_samples * channels as u64 > MAX_CHANNEL_SAMPLES {
            return Err(format!(
                "{total_samples} samples × {channels} channels exceed the limit of {MAX_CHANNEL_SAMPLES} channel samples"
            ));
        }
        if self.max_total_cycles > MAX_SCANS {
            return Err(format!(
                "max_total_cycles exceeds the limit of {MAX_SCANS} scans"
            ));
        }
        let cycles = points * trials;
        if self.seed > 9_007_199_254_740_991
            || cycles > self.max_total_cycles
            || self.observations.is_empty()
            || self.initial_values.len() > 64
            || self.events.len() > 1024
        {
            return Err("execution exceeds cycle, sample or declaration bounds".into());
        }
        let mut observed = HashSet::new();
        for channel in &self.observations {
            if channel.unit.len() > MAX_UNIT_BYTES {
                return Err(format!(
                    "observation unit of {} bytes exceeds the limit of {MAX_UNIT_BYTES}",
                    channel.unit.len()
                ));
            }
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
