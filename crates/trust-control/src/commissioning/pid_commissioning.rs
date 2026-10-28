//! The rules of a Control.PID commissioning run (`pidCommissioning.ts`; specification 35, B6 and C1): the review of a loop, the checks at
//! each task boundary, the transfers the run writes (the Manual staging, the Auto entry and the target change), and the response it
//! requests and judges. How the run ends and the result it records are in `pid_ending`; the recovery that a run ends with is in
//! `pid_recovery`.
//!
//! One run serves two controller structures, held in [`PidRunProfile`]: a program's `parallel-pid-v1` PID (`pidProfile`, labelled
//! `PID`, C2) and the shipped `Control.PID` with its declared `control-pid-v1` states (C1, labelled `Control.PID`). The adapter of a
//! loop gives the profile, and every rule dispatches on it.
//!
//! The caller performs the runtime's work and passes each reading here, in the order the run's methods state: the starting-point
//! checks, the Manual staging, the parameter apply, the Auto entry, the target change under the recording, and the judgement of the
//! response. The run writes nothing and holds no runtime state. Its flags (cancelled, observing, awaiting Restore, recovery confirmed)
//! belong to the caller, who passes them in.
//!
//! Left out, as transport: the runtime connection and its live-write precondition (`requireLiveWrite`), the transfer and parameter
//! clients and every read and write they make, the wait loop of the starting point (its decision is `StartingPoint::wait_step`), the
//! recording session and its persistence, the follow loop and the target change of the observation (`observeTargetChange`), the
//! commissioning journal and its checkpoint writer, the promises and the running and finished bookkeeping, the clocks
//! (`runtimeClockNs`), the progress callback, and the panel's detach flag. [`close_while_observing`] decides what a close does.
//!
//! Left out, as parked features: the ON/OFF switching criteria (C3; the adapter refuses an ON/OFF controller, so no switching limit is
//! set), the cascade's reference hold (C6; the caller passes `reference_hold`, `None` for a loop without a cascade), and multivariable
//! channels (V7; the connection refuses them).

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::model::GeneralModel;
use crate::validation::commissioning_assessment::CommissioningCriteria;
use crate::validation::engineer_verification_plan::EngineerRequirements;
use crate::validation::model_route::bound_channels;

use super::actuator_feedback::{transfer_output_basis, transfer_target_allowance};
use super::channel_mapping::connected_model_channels;
use super::commissioning_result::{CommissioningChannel, TransferStatus};
use super::entry_allowance::{
    auto_entry_allowance, auto_entry_error, auto_entry_error_gain, transfer_tolerance,
};
use super::found_mode::{found_mode_of, FoundMode};
use super::observation::{
    commissioning_observation, commissioning_reference_targets, commissioning_target,
    CommissioningObservation, ResponseSample,
};
use super::observation_assessment::ObservedRecording;
use super::parameter_application::ParameterReview;
use super::parameter_status::{same_parameter_values, ParameterStatus};
use super::profile::{CommissioningProfile, RequiredTracking};
use super::proportional_commissioning::{
    assess_run, binding_failure, check_held_values, check_recoverable, check_staged_values,
    expected_settling_seconds, run_channels, run_criteria, RunAttempt, TransferStep,
};
use super::starting_point::{
    anchor_integrating_outputs, assert_reference_held, compared_plant_inputs,
    with_reference_hold_guard, ReferenceHold, StartingPoint,
};
use super::texts;
use super::transfer::{pid_transfer_binding, TransferBinding, TransferValue};
use super::transfer_tracking::{parallel_pid_tracking, pid_transfer_states, TransferTracking};

/// The label of the program's parallel PID structure (`pidProfile.label`).
const PARALLEL_LABEL: &str = "PID";
/// The tracking kind of the parallel PID structure (`pidProfile.trackingKind`, E6.4).
const PARALLEL_TRACKING: &str = "parallel-pid-v1";

/// The controller structure a Control.PID run commissions (`PidCommissioning`'s profile). The adapter of a loop gives its declared
/// profile when it declares states (C1); otherwise the run is the program's parallel PID (C2).
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)] // One profile per run, built once: boxing it would buy nothing.
pub enum PidRunProfile {
    /// The program's `parallel-pid-v1` PID (`pidProfile`, C2): its integral and derivative states are read at the boundary.
    ParallelPid,
    /// A declared-state structure: the shipped `Control.PID` with its `control-pid-v1` states (C1, C0).
    Declared(CommissioningProfile),
}

