//! The two controller laws an Apply runs (specification 35, B6.1 to B6.3, C1, C2 and C13; annex E11.10): the Control.PID run of
//! `pid_commissioning` (a program's `parallel-pid-v1` PID and the shipped Control.PID with its declared states) and the proportional run
//! of `proportional_commissioning`. Every decision is trust-control's. This module holds what `pidCommissioning.ts` and
//! `proportionalCommissioning.ts` kept between the steps of a run (the parameter review, the model anchored to the first reading, the
//! reference and the mode the review read) and hands each step to its law's rule.

use std::collections::BTreeMap;

use trust_control::commissioning::actuator_feedback::transfer_output_basis;
use trust_control::commissioning::adapters::{AdapterKind, ControllerAdapter};
use trust_control::commissioning::commissioning_result::{CommissioningResult, TransferStatus};
use trust_control::commissioning::entry_allowance::{auto_entry_tolerance, transfer_tolerance};
use trust_control::commissioning::found_mode::{found_mode_of, FoundMode};
use trust_control::commissioning::gain_recovery::{GainRecoveryContext, RecoveryOrigin};
use trust_control::commissioning::observation::{commissioning_observation, commissioning_target};
use trust_control::commissioning::observation_assessment::ObservedRecording;
use trust_control::commissioning::parameter_application::ParameterReview;
use trust_control::commissioning::parameter_status::ParameterStatus;
use trust_control::commissioning::pid_commissioning::{
    PidCampaign, PidRunProfile, PidStart, ReviewSetup,
};
use trust_control::commissioning::pid_ending::RunOutcome;
use trust_control::commissioning::proportional_commissioning::{
    aligned_reference, assess_run, auto_entry_step, check_held_values, check_program_unchanged,
    check_proportional_tracking, check_recoverable, check_reviewed_state, check_staged_values,
    check_starting_point, expected_settling_seconds, manual_step, reviewed_target, run_channels,
    run_criteria, settle_run, stages_manual, target_step, Recovery, RunAttempt, RunEnding,
    TransferStep,
};
use trust_control::commissioning::starting_point::{
    anchor_integrating_outputs, compared_plant_inputs, wait_bound_ms, StartingPoint, WaitStep,
};
use trust_control::commissioning::texts;
use trust_control::commissioning::transfer::{
    proportional_parameter, TransferBinding, TransferValue,
};
use trust_control::commissioning::transfer_tracking::{proportional_tracking, TransferTracking};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;
use trust_control::model::GeneralModel;
use trust_control::validation::engineer_verification_plan::EngineerRequirements;

/// Which law a loop's adapter commissions (`controllerAdapter(loop).commission`; C1, C2, C13): a Control.PID run with its structure, or
/// the connected P. Any other law was already refused by the adapter.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)] // One law per run, chosen once: boxing the PID structure would buy nothing.
pub(crate) enum LawKind {
    /// A Control.PID run: the program's parallel PID, or the shipped Control.PID with its declared states.
    Pid(PidRunProfile),
    /// The connected P (C13).
    Proportional,
}

impl LawKind {
    /// The law of `adapter`, with the structure its declared states give at the candidate's `values` (`declaredProfile`, C1).
    ///
    /// # Errors
    /// The adapter's declared states do not fit the loop or the operating point.
    pub(crate) fn of(
        adapter: &ControllerAdapter<'_>,
        model: &GeneralModel,
        values: &BTreeMap<String, f64>,
    ) -> Result<Self, Failure> {
        match adapter.kind() {
            // A program's own PI, PD or PID runs its declared states, as a Control.PID does (C2): its profile is the declared one.
            AdapterKind::ControlPid(_)
            | AdapterKind::ParallelPid
            | AdapterKind::ProjectPid { .. }
            | AdapterKind::Cascade { .. }
            // A PID group runs the declared states of its members, as a program's own law does (C7).
            | AdapterKind::Group { .. }
            | AdapterKind::State(_)
            // A switching controller runs the Control.PID commissioning with its declared states and its switching limits (C3).
            | AdapterKind::Search { .. }
            | AdapterKind::OnOff(_) => Ok(Self::Pid(PidRunProfile::from_adapter(
                adapter.declared_profile(model, values)?,
            ))),
            AdapterKind::ConnectedP => Ok(Self::Proportional),
        }
    }

