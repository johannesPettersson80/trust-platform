//! The loop of an identification campaign on a connected runtime (`IdentificationCampaign.baseline` and `.hold`; specification 35,
//! B3.4 and A3.2). It follows the recording page by page, polls the experiment, finishes or aborts it when the rules say, paces
//! itself, reports progress, and stops when it is cancelled or when the runtime falls silent. Every decision is taken by the
//! trackers in `trust-control`; this module makes the calls in the order those decisions need, through the traits below.

use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use trust_control::experiment::ControlExperimentPlan;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::campaign::baseline::{
    BaselineStep, BaselineSummary, BaselineTracker,
};
use trust_control::identification::campaign::hold::{hold_state, HoldState, HoldStep, HoldTracker};
use trust_control::identification::campaign::{
    require_campaign_limits, silence_limit_ms, FollowSample, Silence, Watch, CANCELLED_TEXT,
    POLL_MILLISECONDS,
};
use trust_control::identification::reconcile::StatusRead;
use trust_control::loop_definition::LoopDefinition;
use trust_control::runtime::RuntimeConnection;

use crate::operation::OperationContext;
use crate::runtime_client::RuntimeClient;

/// The clock of a campaign: the wall time it supervises, and the pauses between its polls (`CampaignClock`). The wall clock never
/// decides a phase; process time does (A3.2).
pub trait CampaignClock {
    /// The wall time in milliseconds, from an origin the clock fixes.
    fn now_ms(&self) -> f64;
    /// Pauses for `milliseconds`.
    fn wait(&self, milliseconds: u64);
}

/// The clock of this process: the monotonic time since it was made, and a thread sleep (`systemClock`).
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    /// A clock whose origin is now.
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl CampaignClock for SystemClock {
    fn now_ms(&self) -> f64 {
        self.origin.elapsed().as_secs_f64() * 1_000.0
    }

    fn wait(&self, milliseconds: u64) {
        thread::sleep(Duration::from_millis(milliseconds));
    }
}

/// The recording a campaign reads from (`RecordingFollower`): each call returns the next page of its samples, in process order.
pub trait RecordingFollower {
    /// The next page of samples; empty when the runtime has produced none since the last page.
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers out of protocol.
    fn follow(&mut self) -> Result<Vec<FollowSample>, Failure>;
}

/// The experiment a campaign holds (`ControlExperimentClient`): its status, and the two writes that end a hold early or stop it
/// (E5.5, E5.6).
pub trait ExperimentClient {
    /// The experiment's status answer as the runtime gave it; `hold_state` checks it.
    ///
    /// # Errors
    /// The runtime cannot be reached, or refuses the request.
    fn status(&mut self, experiment_id: &str) -> Result<Value, Failure>;

    /// Finishes a running experiment early and normally.
    ///
    /// # Errors
    /// The runtime refuses, cannot be reached, or answers out of protocol.
    fn finish(&mut self, experiment_id: &str) -> Result<(), Failure>;

    /// Aborts a running experiment.
    ///
    /// # Errors
    /// The runtime refuses, cannot be reached, or answers out of protocol.
    fn abort(&mut self, experiment_id: &str) -> Result<(), Failure>;
}

/// What the operation that runs a campaign tells it and shows: its cancellation and its progress (`cancelled` and `progress`).
pub trait CampaignReport {
    /// Whether the operation was cancelled; the campaign checks it between its steps.
    fn is_cancelled(&self) -> bool;

    /// Reports what the campaign is doing.
    fn progress(&self, text: &str);
}

impl CampaignReport for OperationContext {
    fn is_cancelled(&self) -> bool {
        OperationContext::is_cancelled(self)
    }

    fn progress(&self, text: &str) {
        OperationContext::progress(self, text);
    }
}

/// One campaign on a connected runtime, with the ports it reads and writes (`IdentificationCampaign`). Its watch is shared by the
/// baseline and the hold, so the newest process time and the time scale carry across the two phases (A3.6).
pub struct IdentifyCampaign<'a, C, R, E, P> {
    loop_definition: &'a LoopDefinition,
    clock: &'a C,
    recorder: &'a mut R,
    experiment: &'a mut E,
    report: &'a P,
    offer_setpoint_step: bool,
    expected_settling_seconds: Option<f64>,
    watch: Watch,
}