impl PidRunProfile {
    /// The profile of a loop's adapter: its declared profile, or the parallel PID when the adapter declares none (`declaredProfile`;
    /// `PidCommissioning.review` defaults to the parallel PID).
    #[must_use]
    pub fn from_adapter(declared: Option<CommissioningProfile>) -> Self {
        declared.map_or(Self::ParallelPid, Self::Declared)
    }

    /// The label the structure shows (`profile.label`): `PID` for the parallel PID, or the declared structure's own label.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::ParallelPid => PARALLEL_LABEL,
            Self::Declared(profile) => profile.label.as_str(),
        }
    }

    /// The tracking kind the transfers name (`trackingKind`, E6.4).
    #[must_use]
    pub fn tracking_kind(&self) -> &'static str {
        match self {
            Self::ParallelPid => PARALLEL_TRACKING,
            Self::Declared(profile) => profile.tracking_kind(),
        }
    }

    /// The tracking features the runtime must support beyond the kind (`requiredTracking`, E6.3).
    #[must_use]
    pub fn required_tracking(&self) -> Vec<&'static str> {
        match self {
            Self::ParallelPid => Vec::new(),
            Self::Declared(profile) => profile
                .required_tracking
                .iter()
                .copied()
                .map(RequiredTracking::word)
                .collect(),
        }
    }

    /// The transfer binding of the structure (`binding`, A6.3): the parallel PID's binding, or the declared structure's.
    ///
    /// # Errors
    /// The structure's binding does not fit the loop (see `pid_transfer_binding`).
    pub fn binding(&self, loop_definition: &LoopDefinition) -> Result<TransferBinding, Failure> {
        match self {
            Self::ParallelPid => pid_transfer_binding(loop_definition),
            Self::Declared(profile) => profile.binding(),
        }
    }

    /// The states a transfer writes, rounded as their members are typed, from the values the runtime reports (`states`, E6.3).
    ///
    /// # Errors
    /// A state has no finite value in `observed`.
    pub fn states(
        &self,
        binding: &TransferBinding,
        observed: &BTreeMap<String, TransferValue>,
    ) -> Result<BTreeMap<String, f64>, Failure> {
        match self {
            Self::ParallelPid => pid_transfer_states(binding, observed),
            Self::Declared(profile) => profile.states(binding, observed),
        }
    }

    /// The tracking of a transfer (`tracking`, E6.4): `align_output` is true for an Auto entry, which aligns the output; false for a
    /// Manual staging or a target change, which keeps the retained states.
    ///
    /// # Errors
    /// The structure's tracking does not fit the loop and its binding.
    pub fn tracking(
        &self,
        loop_definition: &LoopDefinition,
        binding: &TransferBinding,
        align_output: bool,
    ) -> Result<TransferTracking, Failure> {
        match self {
            Self::ParallelPid => parallel_pid_tracking(loop_definition, binding, align_output)
                .map(TransferTracking::ParallelPid),
            Self::Declared(profile) => profile.tracking(binding, align_output),
        }
    }

    /// The reference symbols the review keeps and a Restore writes back (`referenceSymbols`): the declared structure's references, or
    /// the parallel PID's one reference.
    #[must_use]
    pub fn references(&self, binding: &TransferBinding) -> Vec<String> {
        match self {
            Self::ParallelPid => binding
                .states
                .first()
                .map(|reference| reference.symbol.clone())
                .into_iter()
                .collect(),
            Self::Declared(profile) => profile.references.clone(),
        }
    }

    /// The reference targets the structure sets when the loop is in Auto (`targets`, B6.2): `None` for the parallel PID, whose one
    /// reference takes the reviewed target. A declared structure takes its targets from the model the run is judged on, which the run
    /// anchors to its first reading, as the `TypeScript` passes `this.model`.
    ///
    /// # Errors
    /// The declared structure's reference targets cannot be built from the model and the requirements.
    pub fn targets(
        &self,
        loop_definition: &LoopDefinition,
        model: &GeneralModel,
        task: &EngineerRequirements,
    ) -> Result<Option<BTreeMap<String, f64>>, Failure> {
        match self {
            Self::ParallelPid => Ok(None),
            Self::Declared(_) => {
                commissioning_reference_targets(loop_definition, model, task).map(Some)
            }
        }
    }
}