    /// The transfer binding the run writes through (A6.3): the PID structure's binding, or the connected P's.
    ///
    /// # Errors
    /// The binding does not fit the loop.
    pub(crate) fn binding(
        &self,
        adapter: &ControllerAdapter<'_>,
        loop_definition: &LoopDefinition,
    ) -> Result<TransferBinding, Failure> {
        match self {
            Self::Pid(profile) => profile.binding(loop_definition),
            Self::Proportional => adapter.transfer_binding(),
        }
    }
}

/// What a law begins with: the loop, the study's model and requirements, and the measured output's noise as single executions see it
/// (A6.4).
#[derive(Clone, Copy)]
pub(crate) struct LawSetup<'a> {
    /// The loop being applied.
    pub(crate) loop_definition: &'a LoopDefinition,
    /// The study's model, whose operating point the candidate was validated at (B6.2).
    pub(crate) model: &'a GeneralModel,
    /// The study's requirements.
    pub(crate) task: &'a EngineerRequirements,
    /// The measured output's noise (A6.4); zero without a baseline.
    pub(crate) process_noise: f64,
}

/// A reviewed run before its starting point is reached (`PidCommissioning.review` and `ProportionalCommissioning.review` before their
/// wait; B6.2): the sentence its review shows, and the law's own start.
pub(crate) struct LawStart<'a> {
    description: String,
    task: &'a EngineerRequirements,
    binding: TransferBinding,
    pending: Pending<'a>,
}

#[allow(clippy::large_enum_variant)] // One start per run, built once.
enum Pending<'a> {
    Pid(PidStart<'a>),
    Proportional(ProportionalStart<'a>),
}

/// The proportional run before its starting point is reached (`ProportionalCommissioning.review`).
struct ProportionalStart<'a> {
    setup: LawSetup<'a>,
    model: GeneralModel,
    review: ParameterReview,
    inputs: Vec<String>,
    outputs: Vec<String>,
    found: Option<FoundMode>,
    tolerance: f64,
}

impl<'a> LawStart<'a> {
    /// Begins the review of the run from the first reading of the loop (`review`; B6.2, E6.3): the tracking the runtime must support,
    /// the model anchored to the reading, the plant inputs and outputs the starting point compares, and the sentence the review shows.
    ///
    /// # Errors
    /// The runtime does not support the tracking the law needs, the binding or the model does not fit, the target cannot be built, the
    /// proportional gain cannot align the reference, or the runtime does not keep request identities.
    pub(crate) fn begin(
        kind: LawKind,
        setup: LawSetup<'a>,
        binding: TransferBinding,
        review: ParameterReview,
        first: &TransferStatus,
    ) -> Result<Self, Failure> {
        let measured = [setup
            .loop_definition
            .signals
            .process_variable
            .symbol
            .clone()];
        let model = anchor_integrating_outputs(setup.model, &first.values, &measured);
        match kind {
            LawKind::Pid(profile) => {
                Self::begin_pid(profile, setup, binding, review, first, &model)
            }
            LawKind::Proportional => {
                Self::begin_proportional(setup, binding, review, first, model, measured.to_vec())
            }
        }
    }

    /// The start of a Control.PID run (`PidCommissioning.review`): the layer's own start, and the description at the target of the
    /// model anchored to the first reading.
    fn begin_pid(
        profile: PidRunProfile,
        setup: LawSetup<'a>,
        binding: TransferBinding,
        review: ParameterReview,
        first: &TransferStatus,
        anchored: &GeneralModel,
    ) -> Result<Self, Failure> {
        let loop_definition = setup.loop_definition;
        let label = profile.label().to_owned();
        let start = PidStart::begin(
            ReviewSetup {
                loop_definition,
                model: setup.model,
                task: setup.task,
                profile,
                review,
                reference_hold: None,
                process_noise: Some(setup.process_noise),
            },
            first,
        )?;
        start.check_recoverable()?;
        let target = commissioning_target(loop_definition, anchored, setup.task, &binding)?
            .ok_or_else(binding_unconfigured)?;
        Ok(Self {
            description: texts::pid_commission_description(
                &label,
                target,
                &loop_definition.signals.process_variable.unit,
                setup.task.duration_seconds,
            ),
            task: setup.task,
            binding,
            pending: Pending::Pid(start),
        })
    }

