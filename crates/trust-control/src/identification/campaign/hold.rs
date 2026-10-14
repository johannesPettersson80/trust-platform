//! The hold of a step (`IdentificationCampaign.hold`; specification 35, B3.4 steps 4 to 6). The tracker is a pure state machine:
//! the driver feeds it the runtime's experiment state and each page of the recording, and it answers whether the hold goes on,
//! whether a single-step experiment is to be finished early now, and what progress text is due. A pulse, a PRBS or a relay runs
//! its duration; only a single step is finished early.

#![allow(clippy::module_name_repetitions)] // The hold's names state what the step is held for.

use serde_json::Value;
use trust_control_math::sampling::statistics::TimedValue;

use super::baseline::BaselineSummary;
use super::{format_duration, good_inputs, progress_due, scale_text, FollowSample, Watch};
use crate::experiment::{
    classify_control_experiment_status, ControlExperimentKind, ControlExperimentPlan,
    ControlExperimentRuntimePhase,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::settle::{has_settled, left_noise_band, Baseline};

/// Whether the runtime's experiment has ended, as its status answer says (`phase === "terminal"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldState {
    /// The experiment runs, or it has ended and its restoration is still pending: the hold goes on.
    Holding,
    /// The experiment has ended and its restoration is settled: the hold is over.
    Ended,
}

/// Reads the state of the experiment from its status answer (`classifyControlExperimentStatus`, annex E5.6).
///
/// # Errors
/// The answer is outside the runtime's vocabulary.
pub fn hold_state(status: &Value) -> Result<HoldState, Failure> {
    match classify_control_experiment_status(status) {
        ControlExperimentRuntimePhase::Invalid => Err(Failure::new(
            "The runtime returned an invalid experiment status.",
            Cause::Runtime,
            NextAction::None,
        )),
        ControlExperimentRuntimePhase::Terminal => Ok(HoldState::Ended),
        ControlExperimentRuntimePhase::Running
        | ControlExperimentRuntimePhase::AwaitingRestoration => Ok(HoldState::Holding),
    }
}

/// What one turn of the hold asks of the driver.
#[derive(Debug, Clone, PartialEq)]
pub enum HoldStep {
    /// The experiment has ended. The driver returns the runtime's terminal state; no finish is sent and no progress is due.
    Ended,
    /// The hold goes on. `finish_early` asks the driver to finish the experiment now (B3.4 step 5); `progress` is the text to
    /// report now, when one is due.
    Continue {
        /// Whether the single step has left the noise band and settled, so the experiment is finished early.
        finish_early: bool,
        /// The progress text, when one is due (at most one every ten seconds of process time).
        progress: Option<String>,
    },
}

/// The hold tracker: the outputs seen after the step started, whether they have left the noise band, and whether the
/// experiment was finished early.
#[derive(Debug, Clone, PartialEq)]
pub struct HoldTracker {
    baseline: Baseline,
    /// The plant input's baseline, for a closed-loop setpoint step whose baseline measured it (B3.4 step 4).
    input_baseline: Option<Baseline>,
    step_seconds: f64,
    single: bool,
    duration_seconds: f64,
    points: Vec<TimedValue>,
    inputs: Vec<TimedValue>,
    left_band: bool,
    finished: bool,
    reported_seconds: f64,
}

impl HoldTracker {
    /// A tracker for the hold of `plan`, which steps from `baseline` at process time `step_seconds`. Only a single-step
    /// experiment (one trajectory point, of a setpoint, load or manual-output step) is finished early.
    #[must_use]
    pub fn new(
        plan: &ControlExperimentPlan,
        baseline: &BaselineSummary,
        step_seconds: f64,
    ) -> Self {
        let single = plan.trajectory.len() == 1
            && matches!(
                plan.kind,
                ControlExperimentKind::SetpointStep
                    | ControlExperimentKind::LoadStep
                    | ControlExperimentKind::MvStep
            );
        // In a closed-loop setpoint step the plant input is part of the response: the controller keeps moving it while a slow
        // mode settles, so the step is finished early only once it has settled too (B3.4 step 4).
        let input_baseline = (plan.kind == ControlExperimentKind::SetpointStep)
            .then(|| baseline.input_settle_baseline())
            .flatten();
        Self {
            baseline: baseline.settle_baseline(),
            input_baseline,
            step_seconds,
            single,
            duration_seconds: plan.duration_ms / 1_000.0,
            points: Vec::new(),
            inputs: Vec::new(),
            left_band: false,
            finished: false,
            reported_seconds: f64::NEG_INFINITY,
        }
    }

    /// Feeds one turn of the hold: the state the runtime reported, the page of samples read after it, and the wall time.
    /// Only the outputs after the step count (B3.4 step 4). The experiment is finished early once it has left the noise band
    /// and settled (B3.4 steps 4 and 5), and only once.
    pub fn feed(
        &mut self,
        watch: &mut Watch,
        state: HoldState,
        samples: &[FollowSample],
        wall_ms: f64,
    ) -> HoldStep {
        for value in watch.observe(samples, wall_ms) {
            if value.seconds <= self.step_seconds {
                continue;
            }
            self.left_band |= left_noise_band(value.value, self.baseline.mean, self.baseline.noise);
            self.points.push(value);
        }
        if self.input_baseline.is_some() {
            let step_seconds = self.step_seconds;
            self.inputs.extend(
                good_inputs(samples)
                    .into_iter()
                    .filter(|value| value.seconds > step_seconds),
            );
        }
        if state == HoldState::Ended {
            return HoldStep::Ended;
        }
        let input_settled = self
            .input_baseline
            .as_ref()
            .is_none_or(|input| has_settled(&self.inputs, input, self.step_seconds));
        let finish_early = self.single
            && !self.finished
            && self.left_band
            && has_settled(&self.points, &self.baseline, self.step_seconds)
            && input_settled;
        if finish_early {
            self.finished = true;
        }
        let elapsed = (watch.newest_seconds() - self.step_seconds).max(0.0);
        let progress = if progress_due(elapsed, self.reported_seconds) {
            self.reported_seconds = elapsed;
            Some(self.progress_text(watch, elapsed, wall_ms))
        } else {
            None
        };
        HoldStep::Continue {
            finish_early,
            progress,
        }
    }

    /// The progress of the hold, with the time scale when it is observed (A3.6).
    fn progress_text(&self, watch: &Watch, elapsed: f64, wall_ms: f64) -> String {
        let note = watch.time_scale(wall_ms).map_or_else(String::new, |scale| {
            format!(" (time scale {}×)", scale_text(scale))
        });
        format!(
            "Holding the step until the response settles: {} of at most {}{note}.",
            format_duration(elapsed),
            format_duration(self.duration_seconds)
        )
    }
}
