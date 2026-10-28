//! The start of an experiment (runtime protocol annex, E5.3; specification 35, A8.5): the answer the runtime gives to
//! `control.experiment.start`, checked before the experiment counts as started (`experiment.ts`, `start`).

use serde::Deserialize;
use serde_json::Value;

use super::status::{
    classify_control_experiment_status, ControlExperimentRuntimePhase, RestorationState,
};
use crate::failure::Failure;

/// An experiment the runtime has started (E5.3): its identifier, and whether a restoring write was queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentStart {
    /// The experiment's identifier, the one that was started.
    pub experiment_id: String,
    /// `pending` when a restoring write was queued, `not_required` when none was.
    pub restoration: RestorationState,
}

#[derive(Deserialize)]
struct StartAnswer {
    experiment_id: String,
    restoration: RestorationState,
}

/// Checks the answer of `control.experiment.start` for the experiment `experiment_id` (E5.3). The answer must name that
/// experiment, its status must be `running`, and its restoration must be `pending` or `not_required`. Any other answer is never
/// confirmation (E11.8).
///
/// # Errors
/// The answer does not describe a started experiment of that identifier.
pub fn checked_experiment_start(
    answer: &Value,
    experiment_id: &str,
) -> Result<ExperimentStart, Failure> {
    let refused = || Failure::invalid_response("control.experiment.start");
    if classify_control_experiment_status(answer) != ControlExperimentRuntimePhase::Running {
        return Err(refused());
    }
    let start: StartAnswer = crate::json::decode_value(answer.clone()).map_err(|_| refused())?;
    if start.experiment_id != experiment_id {
        return Err(refused());
    }
    Ok(ExperimentStart {
        experiment_id: start.experiment_id,
        restoration: start.restoration,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::checked_experiment_start;
    use crate::experiment::RestorationState;

    #[test]
    fn a_running_experiment_with_its_restoration_pending_or_not_required_is_started() {
        let pending = checked_experiment_start(
            &json!({ "experiment_id": "experiment-1", "status": "running", "restoration": "pending", "address": "%QL0" }),
            "experiment-1",
        )
        .expect("a started experiment");
        assert_eq!(pending.restoration, RestorationState::Pending);
        let not_required = checked_experiment_start(
            &json!({ "experiment_id": "experiment-1", "status": "running", "restoration": "not_required" }),
            "experiment-1",
        )
        .expect("a started experiment");
        assert_eq!(not_required.restoration, RestorationState::NotRequired);
    }

    #[test]
    fn any_other_answer_is_never_a_started_experiment() {
        let refused = [
            json!({ "experiment_id": "experiment-1", "status": "paused", "restoration": "pending" }),
            json!({ "experiment_id": "experiment-1", "status": "running", "restoration": "restored" }),
            json!({ "experiment_id": "experiment-1", "status": "running", "restoration": "failed" }),
            json!({ "experiment_id": "experiment-1", "status": "running" }),
            json!({ "experiment_id": "someone-else", "status": "running", "restoration": "pending" }),
            json!({ "experiment_id": 7, "status": "running", "restoration": "pending" }),
        ];
        for answer in refused {
            assert!(
                checked_experiment_start(&answer, "experiment-1").is_err(),
                "{answer}"
            );
        }
    }
}