/// What a review of a Control.PID run is made with (`PidCommissioning.review`'s arguments; B6.2): the loop and its model, the
/// requirements, the controller structure, the parameter review, the reference hold of an outer controller (C6), and the measured
/// output's noise as single executions see it (A6.4).
pub struct ReviewSetup<'a> {
    /// The loop being commissioned.
    pub loop_definition: &'a LoopDefinition,
    /// The model whose validated operating point the run starts from.
    pub model: &'a GeneralModel,
    /// The requirements of the study.
    pub task: &'a EngineerRequirements,
    /// The controller structure the run commissions.
    pub profile: PidRunProfile,
    /// The parameter review the candidate was made in.
    pub review: ParameterReview,
    /// The reference hold of an outer controller (C6); `None` for a loop without a cascade.
    pub reference_hold: Option<&'a ReferenceHold>,
    /// The measured output's noise as single executions see it (A6.4); `None` when no baseline was measured.
    pub process_noise: Option<f64>,
}

/// A reviewed run before its starting point is reached (`PidCommissioning.review`, its first reading; B6.2). It holds the model
/// anchored to the first reading, the transfer binding with its reference hold guard, the plant inputs and measured outputs that the
/// starting point compares, and the parameter review. [`PidStart::accept`] turns it into a [`PidCampaign`].
pub struct PidStart<'a> {
    pub(super) loop_definition: &'a LoopDefinition,
    pub(super) model: GeneralModel,
    pub(super) task: &'a EngineerRequirements,
    pub(super) profile: PidRunProfile,
    pub(super) binding: TransferBinding,
    pub(super) review: ParameterReview,
    pub(super) process_noise: Option<f64>,
    pub(super) inputs: Vec<String>,
    pub(super) outputs: Vec<String>,
}

impl<'a> PidStart<'a> {
    /// Begins the review of a Control.PID run from the first reading of the loop (`PidCommissioning.review`; B6.2, B6.7): the transfer
    /// binding with its reference hold guard, the tracking the runtime must support, the plant inputs and measured outputs that the
    /// starting point compares, and the model anchored to the reading.
    ///
    /// # Errors
    /// The runtime does not support the tracking the structure needs, the reference hold is not held, the transfer binding does not fit
    /// the loop, or the model does not connect one input and one output.
    pub fn begin(setup: ReviewSetup<'a>, first: &TransferStatus) -> Result<Self, Failure> {
        let binding = with_reference_hold_guard(
            setup.profile.binding(setup.loop_definition)?,
            setup.reference_hold,
        );
        check_tracking(&setup.profile, first.supported_tracking.as_deref())?;
        assert_reference_held(setup.reference_hold, &first.values)?;
        let (inputs, outputs) =
            starting_point_symbols(setup.loop_definition, setup.model, &binding)?;
        let model = anchor_integrating_outputs(setup.model, &first.values, &outputs);
        Ok(Self {
            loop_definition: setup.loop_definition,
            model,
            task: setup.task,
            profile: setup.profile,
            binding,
            review: setup.review,
            process_noise: setup.process_noise,
            inputs,
            outputs,
        })
    }

    /// The start of a saved campaign's recovery (`reviewRecovery`; B6.4): the profile's binding, without a reference hold and without a
    /// starting point, which a recovery does not compare. The model is taken as it is, since a recovery does not anchor it. The noise of
    /// the original review is not stored, so the return to Auto carries no noise term, as the `TypeScript` passes none.
    ///
    /// # Errors
    /// The transfer binding does not fit the loop.
    pub(super) fn recovery(
        loop_definition: &'a LoopDefinition,
        model: &GeneralModel,
        task: &'a EngineerRequirements,
        profile: PidRunProfile,
        review: ParameterReview,
    ) -> Result<Self, Failure> {
        let binding = profile.binding(loop_definition)?;
        Ok(Self {
            loop_definition,
            model: model.clone(),
            task,
            profile,
            binding,
            review,
            process_noise: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
        })
    }