    /// The start of a proportional run (`ProportionalCommissioning.review`): the tracking it needs, the plant input it compares in the
    /// mode it found, the allowance of that input, a gain that can align the reference, and the description.
    fn begin_proportional(
        setup: LawSetup<'a>,
        binding: TransferBinding,
        review: ParameterReview,
        first: &TransferStatus,
        model: GeneralModel,
        outputs: Vec<String>,
    ) -> Result<Self, Failure> {
        let loop_definition = setup.loop_definition;
        let signals = &loop_definition.signals;
        check_proportional_tracking(first.supported_tracking.as_deref())?;
        let input = transfer_output_basis(&binding, &binding.output)
            .map(|member| member.symbol.clone())
            .ok_or_else(binding_unconfigured)?;
        let found = found_mode_of(
            boolean_at(&first.values, &binding.mode.symbol),
            binding.mode.manual_value,
        );
        let tolerance = auto_entry_tolerance(
            loop_definition,
            setup.task,
            &review.values,
            Some(setup.process_noise),
            None,
        )?;
        aligned_reference(
            loop_definition,
            &binding,
            &first.values,
            gain_of(&setup, &review)?,
        )?;
        check_recoverable(
            Some(review.transaction_id.as_str()),
            review.observed.supports_request_id,
        )?;
        let description = texts::prop_commission_description(
            reviewed_target(&binding, &model, setup.task)?,
            &signals.process_variable.unit,
            setup.task.duration_seconds,
            transfer_tolerance(loop_definition, setup.task.transfer_tolerance)?,
            &signals.manipulated_variable.unit,
        );
        Ok(Self {
            description,
            task: setup.task,
            binding,
            pending: Pending::Proportional(ProportionalStart {
                setup,
                model,
                review,
                inputs: vec![input],
                outputs,
                found,
                tolerance,
            }),
        })
    }

    /// The sentence the review shows (`description`; B6.2, B6.3): the target the run applies and the duration it observes.
    #[must_use]
    pub(crate) fn description(&self) -> &str {
        &self.description
    }

    /// The transfer binding the run writes through.
    #[must_use]
    pub(crate) fn binding(&self) -> &TransferBinding {
        &self.binding
    }

    /// One step of the wait for the starting point (`awaitCommissioningStartingPoint`, B6.2): the reading `observed` against the
    /// validated point, with `process_ms` of process time waited so far, the bound of `wait_bound_ms` and the start of the settle dwell
    /// that the step before returned (`inside_since_ms`).
    ///
    /// # Errors
    /// A plant input is away from its point, an output has not settled within the bound, or a value is not a finite number.
    pub(crate) fn wait_step(
        &self,
        observed: &TransferStatus,
        process_ms: f64,
        inside_since_ms: Option<f64>,
    ) -> Result<WaitStep, Failure> {
        let bound_ms = wait_bound_ms(self.task);
        match &self.pending {
            Pending::Pid(start) => start.starting_point(&observed.values)?.wait_step(
                &observed.values,
                process_ms,
                bound_ms,
                inside_since_ms,
            ),
            Pending::Proportional(start) => StartingPoint {
                loop_definition: start.setup.loop_definition,
                model: &start.model,
                task: start.setup.task,
                input_symbols: compared_plant_inputs(&start.inputs, start.found),
                output_symbols: &start.outputs,
                input_tolerance: start.tolerance,
            }
            .wait_step(&observed.values, process_ms, bound_ms, inside_since_ms),
        }
    }

