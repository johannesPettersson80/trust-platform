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

/// The process time in ms of the execution at which the runtime applied the experiment's last trajectory point, from its status
/// answer (`last_applied.process_time_ms`, annex E5.6); `None` while the runtime has applied none. For a single step it is the
/// execution at which the step took effect, where its response starts (B3.4 step 4).
#[must_use]
pub fn applied_process_ms(status: &Value) -> Option<u64> {
    status.get("last_applied")?.get("process_time_ms")?.as_u64()
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

/// The hold tracker: the outputs seen after the step was started, when the runtime applied it, and whether the experiment was
/// finished early.
#[derive(Debug, Clone, PartialEq)]
pub struct HoldTracker {
    baseline: Baseline,
    /// The plant input's baseline, for a closed-loop setpoint step whose baseline measured it (B3.4 step 4).
    input_baseline: Option<Baseline>,
    step_seconds: f64,
    /// The process time of the execution at which the runtime applied the step, once its status has reported it: the response
    /// starts there (B3.4 step 4).
    applied_seconds: Option<f64>,
    single: bool,
    duration_seconds: f64,
    points: Vec<TimedValue>,
    inputs: Vec<TimedValue>,
    finished: bool,
    reported_seconds: f64,
}

impl HoldTracker {
    /// A tracker for the hold of `plan`, which steps from `baseline` and was started at process time `step_seconds`. Only a
    /// single-step experiment (one trajectory point, of a setpoint, load or manual-output step) is finished early.
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
            applied_seconds: None,
            single,
            duration_seconds: plan.duration_ms / 1_000.0,
            points: Vec::new(),
            inputs: Vec::new(),
            finished: false,
            reported_seconds: f64::NEG_INFINITY,
        }
    }

    /// Notes the process time, in seconds of the recording (A3.2), of the execution at which the runtime applied the step, as
    /// its status reports it (`last_applied`, annex E5.6). The first report counts: the response starts there (B3.4 step 4).
    pub fn step_applied(&mut self, seconds: f64) {
        self.applied_seconds.get_or_insert(seconds);
    }

    /// Feeds one turn of the hold: the state the runtime reported, the page of samples read after it, and the wall time.
    /// Only the outputs from the step's application count (B3.4 step 4). The experiment is finished early once the runtime has
    /// reported the step applied and the response has left the noise band and settled (B3.4 steps 4 and 5), and only once.
    pub fn feed(
        &mut self,
        watch: &mut Watch,
        state: HoldState,
        samples: &[FollowSample],
        wall_ms: f64,
    ) -> HoldStep {
        for value in watch.observe(samples, wall_ms) {
            if value.seconds > self.step_seconds {
                self.points.push(value);
            }
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
        let finish_early = self.single
            && !self.finished
            && self
                .applied_seconds
                .is_some_and(|applied| self.settled_since(applied));
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

    /// Whether the response that starts at `applied`, the step's application, has left the noise band and settled, with the
    /// plant input of a closed-loop setpoint step settled too (B3.4 step 4). The samples before the application, which the
    /// runtime recorded while the start was on its way to it, are not part of the response.
    fn settled_since(&self, applied: f64) -> bool {
        let response = since(&self.points, applied);
        let left_band = response
            .iter()
            .any(|point| left_noise_band(point.value, self.baseline.mean, self.baseline.noise));
        let input_settled = self
            .input_baseline
            .as_ref()
            .is_none_or(|input| has_settled(since(&self.inputs, applied), input, applied));
        left_band && has_settled(response, &self.baseline, applied) && input_settled
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

/// The points at or after `seconds`; the points are in process order.
fn since(points: &[TimedValue], seconds: f64) -> &[TimedValue] {
    &points[points.partition_point(|point| point.seconds < seconds)..]
}