    /// The starting point of one reading (`awaitCommissioningStartingPoint`'s arguments; B6.2, A6.4): the plant inputs the reading's found
    /// mode compares, the measured outputs that must settle, and the allowance of the inputs at the reading's entry error. The wait reads
    /// the runtime again against it, through [`StartingPoint::wait_step`].
    ///
    /// # Errors
    /// The allowance cannot be built from the candidate and the requirements.
    pub fn starting_point(
        &self,
        values: &BTreeMap<String, TransferValue>,
    ) -> Result<StartingPoint<'_>, Failure> {
        let found = found_mode_of(
            boolean_at(values, &self.binding.mode.symbol),
            self.binding.mode.manual_value,
        );
        Ok(StartingPoint {
            loop_definition: self.loop_definition,
            model: &self.model,
            task: self.task,
            input_symbols: compared_plant_inputs(&self.inputs, found),
            output_symbols: &self.outputs,
            input_tolerance: self.allowance(values)?,
        })
    }

    /// Whether the runtime keeps what the run needs to recover after a reload (`setCheckpointWriter`; E7.2).
    ///
    /// # Errors
    /// The review has no transaction identifier, or the runtime keeps no request identities.
    pub fn check_recoverable(&self) -> Result<(), Failure> {
        check_recoverable(
            Some(self.review.transaction_id.as_str()),
            self.review.observed.supports_request_id,
        )
    }

    /// Accepts the starting point the run reached (`review`, `checkStartingPoint`; B6.2, B6.7): the program is the one the review read,
    /// the references and the mode are read from the accepted values, and the starting point holds.
    ///
    /// # Errors
    /// The program changed since the review, a reference is not a finite number, or the starting point does not hold.
    pub fn accept(self, observed: &TransferStatus) -> Result<PidCampaign<'a>, Failure> {
        if observed.program_revision != self.review.observed.program_revision {
            return Err(Failure::new(
                &texts::pid_commission_program_changed(),
                Cause::Runtime,
                NextAction::Validate,
            ));
        }
        let first_state = self.binding.states.first().ok_or_else(binding_failure)?;
        let previous_reference = number_at(&observed.values, &first_state.symbol)?;
        let mut references = BTreeMap::new();
        for symbol in self.profile.references(&self.binding) {
            let value = number_at(&observed.values, &symbol)?;
            references.insert(symbol, value);
        }
        let reviewed_mode = boolean_at(&observed.values, &self.binding.mode.symbol);
        let campaign = PidCampaign {
            start: self,
            references,
            previous_reference,
            reviewed_mode,
            recovery_values: None,
        };
        campaign.check_starting_point(observed)?;
        Ok(campaign)
    }

    /// The Auto-entry allowance of the candidate at the entry error of these values (`autoEntryTolerance`; A6.4).
    fn allowance(&self, values: &BTreeMap<String, TransferValue>) -> Result<f64, Failure> {
        auto_entry_allowance(
            self.loop_definition,
            &self.model,
            self.task,
            &self.review.values,
            self.process_noise,
            auto_entry_error(self.loop_definition, values),
        )
    }

    /// The Auto-entry allowance of the candidate that the transfer sends (A6.4, annex E6.9): the integral step on the error the
    /// loop enters with is the runtime's to add, with [`Self::entry_error_gain`].
    fn entry_allowance(&self) -> Result<f64, Failure> {
        auto_entry_allowance(
            self.loop_definition,
            &self.model,
            self.task,
            &self.review.values,
            self.process_noise,
            None,
        )
    }

    /// The factor of the error at entry that the runtime adds to [`Self::entry_allowance`] (annex E6.9).
    fn entry_error_gain(&self) -> Option<f64> {
        auto_entry_error_gain(
            self.loop_definition,
            self.task,
            &self.review.values,
            self.process_noise,
        )
    }
}

/// A reviewed Control.PID run whose starting point is reached (`PidCommissioning`, after its review; B6, C1). Its methods are the rules
/// that decide each step of the run, the response it judges and the result it records. It holds no runtime state: the caller performs
/// each transfer, parameter operation and recording, and passes each reading here.
pub struct PidCampaign<'a> {
    pub(super) start: PidStart<'a>,
    /// The reference of each controlled channel as the review read it, which a Restore writes back (`previousReferences`, B6.4).
    pub(super) references: BTreeMap<String, f64>,
    /// The first reference as the review read it (`previousReference`).
    pub(super) previous_reference: f64,
    /// The mode's BOOL value the review read (`reviewedMode`); `None` when the review did not read the mode.
    pub(super) reviewed_mode: Option<bool>,
    /// The values a Restore writes back, when the campaign recovers a saved one (`recoveryValues`, B6.4); `None` for a fresh run.
    pub(super) recovery_values: Option<BTreeMap<String, f64>>,
}