impl<'a, C, R, E, P> IdentifyCampaign<'a, C, R, E, P>
where
    C: CampaignClock,
    R: RecordingFollower,
    E: ExperimentClient,
    P: CampaignReport,
{
    /// A campaign of `loop_definition` on the given ports. `offer_setpoint_step` and `expected_settling_seconds` are passed to the
    /// baseline as [`BaselineTracker::new`] takes them.
    #[must_use]
    pub fn new(
        loop_definition: &'a LoopDefinition,
        clock: &'a C,
        recorder: &'a mut R,
        experiment: &'a mut E,
        report: &'a P,
        offer_setpoint_step: bool,
        expected_settling_seconds: Option<f64>,
    ) -> Self {
        Self {
            loop_definition,
            clock,
            recorder,
            experiment,
            report,
            offer_setpoint_step,
            expected_settling_seconds,
            watch: Watch::new(),
        }
    }

    /// The process time of the newest sample followed so far, in seconds (`seconds`).
    #[must_use]
    pub fn seconds(&self) -> f64 {
        self.watch.newest_seconds()
    }

    /// Follows the recording until the output is steady over one window, or refuses it with its rate (B3.4 step 2).
    ///
    /// # Errors
    /// The campaign is cancelled, the recording cannot be followed, the runtime stops sending samples, the output is still
    /// changing after the budget, or the campaign's limits cannot be read.
    pub fn baseline(&mut self, maximum_ms: u64) -> Result<BaselineSummary, Failure> {
        let loop_definition = self.loop_definition;
        let mut tracker = BaselineTracker::new(
            &loop_definition.signals.process_variable,
            maximum_ms,
            loop_definition.controller.task_period_ms,
            self.expected_settling_seconds,
            self.offer_setpoint_step,
        );
        let mut silence = Silence::starting_at(
            self.clock.now_ms(),
            campaign_silence_limit(loop_definition, self.expected_settling_seconds)?,
        );
        loop {
            self.check_cancelled()?;
            let page = self.follow(&mut silence)?;
            match tracker.feed(&mut self.watch, &page, self.clock.now_ms())? {
                BaselineStep::Steady(summary) => return Ok(summary),
                BaselineStep::Waiting { progress } => self.report_progress(progress.as_deref()),
            }
            self.pace();
        }
    }

    /// Holds the step until it has settled, finishes it early when the rules say, and returns the runtime's status once the
    /// experiment has ended (B3.4 steps 4 to 6). A cancelled hold aborts its experiment first.
    ///
    /// # Errors
    /// The status is outside the runtime's vocabulary, the status or finish or abort request fails, the recording cannot be
    /// followed, or the runtime stops sending samples.
    pub fn hold(
        &mut self,
        plan: &ControlExperimentPlan,
        baseline: &BaselineSummary,
        step_seconds: f64,
    ) -> Result<Value, Failure> {
        let mut tracker = HoldTracker::new(plan, baseline, step_seconds);
        let mut silence = Silence::starting_at(
            self.clock.now_ms(),
            campaign_silence_limit(self.loop_definition, self.expected_settling_seconds)?,
        );
        loop {
            if self.report.is_cancelled() {
                self.experiment.abort(&plan.id)?;
            }
            let status = self.experiment.status(&plan.id)?;
            let state: HoldState = hold_state(&status)?;
            let page = self.follow(&mut silence)?;
            match tracker.feed(&mut self.watch, state, &page, self.clock.now_ms()) {
                HoldStep::Ended => return Ok(status),
                HoldStep::Continue {
                    finish_early,
                    progress,
                } => {
                    if finish_early {
                        self.experiment.finish(&plan.id)?;
                    }
                    self.report_progress(progress.as_deref());
                }
            }
            self.pace();
        }
    }

    /// One page of the recording, supervised by the connection-loss timeout (A3.4, A7).
    fn follow(&mut self, silence: &mut Silence) -> Result<Vec<FollowSample>, Failure> {
        let page = self.recorder.follow()?;
        silence.observe_page(page.len(), self.clock.now_ms())?;
        Ok(page)
    }

    fn check_cancelled(&self) -> Result<(), Failure> {
        if self.report.is_cancelled() {
            return Err(Failure::new(
                CANCELLED_TEXT,
                Cause::Unclassified,
                NextAction::None,
            ));
        }
        Ok(())
    }

    fn report_progress(&self, progress: Option<&str>) {
        if let Some(text) = progress {
            self.report.progress(text);
        }
    }

    fn pace(&self) {
        self.clock.wait(POLL_MILLISECONDS);
    }
}