    /// Accepts the starting point the run reached (`review` after its wait; B6.2, B6.7): the program is the reviewed one, the reference
    /// and the mode are read from `settled`, and the starting point holds.
    ///
    /// # Errors
    /// The program changed, a reference is missing, the starting point does not hold, the gain cannot align the reference, or the runtime
    /// does not keep request identities.
    pub(crate) fn accept(self, settled: &TransferStatus) -> Result<Law<'a>, Failure> {
        match self.pending {
            Pending::Pid(start) => Ok(Law::Pid(start.accept(settled)?)),
            Pending::Proportional(start) => {
                let ProportionalStart {
                    setup,
                    model,
                    review,
                    tolerance,
                    ..
                } = start;
                check_program_unchanged(
                    &review.observed.program_revision,
                    &settled.program_revision,
                )?;
                let reference = self
                    .binding
                    .states
                    .first()
                    .ok_or_else(binding_unconfigured)?;
                let previous_reference = number_at(&settled.values, &reference.symbol)?;
                let reviewed_mode = boolean_at(&settled.values, &self.binding.mode.symbol);
                check_starting_point(
                    setup.loop_definition,
                    &model,
                    setup.task,
                    &self.binding,
                    found_mode_of(reviewed_mode, self.binding.mode.manual_value),
                    &settled.values,
                    tolerance,
                )?;
                aligned_reference(
                    setup.loop_definition,
                    &self.binding,
                    &settled.values,
                    gain_of(&setup, &review)?,
                )?;
                check_recoverable(
                    Some(review.transaction_id.as_str()),
                    review.observed.supports_request_id,
                )?;
                Ok(Law::Proportional(ProportionalRun {
                    setup,
                    model,
                    binding: self.binding,
                    review,
                    tolerance,
                    previous_reference,
                    reviewed_mode,
                }))
            }
        }
    }
}

/// A reviewed run whose starting point is reached (`PidCommissioning` or `ProportionalCommissioning` after their review; B6, C1, C13).
#[allow(clippy::large_enum_variant)] // One run per Apply, built once.
pub(crate) enum Law<'a> {
    /// A Control.PID run.
    Pid(PidCampaign<'a>),
    /// A proportional run.
    Proportional(ProportionalRun<'a>),
}

/// A proportional run after its review (`ProportionalCommissioning`): the review, the model anchored to its first reading, the reference
/// and the mode the review read, and the allowance of its starting point.
pub(crate) struct ProportionalRun<'a> {
    setup: LawSetup<'a>,
    model: GeneralModel,
    binding: TransferBinding,
    review: ParameterReview,
    tolerance: f64,
    previous_reference: f64,
    reviewed_mode: Option<bool>,
}

impl ProportionalRun<'_> {
    fn found_mode(&self) -> Option<FoundMode> {
        found_mode_of(self.reviewed_mode, self.binding.mode.manual_value)
    }
}