impl PidCampaign<'_> {
    /// The loop the run commissions.
    #[must_use]
    pub fn loop_definition(&self) -> &LoopDefinition {
        self.start.loop_definition
    }

    /// The controller structure of the run.
    #[must_use]
    pub fn profile(&self) -> &PidRunProfile {
        &self.start.profile
    }

    /// The transfer binding the run writes, with its reference hold guard.
    #[must_use]
    pub fn binding(&self) -> &TransferBinding {
        &self.start.binding
    }

    /// The parameter review the run applies.
    #[must_use]
    pub fn review(&self) -> &ParameterReview {
        &self.start.review
    }

    /// The model the run is judged on, anchored to its first reading (B6.2).
    #[must_use]
    pub fn model(&self) -> &GeneralModel {
        &self.start.model
    }

    /// The requirements of the run.
    #[must_use]
    pub fn task(&self) -> &EngineerRequirements {
        self.start.task
    }

    /// The reference of each controlled channel as the review read it (`previousReferences`, B6.4).
    #[must_use]
    pub fn references(&self) -> &BTreeMap<String, f64> {
        &self.references
    }

    /// The first reference as the review read it (`previousReference`, B6.4).
    #[must_use]
    pub fn previous_reference(&self) -> f64 {
        self.previous_reference
    }

    /// The mode the loop was found in (A6.2), or `None` when the review did not read the mode.
    #[must_use]
    pub fn found_mode(&self) -> Option<FoundMode> {
        found_mode_of(self.reviewed_mode, self.start.binding.mode.manual_value)
    }

    /// Whether the loop was found in Auto, so that Restore returns it there (A6.2, B6.4).
    #[must_use]
    pub fn found_in_auto(&self) -> bool {
        self.found_mode() == Some(FoundMode::Auto)
    }

    /// Checks a reading against the starting point of the run (`checkStartingPoint`; B6.2, B6.7): a plant input that the found mode
    /// compares must lie within the allowance of the entry error, and each measured output must rest in its band.
    ///
    /// # Errors
    /// A plant input is away from its validated point, an output has not settled, or a value is not a finite number.
    pub fn check_starting_point(&self, observed: &TransferStatus) -> Result<(), Failure> {
        StartingPoint {
            loop_definition: self.start.loop_definition,
            model: &self.start.model,
            task: self.start.task,
            input_symbols: compared_plant_inputs(&self.start.inputs, self.found_mode()),
            output_symbols: &self.start.outputs,
            input_tolerance: self.start.allowance(&observed.values)?,
        }
        .assert_settled(&observed.values)
    }

    /// Whether the parameter record, the mode and each reference still hold what the review read, before the run starts (`execute`;
    /// B6.2, B6.7): a changed record, mode or reference refuses the run, and the review is made again.
    ///
    /// # Errors
    /// The record, the mode or a reference changed since the review, or a reference the runtime reports is not a finite number.
    pub fn check_reviewed_held(
        &self,
        parameters: &ParameterStatus,
        boundary: &BTreeMap<String, TransferValue>,
    ) -> Result<(), Failure> {
        let reviewed = &self.start.review.observed;
        let record_held = parameters.program_revision == reviewed.program_revision
            && parameters.binding_revision == reviewed.binding_revision
            && parameters.active_revision == reviewed.active_revision
            && same_parameter_values(&parameters.values, &reviewed.values);
        let mode_held = self.reviewed_mode.is_some_and(|mode| {
            boundary.get(&self.start.binding.mode.symbol) == Some(&TransferValue::Boolean(mode))
        });
        if record_held && mode_held && self.references_held(boundary)? {
            return Ok(());
        }
        Err(Failure::new(
            &texts::pid_commission_reviewed_changed(),
            Cause::Runtime,
            NextAction::Validate,
        ))
    }

    /// Whether every reference the review read still holds its value in `values` (`previousReferences`; B6.4).
    ///
    /// # Errors
    /// A reference has no finite value in `values`.
    #[allow(clippy::float_cmp)] // The references are compared exactly, as the TypeScript compares them.
    pub(super) fn references_held(
        &self,
        values: &BTreeMap<String, TransferValue>,
    ) -> Result<bool, Failure> {
        for (symbol, expected) in &self.references {
            if number_at(values, symbol)? != *expected {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The Manual staging of a run that found the loop out of Manual (`execute`; B6.2): the states the structure holds at the boundary,
    /// the transfer allowance, and the tracking that keeps the retained states. The caller stages only when the loop is not in Manual
    /// (`stages_manual`).
    ///
    /// # Errors
    /// The structure's states or tracking do not fit the loop, or a state is missing from the reading.
    pub fn manual_step(&self, observed: &TransferStatus) -> Result<TransferStep, Failure> {
        Ok(TransferStep {
            values: self
                .start
                .profile
                .states(&self.start.binding, &observed.values)?,
            tolerance: transfer_tolerance(
                self.start.loop_definition,
                self.start.task.transfer_tolerance,
            )?,
            entry_error_gain: None,
            tracking: self.start.profile.tracking(
                self.start.loop_definition,
                &self.start.binding,
                false,
            )?,
        })
    }

    /// Checks that the staged parameters still hold their reviewed values before the Auto entry (`execute`; A6.8, B6.7): the refusal
    /// names each parameter the program changed.
    ///
    /// # Errors
    /// A staged parameter does not hold its reviewed value.
    pub fn check_staged(&self, observed: &TransferStatus) -> Result<(), Failure> {
        check_staged_values(&self.start.review.values, &observed.values)
    }

    /// The Auto entry (`execute`; A6.3, A6.4, B6.3): the states the structure holds at the boundary, the allowance of the first output for
    /// the candidate at the entry error of this reading, and the tracking that aligns the output.
    ///
    /// # Errors
    /// The structure's states or tracking do not fit the loop, a state is missing, or the allowance cannot be built.
    pub fn auto_entry_step(&self, observed: &TransferStatus) -> Result<TransferStep, Failure> {
        Ok(TransferStep {
            values: self
                .start
                .profile
                .states(&self.start.binding, &observed.values)?,
            tolerance: self.start.entry_allowance()?,
            entry_error_gain: self.start.entry_error_gain(),
            tracking: self.start.profile.tracking(
                self.start.loop_definition,
                &self.start.binding,
                true,
            )?,
        })
    }

    /// The reviewed target of the setpoint, rounded to its IEC type as the transfer holds it (`target`; B6.2): the operating point's output
    /// plus the reviewed reference change, so that the transfer, the report and the assessment agree.
    ///
    /// # Errors
    /// The loop has no setpoint, the transfer binding has no reference, or the target does not fit the reference's type and bounds.
    pub fn target(&self) -> Result<f64, Failure> {
        commissioning_target(
            self.start.loop_definition,
            &self.start.model,
            self.start.task,
            &self.start.binding,
        )?
        .ok_or_else(binding_failure)
    }

    /// The values of the target change (`execute`'s target; B6.3): the states the structure holds once the loop is in Auto, with the
    /// reference set to the reviewed target, or to the structure's own targets.
    ///
    /// # Errors
    /// A state is missing from the reading, the target cannot be built, or the structure's targets cannot be built.
    pub fn target_values(
        &self,
        entered: &TransferStatus,
    ) -> Result<BTreeMap<String, f64>, Failure> {
        let mut values = self
            .start
            .profile
            .states(&self.start.binding, &entered.values)?;
        if let Some(targets) = self.start.profile.targets(
            self.start.loop_definition,
            &self.start.model,
            self.start.task,
        )? {
            values.extend(targets);
        } else {
            let reference = self
                .start
                .binding
                .states
                .first()
                .ok_or_else(binding_failure)?;
            values.insert(reference.symbol.clone(), self.target()?);
        }
        Ok(values)
    }

    /// The step to the reviewed target while the loop is in Auto (`applyTarget`; A6.3, B6.3): the target values, the allowance of the
    /// target change across the outputs, and the tracking that keeps the retained states.
    ///
    /// # Errors
    /// The target values cannot be built, or the transfer binding has no output allowance.
    pub fn target_step(&self, entered: &TransferStatus) -> Result<TransferStep, Failure> {
        Ok(TransferStep {
            values: self.target_values(entered)?,
            tolerance: transfer_target_allowance(&self.start.binding)
                .ok_or_else(binding_failure)?,
            entry_error_gain: None,
            tracking: self.start.profile.tracking(
                self.start.loop_definition,
                &self.start.binding,
                false,
            )?,
        })
    }

    /// The criteria the response is judged by (`criteria`; B6.3, C1, C3): the reviewed target and duration, the settling time and error
    /// limit, the task period, and the ranges of the process and of the output. A switching controller adds its switching limits, from
    /// its levels and the reviewed minimum times (`pidCommissioning.ts`).
    ///
    /// # Errors
    /// The target cannot be built, or a switching controller's reviewed settings hold no minimum time.
    pub fn criteria(&self) -> Result<CommissioningCriteria, Failure> {
        super::on_off_adapter::with_switching(
            &run_criteria(
                self.start.loop_definition,
                &self.start.binding,
                self.start.task,
                self.target()?,
            ),
            self.start.loop_definition,
            &self.start.review.values,
        )
    }

    /// The channels the run records (`channels`; C1, C13.8): the setpoint, the measurement and the manipulated variable, then every
    /// additional signal of the loop, in the order the loop declares them.
    ///
    /// # Errors
    /// The loop has no setpoint.
    pub fn channels(&self) -> Result<Vec<CommissioningChannel>, Failure> {
        let mut channels = run_channels(self.start.loop_definition)?;
        channels.extend(
            self.start
                .loop_definition
                .signals
                .additional
                .iter()
                .flatten()
                .map(|signal| CommissioningChannel {
                    id: signal.symbol.clone(),
                    unit: signal.unit.clone(),
                }),
        );
        Ok(channels)
    }

    /// The observation the response is judged against (`commissioningObservation`; B6.3, N8): its channels and requirements, built from
    /// the candidate's values.
    ///
    /// # Errors
    /// A requirement cannot be built from the loop and the model.
    pub fn observation(&self) -> Result<CommissioningObservation, Failure> {
        commissioning_observation(
            self.start.loop_definition,
            &self.start.model,
            self.start.task,
            &self.start.review.values,
        )
    }

    /// The response the run requests (`observeTargetChange`'s plan; B6.3): its duration in ms, the target it holds, and the settling time
    /// the record period follows (A4.2). An ON/OFF output has no settling time, so it is absent there.
    ///
    /// # Errors
    /// The target cannot be built.
    pub fn response_request(&self) -> Result<ResponseRequest, Failure> {
        Ok(ResponseRequest {
            duration_ms: self.start.task.duration_seconds * 1000.0,
            target: self.target()?,
            expected_settling_seconds: expected_settling_seconds(
                self.start.loop_definition,
                self.start.task,
            ),
        })
    }

    /// The sentence the engineer reads before the run starts (`description`; B6.2, B6.3): the target it applies, and the duration it observes.
    ///
    /// # Errors
    /// The target cannot be built.
    pub fn description(&self) -> Result<String, Failure> {
        Ok(texts::pid_commission_description(
            self.start.profile.label(),
            self.target()?,
            &self.start.loop_definition.signals.process_variable.unit,
            self.start.task.duration_seconds,
        ))
    }

    /// The sentence that describes the rollback (`rollbackDescription`; B6.4, A6.2): the reference it restores, the record it restores,
    /// and whether the loop returns to Auto.
    #[must_use]
    pub fn rollback_description(&self) -> String {
        texts::pid_commission_rollback_description(
            self.start.profile.label(),
            self.previous_reference,
            &self.start.loop_definition.signals.process_variable.unit,
            self.found_in_auto(),
        )
    }

    /// Judges the recorded response of the run (`assessObservation`; B6.3): the response passes with its largest settled error, or the
    /// attempt says how it failed. The caller calls [`PidCampaign::held_after`] only for a passed response.
    ///
    /// # Errors
    /// The criteria or the observation cannot be built.
    pub fn assess(&self, recording: &ObservedRecording) -> Result<RunAttempt, Failure> {
        Ok(assess_run(
            recording,
            &self.criteria()?,
            &self.observation()?,
        ))
    }

    /// Checks that the parameter record still holds the reviewed program and values after the observation (`execute`; A6.4, A6.8): the
    /// refusal names each parameter the program changed.
    ///
    /// # Errors
    /// The program changed, or a parameter does not hold its reviewed value.
    pub fn held_after(&self, parameters: &ParameterStatus) -> Result<(), Failure> {
        check_held_values(
            &self.start.review.observed.program_revision,
            &self.start.review.values,
            parameters,
        )
    }
}

/// The response a run requests from the runtime (`observeTargetChange`'s plan; B6.3): its duration in ms, the target it holds, and the
/// settling time the record period follows (A4.2), absent for an ON/OFF output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResponseRequest {
    /// The reviewed duration of the response, in ms of process time.
    pub duration_ms: f64,
    /// The reviewed target the response holds.
    pub target: f64,
    /// The settling time the record period follows, in s; `None` for an ON/OFF output.
    pub expected_settling_seconds: Option<f64>,
}

/// What closing the panel does to a Control.PID run (`detachForClose`, `settleForClose`; A8.5, A10.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    /// The panel closes during a recorded observation: the observation is detached at once, nothing is restored or stored, and the runtime
    /// keeps recording it. The campaign's outcome is awaited, and reopening reattaches it.
    Detach,
    /// The panel closes with no observation running: the campaign is cancelled (`cancel`).
    Cancel,
}