/// The experiment of a connected runtime: its status is read over a control connection of its own, and its finish and abort go
/// through the runtime client, which checks each answer (E5.5, E5.6).
pub struct ConnectedExperiment {
    connection: RuntimeConnection,
}

impl ConnectedExperiment {
    /// The experiment on `connection`.
    #[must_use]
    pub fn new(connection: RuntimeConnection) -> Self {
        Self { connection }
    }
}

impl ExperimentClient for ConnectedExperiment {
    fn status(&mut self, experiment_id: &str) -> Result<Value, Failure> {
        match RuntimeClient::read_experiment_status(&self.connection, experiment_id) {
            StatusRead::Answer(answer) => Ok(answer),
            StatusRead::Refused { code, text } => Err(Failure::from_runtime(
                code.as_deref().unwrap_or_default(),
                &text,
            )),
            StatusRead::Unreadable(text) => Err(Failure::unreachable(&text)),
        }
    }

    fn finish(&mut self, experiment_id: &str) -> Result<(), Failure> {
        RuntimeClient::connect(&self.connection)?
            .finish_experiment(experiment_id)
            .map(|_| ())
    }

    fn abort(&mut self, experiment_id: &str) -> Result<(), Failure> {
        RuntimeClient::connect(&self.connection)?
            .abort_experiment(experiment_id)
            .map(|_| ())
    }
}