impl Law<'_> {
    /// The transfer binding the run writes through.
    #[must_use]
    pub(crate) fn binding(&self) -> &TransferBinding {
        match self {
            Self::Pid(campaign) => campaign.binding(),
            Self::Proportional(run) => &run.binding,
        }
    }

    /// The parameter review the run applies (E7.3).
    #[must_use]
    pub(crate) fn review(&self) -> &ParameterReview {
        match self {
            Self::Pid(campaign) => campaign.review(),
            Self::Proportional(run) => &run.review,
        }
    }

    /// The model the run is judged on, anchored to its first reading (B6.2).
    #[must_use]
    pub(crate) fn model(&self) -> &GeneralModel {
        match self {
            Self::Pid(campaign) => campaign.model(),
            Self::Proportional(run) => &run.model,
        }
    }

    /// The mode the loop was found in (A6.2).
    #[must_use]
    pub(crate) fn found_mode(&self) -> Option<FoundMode> {
        match self {
            Self::Pid(campaign) => campaign.found_mode(),
            Self::Proportional(run) => run.found_mode(),
        }
    }

    /// The reference of each controlled channel as the review read it, which a Restore writes back (B6.4).
    #[must_use]
    pub(crate) fn references(&self) -> BTreeMap<String, f64> {
        match self {
            Self::Pid(campaign) => campaign.references().clone(),
            Self::Proportional(run) => run
                .binding
                .states
                .first()
                .map(|reference| (reference.symbol.clone(), run.previous_reference))
                .into_iter()
                .collect(),
        }
    }

    /// The tracking that a recovery to Manual holds (`recoveryTracking`, A6.2).
    ///
    /// # Errors
    /// The structure's tracking does not fit the loop.
    pub(crate) fn recovery_tracking(&self) -> Result<TransferTracking, Failure> {
        match self {
            Self::Pid(campaign) => {
                campaign
                    .profile()
                    .tracking(campaign.loop_definition(), campaign.binding(), false)
            }
            Self::Proportional(run) => {
                proportional_tracking(run.setup.loop_definition, &run.binding, false)
                    .map(TransferTracking::Proportional)
            }
        }
    }

    /// The transfer allowance a recovery's transfers use (`transferTolerance`, A6.4, E6.9).
    ///
    /// # Errors
    /// The reviewed tolerance is out of range, or an output limit is not set.
    pub(crate) fn tolerance(&self) -> Result<f64, Failure> {
        let (loop_definition, task) = self.loop_and_task();
        transfer_tolerance(loop_definition, task.transfer_tolerance)
    }

    /// Checks the reading before the run writes anything (`execute`; B6.2, B6.7): the reviewed record, mode and reference still hold,
    /// and for a Control.PID run the starting point too.
    ///
    /// # Errors
    /// The starting point does not hold, or the record, the mode or the reference changed since the review.
    pub(crate) fn check_before_manual(
        &self,
        boundary: &TransferStatus,
        parameters: &ParameterStatus,
    ) -> Result<(), Failure> {
        match self {
            Self::Pid(campaign) => {
                campaign.check_starting_point(boundary)?;
                campaign.check_reviewed_held(parameters, &boundary.values)
            }
            Self::Proportional(run) => check_reviewed_state(
                &run.review.observed,
                parameters,
                run.reviewed_mode,
                &run.binding,
                &boundary.values,
                run.previous_reference,
            ),
        }
    }

    /// Whether the loop is not in Manual at `boundary`, so the run stages Manual before it writes the record (B6.2).
    #[must_use]
    pub(crate) fn stages_manual(&self, boundary: &TransferStatus) -> bool {
        stages_manual(self.binding(), &boundary.values)
    }

    /// The Manual staging of a loop found out of Manual (`execute`, B6.2).
    ///
    /// # Errors
    /// A state is missing from the reading, or the tracking does not fit.
    pub(crate) fn manual_step(&self, boundary: &TransferStatus) -> Result<TransferStep, Failure> {
        match self {
            Self::Pid(campaign) => campaign.manual_step(boundary),
            Self::Proportional(run) => manual_step(
                run.setup.loop_definition,
                &run.binding,
                run.setup.task,
                &boundary.values,
                run.previous_reference,
            ),
        }
    }

    /// Checks the reading after the record is staged and before the Auto entry (`execute`; B6.2, A6.8): the starting point holds and the
    /// staged parameters hold their reviewed values.
    ///
    /// # Errors
    /// The starting point does not hold, or the program changed a staged parameter.
    pub(crate) fn check_before_auto(&self, observed: &TransferStatus) -> Result<(), Failure> {
        match self {
            Self::Pid(campaign) => {
                campaign.check_starting_point(observed)?;
                campaign.check_staged(observed)
            }
            Self::Proportional(run) => {
                check_starting_point(
                    run.setup.loop_definition,
                    &run.model,
                    run.setup.task,
                    &run.binding,
                    run.found_mode(),
                    &observed.values,
                    run.tolerance,
                )?;
                check_staged_values(&run.review.values, &observed.values)
            }
        }
    }

    /// The Auto entry (`execute`; A6.3, A6.4, C13.3): the states, the allowance of the first output and the aligning tracking.
    ///
    /// # Errors
    /// A state is missing, the allowance cannot be built, or the gain cannot align the reference.
    pub(crate) fn auto_entry_step(
        &self,
        observed: &TransferStatus,
    ) -> Result<TransferStep, Failure> {
        match self {
            Self::Pid(campaign) => campaign.auto_entry_step(observed),
            Self::Proportional(run) => auto_entry_step(
                run.setup.loop_definition,
                &run.binding,
                run.setup.task,
                &run.review.values,
                Some(run.setup.process_noise),
                &observed.values,
            ),
        }
    }

    /// The status a transfer of the run is reviewed on, when the law names one (`execute`): a Control.PID run names the reading it built
    /// the transfer from; a proportional run lets the transfer read its own, except for its Manual staging (`proportionalCommissioning.ts`).
    #[must_use]
    pub(crate) fn reviewed_for(&self, observed: &TransferStatus) -> Option<TransferStatus> {
        match self {
            Self::Pid(_) => Some(observed.clone()),
            Self::Proportional(_) => None,
        }
    }

    /// The reviewed target, rounded as the reference holds it (B6.2).
    ///
    /// # Errors
    /// The target does not fit the reference's type and bounds, or the binding has no reference.
    pub(crate) fn target(&self) -> Result<f64, Failure> {
        match self {
            Self::Pid(campaign) => campaign.target(),
            Self::Proportional(run) => reviewed_target(&run.binding, &run.model, run.setup.task),
        }
    }

    /// The step to the reviewed target while the loop is in Auto (`applyTarget`; B6.3, C13.6, E11.10 step 7).
    ///
    /// # Errors
    /// A state is missing, the target cannot be built, or the binding has no output allowance.
    pub(crate) fn target_step(&self, entered: &TransferStatus) -> Result<TransferStep, Failure> {
        match self {
            Self::Pid(campaign) => campaign.target_step(entered),
            Self::Proportional(run) => target_step(
                run.setup.loop_definition,
                &run.binding,
                &entered.values,
                self.target()?,
            ),
        }
    }

    /// The reviewed duration of the observation, in ms of process time (B6.3).
    #[must_use]
    pub(crate) fn duration_ms(&self) -> f64 {
        self.loop_and_task().1.duration_seconds * 1000.0
    }

    /// The settling time the record period of the observation follows, in s (A4.2); absent for an ON/OFF output.
    #[must_use]
    pub(crate) fn expected_settling_seconds(&self) -> Option<f64> {
        let (loop_definition, task) = self.loop_and_task();
        expected_settling_seconds(loop_definition, task)
    }

    /// Judges the recorded response (`assessObservation`, B6.3).
    ///
    /// # Errors
    /// The criteria or the observation cannot be built.
    pub(crate) fn assess(&self, recording: &ObservedRecording) -> Result<RunAttempt, Failure> {
        match self {
            Self::Pid(campaign) => campaign.assess(recording),
            Self::Proportional(run) => Ok(assess_run(
                recording,
                &run_criteria(
                    run.setup.loop_definition,
                    &run.binding,
                    run.setup.task,
                    self.target()?,
                ),
                &commissioning_observation(
                    run.setup.loop_definition,
                    &run.model,
                    run.setup.task,
                    &run.review.values,
                )?,
            )),
        }
    }

    /// Checks that the record still holds the reviewed program and values after the observation (`execute`; A6.4, A6.8).
    ///
    /// # Errors
    /// The program changed, or a parameter does not hold its reviewed value.
    pub(crate) fn held_after(&self, parameters: &ParameterStatus) -> Result<(), Failure> {
        match self {
            Self::Pid(campaign) => campaign.held_after(parameters),
            Self::Proportional(run) => check_held_values(
                &run.review.observed.program_revision,
                &run.review.values,
                parameters,
            ),
        }
    }

    /// Ends the run (`execute`; B6.3, B6.7, A6.3, A8.5): the status, the reason, the failure, the recovery and the journal stage.
    #[must_use]
    pub(crate) fn settle(
        &self,
        attempt: RunAttempt,
        cancelled: bool,
        recovery: Recovery,
        recording_stop: Option<&str>,
    ) -> Option<RunEnding> {
        match self {
            Self::Pid(campaign) => campaign.settle(attempt, cancelled, recovery, recording_stop),
            Self::Proportional(run) => settle_run(
                attempt,
                cancelled,
                recovery,
                run.found_mode() == Some(FoundMode::Auto),
                &run.setup.loop_definition.signals.process_variable.unit,
                recording_stop,
            ),
        }
    }

    /// The result the run records (`execute`'s return; B6.3, B6.7, C1, C13.8).
    ///
    /// # Errors
    /// The target, the criteria or the channels cannot be built.
    pub(crate) fn result(
        &self,
        ending: RunEnding,
        outcome: RunOutcome,
    ) -> Result<CommissioningResult, Failure> {
        match self {
            Self::Pid(campaign) => campaign.result(ending, outcome),
            Self::Proportional(run) => {
                let target = self.target()?;
                let loop_definition = run.setup.loop_definition;
                Ok(CommissioningResult {
                    status: ending.status,
                    reason: ending.reason,
                    recording: outcome.recording,
                    transfers: outcome.transfers,
                    recovery_confirmed: ending.recovery_confirmed,
                    maximum_error: outcome.maximum_error,
                    target,
                    parameters: outcome.parameters,
                    model_revision: run.model.identity().revision.clone(),
                    channels: run_channels(loop_definition)?,
                    criteria: run_criteria(loop_definition, &run.binding, run.setup.task, target),
                    gain_recovery: Some(GainRecoveryContext {
                        origin: RecoveryOrigin::of(&run.review.observed),
                        candidate: run.review.values.clone(),
                    }),
                    failure: ending.failure,
                    found_mode: run.found_mode(),
                })
            }
        }
    }

    /// The loop and the requirements of the run.
    fn loop_and_task(&self) -> (&LoopDefinition, &EngineerRequirements) {
        match self {
            Self::Pid(campaign) => (campaign.loop_definition(), campaign.task()),
            Self::Proportional(run) => (run.setup.loop_definition, run.setup.task),
        }
    }
}