/// The decision a close takes for a Control.PID run (A8.5, A10.4): an observation the runtime is recording is detached, never cancelled.
#[must_use]
pub fn close_while_observing(observing: bool) -> CloseAction {
    if observing {
        CloseAction::Detach
    } else {
        CloseAction::Cancel
    }
}

/// Whether a reviewed Control.PID run may start (`run`; A6.6): a campaign starts once. The caller passes whether it is running or has
/// finished.
///
/// # Errors
/// The campaign has already started.
pub fn check_not_started(started: bool) -> Result<(), Failure> {
    if started {
        return Err(Failure::new(
            &texts::pid_commission_already_started(),
            Cause::Unclassified,
            NextAction::None,
        ));
    }
    Ok(())
}

/// Whether a scan starts the response the run judges (`responseStart`; B6.3): the controller is in Auto, and its reference is the
/// reviewed target.
#[must_use]
#[allow(clippy::float_cmp)] // The reference is compared exactly: the target is rounded to the reference's type, and the runtime holds that value.
pub fn response_starts(sample: &ResponseSample, target: f64) -> bool {
    sample.controller_in_auto && sample.setpoint == Some(target)
}

/// The time in s from the start of the recording at which the response starts: the first scan that [`response_starts`] names, or none
/// when no scan does.
#[must_use]
pub fn response_start_seconds(samples: &[ResponseSample], target: f64) -> Option<f64> {
    samples
        .iter()
        .find(|sample| response_starts(sample, target))
        .map(|sample| sample.elapsed_seconds)
}