/// The connection-loss timeout of a campaign's pages (A3.4, A7), read after the campaign's limits are checked, so that a missing
/// one is named where Limits opens (B3.4 step 1, B3.9). A resumed campaign reaches its hold without the run's own check.
fn campaign_silence_limit(
    loop_definition: &LoopDefinition,
    expected_settling_seconds: Option<f64>,
) -> Result<u64, Failure> {
    require_campaign_limits(loop_definition)?;
    silence_limit_ms(loop_definition, expected_settling_seconds)
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use serde_json::{json, Value};
    use trust_control::experiment::ControlExperimentPlan;
    use trust_control::failure::{Cause, Failure, NextAction};
    use trust_control::identification::campaign::baseline::BaselineSummary;
    use trust_control::identification::campaign::{
        FollowSample, CANCELLED_TEXT, SILENT_RUNTIME_TEXT,
    };
    use trust_control::loop_definition::LoopDefinition;

    use super::{
        CampaignClock, CampaignReport, ExperimentClient, IdentifyCampaign, RecordingFollower,
    };

    /// A clock that moves only when the campaign waits on it.
    struct FakeClock {
        now: Cell<f64>,
    }

    impl CampaignClock for FakeClock {
        fn now_ms(&self) -> f64 {
            self.now.get()
        }

        fn wait(&self, milliseconds: u64) {
            #[allow(clippy::cast_precision_loss)]
            // A poll is 100 ms; the test clock never nears 2⁵³.
            let step = milliseconds as f64;
            self.now.set(self.now.get() + step);
        }
    }

    /// A recording of a plant: one sample per second. It serves the samples up to the clock, or all of them through `through`
    /// at once, as the runtime does when a page arrives late (A3.2).
    struct FakeRecorder<'a> {
        clock: &'a FakeClock,
        plant: Box<dyn Fn(f64) -> f64>,
        next: f64,
        through: Option<f64>,
        silent: bool,
    }

    impl RecordingFollower for FakeRecorder<'_> {
        fn follow(&mut self) -> Result<Vec<FollowSample>, Failure> {
            if self.silent {
                return Ok(Vec::new());
            }
            let limit = self.through.unwrap_or(self.clock.now.get() / 1_000.0);
            let mut page = Vec::new();
            while self.next <= limit + 1e-9 {
                page.push(FollowSample {
                    elapsed_seconds: self.next,
                    good: true,
                    output: Some((self.plant)(self.next)),
                    input: None,
                });
                self.next += 1.0;
            }
            Ok(page)
        }
    }

    /// An experiment that runs for `duration_s` from its start, and ends early when it is finished or aborted.
    struct FakeExperiment<'a> {
        clock: &'a FakeClock,
        duration_s: f64,
        ended: Option<&'static str>,
        calls: Vec<String>,
    }

    impl ExperimentClient for FakeExperiment<'_> {
        fn status(&mut self, _experiment_id: &str) -> Result<Value, Failure> {
            let due = self.clock.now.get() / 1_000.0 >= self.duration_s;
            Ok(match self.ended {
                Some("finished_early") => {
                    json!({ "experiment_id": "experiment-1", "status": "completed",
                    "restoration": "restored", "stop_reason": "finished_early" })
                }
                Some(stop) => json!({ "experiment_id": "experiment-1", "status": "aborted",
                    "restoration": "restored", "stop_reason": stop }),
                None if due => json!({ "experiment_id": "experiment-1", "status": "completed",
                    "restoration": "restored", "stop_reason": "duration_complete" }),
                None => {
                    json!({ "experiment_id": "experiment-1", "status": "running", "restoration": "pending",
                    "stop_reason": null })
                }
            })
        }

        fn finish(&mut self, experiment_id: &str) -> Result<(), Failure> {
            self.calls.push(format!("finish {experiment_id}"));
            self.ended = Some("finished_early");
            Ok(())
        }

        fn abort(&mut self, experiment_id: &str) -> Result<(), Failure> {
            self.calls.push(format!("abort {experiment_id}"));
            self.ended = Some("user_abort");
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeReport {
        cancelled: Cell<bool>,
        texts: RefCell<Vec<String>>,
    }

    impl CampaignReport for FakeReport {
        fn is_cancelled(&self) -> bool {
            self.cancelled.get()
        }

        fn progress(&self, text: &str) {
            self.texts.borrow_mut().push(text.to_string());
        }
    }

    /// The tank of the `TypeScript` campaign tests: 0 to 100 degC, a 100 ms task, a 600 s maximum.
    fn tank_loop() -> LoopDefinition {
        serde_json::from_value(json!({
            "schemaVersion": 2, "id": "tank", "name": "Tank", "revision": "rev-1",
            "projectId": "project", "runtimeId": "runtime",
            "controller": { "instance": "Main.TIC101", "type": "Control.PID", "revision": "sha256:test",
                "action": "direct", "taskPeriodMs": 100 },
            "signals": {
                "setpoint": { "symbol": "Main.TIC101_Setpoint", "iecType": "LREAL", "direction": "read",
                    "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
                "processVariable": { "symbol": "Main.TT101.Value", "iecType": "LREAL", "direction": "read",
                    "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
                "manipulatedVariable": { "symbol": "Main.TIC101_Output", "iecType": "LREAL", "direction": "write",
                    "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
            },
            "limits": { "outputMinimum": 0, "outputMaximum": 100 },
            "observedSampleTiming": { "nominalPeriodMs": 100 },
            "experiment": { "execution": "control-studio", "kind": "observation", "amplitude": 0,
                "durationSeconds": 600, "seed": 1 }
        }))
        .expect("the tank loop")
    }

    /// The `TypeScript` simulated plant on process time: a step of `change` from `start`, with a drift and a periodic noise.
    fn plant(drift: f64, change: f64, start: f64) -> impl Fn(f64) -> f64 {
        move |seconds: f64| {
            let step = if seconds >= start {
                change * (1.0 - (-(seconds - start) / 50.0).exp())
            } else {
                0.0
            };
            45.0 + drift * seconds + step + 0.025 * ((1.7 * seconds).sin() + (2.3 * seconds).cos())
        }
    }

    fn setpoint_plan() -> ControlExperimentPlan {
        serde_json::from_value(json!({
            "id": "experiment-1", "source": "simulation", "kind": "setpoint_step",
            "affectedSignal": "Main.TIC101_Setpoint", "valueType": "LREAL", "from": 45, "to": 47,
            "unit": "degC", "minimum": 0, "maximum": 100, "durationMs": 540_000, "expectedScan": 300,
            "stopConditions": ["Abort"], "restorationValue": 45, "requiresPhysicalApproval": false,
            "trajectory": [{ "atMs": 0, "value": 47 }]
        }))
        .expect("the plan")
    }

    fn baseline_summary() -> BaselineSummary {
        BaselineSummary {
            mean: 45.0,
            noise: 0.03,
            half_range: 0.05,
            spacing_seconds: 1.0,
            window_seconds: 30.0,
            input_mean: None,
            input_noise: None,
        }
    }

    fn whole_text(failure: &Failure) -> &str {
        failure.detail.as_deref().unwrap_or(&failure.message)
    }

    #[test]
    fn a_five_minute_baseline_completes_in_process_time_when_the_samples_arrive_instantly() {
        let loop_definition = tank_loop();
        let clock = FakeClock {
            now: Cell::new(0.0),
        };
        let mut recorder = FakeRecorder {
            clock: &clock,
            plant: Box::new(plant(0.0, 0.0, f64::INFINITY)),
            next: 0.0,
            through: Some(400.0),
            silent: false,
        };
        let mut experiment = FakeExperiment {
            clock: &clock,
            duration_s: 0.0,
            ended: None,
            calls: Vec::new(),
        };
        let report = FakeReport::default();
        let mut campaign = IdentifyCampaign::new(
            &loop_definition,
            &clock,
            &mut recorder,
            &mut experiment,
            &report,
            true,
            None,
        );
        let summary = campaign.baseline(100 * 60_000).expect("a steady baseline");
        assert!(
            (summary.window_seconds - 300.0).abs() < 1e-9,
            "100 minutes at 5 % is a 5-minute window"
        );
        assert!((summary.mean - 45.0).abs() < 0.1);
        assert!(
            clock.now.get() < 300_000.0,
            "no five minutes of wall time were waited for process time that had passed"
        );
    }

    #[test]
    fn a_drifting_output_is_refused_with_its_rate_and_the_setpoint_step_offer() {
        let loop_definition = tank_loop();
        let clock = FakeClock {
            now: Cell::new(0.0),
        };
        let mut recorder = FakeRecorder {
            clock: &clock,
            plant: Box::new(plant(0.01, 0.0, f64::INFINITY)),
            next: 0.0,
            through: None,
            silent: false,
        };
        let mut experiment = FakeExperiment {
            clock: &clock,
            duration_s: 0.0,
            ended: None,
            calls: Vec::new(),
        };
        let report = FakeReport::default();
        let mut campaign = IdentifyCampaign::new(
            &loop_definition,
            &clock,
            &mut recorder,
            &mut experiment,
            &report,
            true,
            None,
        );
        let refusal = campaign
            .baseline(3_600_000)
            .expect_err("the output keeps drifting");
        assert_eq!(
            whole_text(&refusal),
            "Main.TT101.Value is still changing by 0.6 degC/min. Wait for it to settle, or identify with a setpoint step."
        );
        assert!(
            report.texts.borrow().iter().any(|text| text
                .starts_with("Waiting for Main.TT101.Value to settle (changing 0.6 degC/min)")),
            "the wait reports its progress with the rate"
        );
    }

    #[test]
    fn a_runtime_that_sends_no_samples_stops_the_campaign_after_the_silence_limit() {
        let loop_definition = tank_loop();
        let clock = FakeClock {
            now: Cell::new(0.0),
        };
        let mut recorder = FakeRecorder {
            clock: &clock,
            plant: Box::new(plant(0.0, 0.0, f64::INFINITY)),
            next: 0.0,
            through: None,
            silent: true,
        };
        let mut experiment = FakeExperiment {
            clock: &clock,
            duration_s: 0.0,
            ended: None,
            calls: Vec::new(),
        };
        let report = FakeReport::default();
        let mut campaign = IdentifyCampaign::new(
            &loop_definition,
            &clock,
            &mut recorder,
            &mut experiment,
            &report,
            true,
            None,
        );
        let refusal = campaign
            .baseline(600_000)
            .expect_err("the runtime is silent");
        assert_eq!(whole_text(&refusal), SILENT_RUNTIME_TEXT);
        assert!(
            clock.now.get() > 5_000.0,
            "the five-second limit has passed"
        );
    }

    /// B3.4 step 1, B3.9: a hold on a loop that declares no duration names the field before the runtime is asked. The fake
    /// experiment would complete at once, so a status read first would have given a finished hold instead of this refusal.
    #[test]
    fn a_hold_on_a_loop_without_its_duration_names_the_field_before_the_runtime_is_asked() {
        let mut loop_definition = tank_loop();
        loop_definition.experiment = None;
        let clock = FakeClock {
            now: Cell::new(0.0),
        };
        let mut recorder = FakeRecorder {
            clock: &clock,
            plant: Box::new(plant(0.0, 0.0, f64::INFINITY)),
            next: 0.0,
            through: None,
            silent: false,
        };
        let mut experiment = FakeExperiment {
            clock: &clock,
            duration_s: 0.0,
            ended: None,
            calls: Vec::new(),
        };
        let report = FakeReport::default();
        let mut campaign = IdentifyCampaign::new(
            &loop_definition,
            &clock,
            &mut recorder,
            &mut experiment,
            &report,
            true,
            None,
        );
        let refusal = campaign
            .hold(&setpoint_plan(), &baseline_summary(), 0.0)
            .expect_err("the loop declares no duration");
        assert_eq!(
            whole_text(&refusal),
            "The maximum experiment duration is missing: experiment.durationSeconds must be set, between 1 s and 24 h."
        );
        assert_eq!(
            (refusal.cause, refusal.next_action),
            (Cause::Settings, NextAction::OpenLimits)
        );
    }

    #[test]
    fn a_cancelled_baseline_stops_at_its_next_step_and_a_cancelled_hold_aborts_its_experiment() {
        let loop_definition = tank_loop();
        let clock = FakeClock {
            now: Cell::new(0.0),
        };
        let mut recorder = FakeRecorder {
            clock: &clock,
            plant: Box::new(plant(0.0, 0.0, f64::INFINITY)),
            next: 0.0,
            through: None,
            silent: false,
        };
        let mut experiment = FakeExperiment {
            clock: &clock,
            duration_s: 0.0,
            ended: None,
            calls: Vec::new(),
        };
        let report = FakeReport::default();
        report.cancelled.set(true);
        let mut campaign = IdentifyCampaign::new(
            &loop_definition,
            &clock,
            &mut recorder,
            &mut experiment,
            &report,
            true,
            None,
        );
        let refusal = campaign.baseline(600_000).expect_err("cancelled");
        assert_eq!(whole_text(&refusal), CANCELLED_TEXT);

        let step = setpoint_plan();
        let mut campaign = IdentifyCampaign::new(
            &loop_definition,
            &clock,
            &mut recorder,
            &mut experiment,
            &report,
            true,
            None,
        );
        let status = campaign
            .hold(&step, &baseline_summary(), 0.0)
            .expect("the aborted experiment has ended");
        assert_eq!(status["status"], "aborted");
        assert_eq!(experiment.calls, vec!["abort experiment-1".to_string()]);
    }

    #[test]
    fn a_settled_single_step_is_finished_early_once_and_the_hold_returns_the_terminal_status() {
        let loop_definition = tank_loop();
        let clock = FakeClock {
            now: Cell::new(0.0),
        };
        let mut recorder = FakeRecorder {
            clock: &clock,
            plant: Box::new(plant(0.0, 2.0, 0.0)),
            next: 0.0,
            through: None,
            silent: false,
        };
        let mut experiment = FakeExperiment {
            clock: &clock,
            duration_s: 540.0,
            ended: None,
            calls: Vec::new(),
        };
        let report = FakeReport::default();
        let mut campaign = IdentifyCampaign::new(
            &loop_definition,
            &clock,
            &mut recorder,
            &mut experiment,
            &report,
            true,
            None,
        );
        let status = campaign
            .hold(&setpoint_plan(), &baseline_summary(), 0.0)
            .expect("the hold ends");
        assert_eq!(status["stop_reason"], "finished_early");
        assert_eq!(
            experiment.calls,
            vec!["finish experiment-1".to_string()],
            "finished once"
        );
        assert!(report
            .texts
            .borrow()
            .iter()
            .any(|text| text.starts_with("Holding the step until the response settles:")));
    }

    #[test]
    fn a_response_that_never_settles_runs_to_its_duration_without_an_early_finish() {
        let loop_definition = tank_loop();
        let clock = FakeClock {
            now: Cell::new(0.0),
        };
        let mut recorder = FakeRecorder {
            clock: &clock,
            plant: Box::new(plant(0.0, 2.0, 0.0)),
            next: 0.0,
            through: None,
            silent: false,
        };
        let mut experiment = FakeExperiment {
            clock: &clock,
            duration_s: 60.0,
            ended: None,
            calls: Vec::new(),
        };
        let report = FakeReport::default();
        let mut campaign = IdentifyCampaign::new(
            &loop_definition,
            &clock,
            &mut recorder,
            &mut experiment,
            &report,
            true,
            None,
        );
        let mut multi = setpoint_plan();
        multi
            .trajectory
            .push(serde_json::from_value(json!({ "atMs": 1000, "value": 46 })).expect("a point"));
        let status = campaign
            .hold(&multi, &baseline_summary(), 0.0)
            .expect("the hold ends");
        assert_eq!(status["stop_reason"], "duration_complete");
        assert!(
            experiment.calls.is_empty(),
            "a multi-point plan is never finished early"
        );
    }
}