/// The reviewed target of a journaled checkpoint's run, as its law rounds it (B6.2, B6.3): a Control.PID run's target, or the connected
/// P's. A reattached observation is judged against it (A8.6).
///
/// # Errors
/// The loop or the model does not give a target, or the target does not fit the reference.
pub(crate) fn law_target(
    adapter: &ControllerAdapter<'_>,
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    binding: &TransferBinding,
) -> Result<f64, Failure> {
    match adapter.kind() {
        AdapterKind::ControlPid(_)
        | AdapterKind::ParallelPid
        | AdapterKind::ProjectPid { .. }
        | AdapterKind::Cascade { .. }
        | AdapterKind::Group { .. }
        | AdapterKind::State(_)
        | AdapterKind::Search { .. }
        | AdapterKind::OnOff(_) => commissioning_target(loop_definition, model, task, binding)?
            .ok_or_else(binding_unconfigured),
        AdapterKind::ConnectedP => reviewed_target(binding, model, task),
    }
}

/// The proportional gain of the candidate (`parameterReview.values[proportionalParameter(loop).symbol]`): NaN when the candidate has
/// none, so that the alignment refuses it.
fn gain_of(setup: &LawSetup<'_>, review: &ParameterReview) -> Result<f64, Failure> {
    let symbol = &proportional_parameter(setup.loop_definition)?.symbol;
    Ok(review.values.get(symbol).copied().unwrap_or(f64::NAN))
}

/// The refusal of a binding without the reference, the Manual command or the output that a run writes (`transferConfigure`, A6.3).
fn binding_unconfigured() -> Failure {
    Failure::new(
        &texts::transfer_configure_binding(),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    )
}

/// The BOOL a reading holds for `symbol`, such as a mode, when it holds one.
fn boolean_at(values: &BTreeMap<String, TransferValue>, symbol: &str) -> Option<bool> {
    match values.get(symbol) {
        Some(TransferValue::Boolean(flag)) => Some(*flag),
        _ => None,
    }
}

/// A finite number the runtime reports for `symbol` (E6.3).
fn number_at(values: &BTreeMap<String, TransferValue>, symbol: &str) -> Result<f64, Failure> {
    values
        .get(symbol)
        .copied()
        .and_then(TransferValue::as_number)
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            Failure::new(
                &texts::start_value_missing(symbol),
                Cause::Runtime,
                NextAction::None,
            )
        })
}