/// The plant inputs and measured outputs that the starting point compares (B6.2), in the model's channel order, which the operating
/// point's values follow: the loop's own pair, and for a PID group every member's (C7), so that Apply waits until every member's output
/// has settled, not only the primary member's. Each plant input is read where the transfer reads it: its actuator's feedback when the
/// loop declares one.
///
/// # Errors
/// The model does not connect the loop's channels, or a plant input is not an output of the transfer binding.
pub(super) fn starting_point_symbols(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    binding: &TransferBinding,
) -> Result<(Vec<String>, Vec<String>), Failure> {
    if loop_definition.controller.group.is_some() {
        let bound = bound_channels(loop_definition, model)?;
        let inputs = bound
            .inputs
            .iter()
            .map(|input| {
                std::iter::once(&binding.output)
                    .chain(&binding.additional_outputs)
                    .find(|member| member.symbol == input.signal.symbol)
                    .and_then(|member| transfer_output_basis(binding, member))
                    .map(|basis| basis.symbol.clone())
                    .ok_or_else(binding_failure)
            })
            .collect::<Result<Vec<String>, Failure>>()?;
        let outputs = bound
            .outputs
            .iter()
            .map(|output| output.signal.symbol.to_owned())
            .collect();
        return Ok((inputs, outputs));
    }
    let channels = connected_model_channels(loop_definition, model)?;
    let input = transfer_output_basis(binding, &binding.output).ok_or_else(binding_failure)?;
    Ok((
        vec![input.symbol.clone()],
        vec![channels.output.symbol.clone()],
    ))
}

