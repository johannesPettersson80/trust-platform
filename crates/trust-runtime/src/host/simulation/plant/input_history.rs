//! Ordered, bounded transport history shared by scalar and multichannel plants.
use std::collections::VecDeque;

use super::{PlantConfig, PlantModelConfig, RuntimeError};

const MAX_HISTORY_ENTRIES_PER_PLANT: u64 = 100_000;
const HISTORY_ANCHOR_MARGIN: u64 = 3;

#[derive(Debug, Clone)]
pub(super) struct InputHistory {
    delays_nanos: Vec<i64>,
    channels: Vec<VecDeque<(i64, f64)>>,
}

impl InputHistory {
    fn configured_delays(
        model: &PlantModelConfig,
        discrete_state_space: bool,
        input_delays_nanos: &[i64],
    ) -> Vec<i64> {
        match model {
            PlantModelConfig::FirstOrderDeadTime { dead_time_s, .. }
            | PlantModelConfig::IntegratorDeadTime { dead_time_s, .. } => {
                vec![(*dead_time_s * 1_000_000_000.0).round() as i64]
            }
            PlantModelConfig::StateSpace { .. }
                if !discrete_state_space && input_delays_nanos.iter().any(|delay| *delay > 0) =>
            {
                input_delays_nanos.to_vec()
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn validate_configuration(
        plant_id: &str,
        model: &PlantModelConfig,
        discrete_state_space: bool,
        input_delays_nanos: &[i64],
        step_nanos: i64,
    ) -> Result<(), RuntimeError> {
        let delays = Self::configured_delays(model, discrete_state_space, input_delays_nanos);
        Self::checked_capacities(plant_id, &delays, step_nanos).map(|_| ())
    }

    fn checked_capacities(
        plant_id: &str,
        delays: &[i64],
        step_nanos: i64,
    ) -> Result<Vec<usize>, RuntimeError> {
        let invalid_capacity = || {
            RuntimeError::InvalidConfig(format!(
            "plant '{plant_id}' transport history capacity arithmetic is invalid; limit {MAX_HISTORY_ENTRIES_PER_PLANT} entries"
        ).into())
        };
        let step = u64::try_from(step_nanos)
            .ok()
            .filter(|step| *step > 0)
            .ok_or_else(invalid_capacity)?;
        let mut total = 0_u64;
        let mut capacities = Vec::new();
        for delay in delays {
            let delay = u64::try_from(*delay).map_err(|_| invalid_capacity())?;
            let capacity = (delay / step)
                .checked_add(u64::from(delay % step != 0))
                .and_then(|steps| steps.checked_add(HISTORY_ANCHOR_MARGIN))
                .ok_or_else(invalid_capacity)?;
            total = total.checked_add(capacity).ok_or_else(invalid_capacity)?;
            if total > MAX_HISTORY_ENTRIES_PER_PLANT {
                return Err(RuntimeError::InvalidConfig(format!(
                    "plant '{plant_id}' transport history requires {total} entries; limit {MAX_HISTORY_ENTRIES_PER_PLANT} entries"
                ).into()));
            }
            capacities.push(usize::try_from(capacity).map_err(|_| invalid_capacity())?);
        }
        Ok(capacities)
    }

    pub(super) fn for_config(config: &PlantConfig) -> Self {
        let delays_nanos = Self::configured_delays(
            &config.model,
            config.discrete_state_space,
            &config.input_delays_nanos,
        );
        // PlantConfig is private to this owner and passes the same check before
        // construction. No history allocation occurs until the aggregate passes.
        let capacities =
            Self::checked_capacities(&config.id, &delays_nanos, config.step.as_nanos())
                .expect("transport history capacity validated before plant construction");
        let channels = capacities
            .into_iter()
            .map(VecDeque::with_capacity)
            .collect();
        Self {
            delays_nanos,
            channels,
        }
    }

    pub(super) fn enabled(&self) -> bool {
        !self.channels.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.channels.iter().map(VecDeque::len).sum()
    }

    pub(super) fn clear(&mut self) {
        self.channels.iter_mut().for_each(VecDeque::clear);
    }

    pub(super) fn copy_runtime_from(&mut self, source: &Self) {
        // Preserve the existing checkpoint owner's reserved channel buffers.
        self.delays_nanos.clone_from(&source.delays_nanos);
        self.channels.clone_from(&source.channels);
    }

    pub(super) fn channel(&self, index: usize) -> &VecDeque<(i64, f64)> {
        &self.channels[index]
    }

    pub(super) fn record(&mut self, time: i64, applied_inputs: &[f64]) {
        for (history, input) in self.channels.iter_mut().zip(applied_inputs) {
            history.push_back((time, *input));
        }
    }

    pub(super) fn prune(&mut self, model_time: i64, step: i64) {
        for (history, delay) in self.channels.iter_mut().zip(&self.delays_nanos) {
            let cutoff = model_time.saturating_sub(*delay).saturating_sub(step);
            // Keep the newest sample before the window as its held-value anchor.
            while history.len() > 1 && history[1].0 <= cutoff {
                history.pop_front();
            }
        }
    }

    pub(super) fn values_at(&self, time: i64, initial: &[f64], output: &mut [f64]) {
        for (index, (history, delay)) in self.channels.iter().zip(&self.delays_nanos).enumerate() {
            let sampled_at = time.saturating_sub(*delay);
            output[index] = history
                .iter()
                .rev()
                .find(|(recorded, _)| *recorded <= sampled_at)
                .map_or(initial[index], |(_, value)| *value);
        }
    }

    pub(super) fn next_boundary(&self, start: i64, end: i64) -> i64 {
        self.channels
            .iter()
            .zip(&self.delays_nanos)
            .flat_map(|(history, delay)| {
                history
                    .iter()
                    .map(move |(time, _)| time.saturating_add(*delay))
            })
            .filter(|due| *due > start && *due < end)
            .min()
            .unwrap_or(end)
    }
}