/// Whether the runtime supports the tracking the structure needs (`review`; E6.3): its kind, and each required feature.
fn check_tracking(profile: &PidRunProfile, supported: Option<&[String]>) -> Result<(), Failure> {
    let mut needed = vec![profile.tracking_kind()];
    needed.extend(profile.required_tracking());
    let held = supported.is_some_and(|offered_kinds| {
        needed.iter().all(|kind| {
            offered_kinds
                .iter()
                .any(|offered| offered.as_str() == *kind)
        })
    });
    if held {
        return Ok(());
    }
    Err(Failure::new(
        &texts::pid_commission_tracking_required(profile.tracking_kind()),
        Cause::ControllerCapability,
        NextAction::None,
    ))
}

/// A finite number the runtime reports for `symbol` (the runtime's answer, E6.3).
///
/// # Errors
/// The reading has no finite number for `symbol`.
pub(super) fn number_at(
    values: &BTreeMap<String, TransferValue>,
    symbol: &str,
) -> Result<f64, Failure> {
    values
        .get(symbol)
        .copied()
        .and_then(TransferValue::as_number)
        .filter(|number| number.is_finite())
        .ok_or_else(|| {
            Failure::new(
                &texts::start_value_missing(symbol),
                Cause::Runtime,
                NextAction::None,
            )
        })
}

/// The BOOL that a reading holds for `symbol`, such as a mode, when it holds one.
fn boolean_at(values: &BTreeMap<String, TransferValue>, symbol: &str) -> Option<bool> {
    match values.get(symbol)? {
        TransferValue::Boolean(flag) => Some(*flag),
        TransferValue::Number(_) => None,
    }
}

#[cfg(test)]
#[path = "pid_commissioning_tests.rs"]
mod tests;
