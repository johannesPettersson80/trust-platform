//! The review of an Apply (specification 35, §0.1b, B6.1, B6.2, A2.4, A2.5, A6.4 and A8.4; annex E7.2, E11.8): what the confirmation
//! would write, read from the study and from the live runtime without writing anything, and the identity the confirmation names. A
//! confirmed Apply makes the same reading again and runs only while it gives the same identity. Every rule is trust-control's: the
//! Apply rule of the stages, the adapter's record, the parameter review, the identity parts; this module reads and composes.

use std::path::Path;

use trust_control::commissioning::adapters::{controller_adapter, ControllerGains};
use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::application_records::{
    apply_identity_parts, review_identity, ReviewedParameter,
};
use trust_control::commissioning::apply_texts;
use trust_control::commissioning::checkpoint::CommissioningCheckpoint;
use trust_control::commissioning::checkpoint_restore::check_runtime_identity;
use trust_control::commissioning::commissioning_result::TransferStatus;
use trust_control::commissioning::found_mode::{found_mode_of, FoundMode};
use trust_control::commissioning::parameter_application::{
    review_input, review_of, ParameterReview, RuntimeBinding,
};
use trust_control::commissioning::parameter_binding::runtime_binding_of;
use trust_control::commissioning::parameter_status::ParameterStatus;
use trust_control::commissioning::transfer::{TransferBinding, TransferValue};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::connected_record::connected_identification_record;
use trust_control::identification::connected_run::{
    stored_fit_window, unfinished_operation_refusal,
};
use trust_control::identification::process_noise::measured_process_noise;
use trust_control::loop_definition::{ControllerLaw, LoopDefinition};
use trust_control::model::GeneralModel;
use trust_control::recording::EvidenceSource;
use trust_control::runtime::{RuntimeConnection, RuntimeStatus};
use trust_control::stages::apply_available;
use trust_control::study::identify::IdentifySection;
use trust_control::study::inputs::{held_scenarios, InputsSection};
use trust_control::study::tune::TuneSection;
use trust_control::study::validate::{stage_evidence, ValidateSection};
use trust_control::study::Study;
use trust_control::tuning::candidate::ControlPidCandidate;
use trust_control::validation::engineer_verification_plan::EngineerRequirements;

use super::journaling::{stage_word, unresolved};
use super::law::LawKind;
use super::ApplyContext;
use crate::journal::{Journal, JournalEntry};
use crate::runtime_client::parameters::ParameterTransactions;
use crate::runtime_client::transfers::TransferSession;
use crate::runtime_client::{new_transaction_id, RuntimeClient};
use crate::storage;
use crate::workflow::apply_texts as engine_texts;

/// What an Apply takes from the study (B6.1, B6.2, A6.4): the model the candidate was validated on, the requirements, the candidate's
/// gains, the identity of the validation, and the measured output's noise as single executions see it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ApplyInputs {
    /// The study's model, whose operating point the candidate was validated at (B6.2).
    pub(crate) model: GeneralModel,
    /// The requirements the candidate was validated against.
    pub(crate) task: EngineerRequirements,
    /// The gains of the validated candidate, which the adapter writes as its record (C0).
    pub(crate) gains: ControllerGains,
    /// The identity of the validation that authorizes the Apply (B5.4): the review identity of the study's Validate section.
    pub(crate) validation_identity: String,
    /// The measured output's noise (A6.4): zero without a baseline.
    pub(crate) process_noise: f64,
}

/// The preconditions of an Apply that need no runtime, in their order (B6.1, B6.7, A8.4, A8.5), and what the Apply takes from the study:
/// the study's validation of its candidate is current and passed, so Apply is available; the Rust engine runs the loop; and the loop's
/// journal holds no unfinished operation other than an earlier Apply's checkpoint (any other is waited for). That checkpoint is decided
/// against the running program once the runtime is read ([`checkpoint_to_retire`]).
///
/// # Errors
/// The first precondition that fails, with its next action; or the study's Identify result or its recordings cannot be read.
pub(crate) fn preconditions(context: &ApplyContext<'_>) -> Result<ApplyInputs, Failure> {
    let loop_definition = context.loop_definition;
    let (identify, validate) = validated_sections(context.study, loop_definition)?;
    loop_definition.rust_engine_support()?;
    refuse_unfinished(&*context.journal)?;
    let model = identify.selected_model;
    let process_noise =
        measured_noise(context.project_root, context.study, loop_definition, &model)?;
    Ok(ApplyInputs {
        gains: candidate_gains(loop_definition, &validate.candidate),
        validation_identity: validation_identity(&validate)?,
        task: validate.requirements,
        model,
        process_noise,
    })
}

/// The study's Identify and Validate sections, when its validation is current for the model, the candidate and the requirements and
/// passed (B5.4, A8.4: `stages::apply_available`).
fn validated_sections(
    study: &Study,
    loop_definition: &LoopDefinition,
) -> Result<(IdentifySection, ValidateSection), Failure> {
    let identify = study.section::<IdentifySection>()?;
    let tune = study.section::<TuneSection>()?;
    let validate = study.section::<ValidateSection>()?;
    let inputs = study.section::<InputsSection>()?;
    let evidence = stage_evidence(
        loop_definition,
        identify.as_ref(),
        tune.as_ref(),
        validate.as_ref(),
        !study.recordings.is_empty(),
        &held_scenarios(loop_definition, inputs.as_ref()),
    );
    match (identify, validate) {
        (Some(identify), Some(validate)) if apply_available(&evidence) => Ok((identify, validate)),
        _ => Err(apply_texts::validation_not_current()),
    }
}

/// Refuses an Apply while the loop's journal holds an unfinished operation other than an earlier Apply's checkpoint (A8.5): an Identify
/// campaign, a recording or a mode change, which is waited for.
///
/// # Errors
/// The journal holds another unfinished operation.
pub(crate) fn refuse_unfinished(journal: &Journal) -> Result<(), Failure> {
    let others: Vec<&str> = journal
        .entries()
        .iter()
        .filter(|entry| !matches!(entry, JournalEntry::Commissioning { .. }))
        .map(JournalEntry::operation_id)
        .collect();
    unfinished_operation_refusal(&others).map_or(Ok(()), Err)
}

/// What a new Apply does with the checkpoint that an earlier Apply left in the loop's journal (A8.5, B6.4, E11.7; V4 decision 5), given
/// the parameter record the runtime reports now (`observed`):
/// a completed, recovered or retired checkpoint is replaced when this Apply journals its own, so nothing is returned;
/// an unresolved checkpoint (stage `prepared`, `recording`, `parameters` or `transfer`) of a runtime that was restarted or whose program
/// was reloaded since (`check_runtime_identity`) can no longer be restored, so it is returned for the run to retire as history before it
/// journals its own (the review writes nothing and only lets it pass);
/// an unresolved checkpoint of the running program refuses the Apply, since new work cannot bypass its owners and only Restore resolves
/// them (E11.7).
///
/// # Errors
/// The journal holds an unresolved checkpoint of the running program; the refusal names it and its stage and offers Restore.
pub(crate) fn checkpoint_to_retire(
    journal: &Journal,
    observed: &ParameterStatus,
) -> Result<Option<CommissioningCheckpoint>, Failure> {
    let earlier = journal.entries().iter().find_map(|entry| match entry {
        JournalEntry::Commissioning { checkpoint } => Some(checkpoint.as_ref()),
        _ => None,
    });
    let Some(checkpoint) = earlier.filter(|checkpoint| unresolved(checkpoint.stage)) else {
        return Ok(None);
    };
    if check_runtime_identity(checkpoint, observed).is_err() {
        return Ok(Some(checkpoint.clone()));
    }
    Err(apply_texts::checkpoint_unresolved(
        &checkpoint.id,
        stage_word(checkpoint.stage),
    ))
}

/// The measured output's noise as single executions see it (A6.4; `measuredProcessNoise`): from the steady baseline of the recording the
/// model was fitted from, rebuilt from the study with the request it was taken with (B3.6). Zero without a baseline: an Identify result
/// without its request, or a recording that does not rebuild into a record.
pub(crate) fn measured_noise(
    project_root: &Path,
    study: &Study,
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<f64, Failure> {
    let Some(stored) = storage::read_identify(project_root, study)? else {
        return Ok(0.0);
    };
    let Some(taken) = &stored.section.record_request else {
        return Ok(0.0);
    };
    let window = stored_fit_window(&taken.request);
    let record = connected_identification_record(
        loop_definition,
        &stored.estimation.run,
        None,
        &taken.request.experiment,
        window.fit_from_seconds,
        Some(window.baseline_end_seconds),
        |_, _| Ok(()),
    );
    Ok(record.map_or(0.0, |record| {
        measured_process_noise(loop_definition, Some(model), std::slice::from_ref(&record))
    }))
}

/// The gains of a validated candidate, as the adapter writes them (C0; `materialize(design.candidate, observed)`): the three gains, the
/// setpoint weights and the measurement filter the record shapes, and the back-calculation gain `1/Tt` where the law declares a
/// back-calculation anti-windup, whose tracking time the record's `Tt` is.
#[must_use]
pub(crate) fn candidate_gains(
    loop_definition: &LoopDefinition,
    candidate: &ControlPidCandidate,
) -> ControllerGains {
    let record = &candidate.parameters;
    let back_calculation = matches!(
        &loop_definition.controller.law,
        Some(ControllerLaw::Pid(law)) if law.anti_windup.is_some()
    );
    ControllerGains {
        proportional_gain: Some(record.proportional_gain),
        integral_gain: Some(record.integral_gain),
        derivative_gain: Some(record.derivative_gain),
        back_calculation_gain: back_calculation.then_some(record.tracking_time.recip()),
        proportional_setpoint_weight: Some(record.proportional_setpoint_weight),
        derivative_setpoint_weight: Some(record.derivative_setpoint_weight),
        measurement_filter_seconds: candidate.measurement_filter_seconds,
    }
}

/// The identity of the study's validation (B5.4, A8.4): the review identity of its Validate section, so that any change of the model,
/// the candidate, the requirements or the result gives another identity.
fn validation_identity(validate: &ValidateSection) -> Result<String, Failure> {
    let value = serde_json::to_value(validate).map_err(|error| {
        Failure::new(
            &engine_texts::validation_unreadable(&error.to_string()),
            Cause::Implementation,
            NextAction::None,
        )
    })?;
    Ok(review_identity(&value))
}

/// The two connections an Apply holds: one for its transfers and one for its parameter transactions (annex E6, E7).
pub(crate) struct Clients {
    /// The connection of the transfers and of the runtime's facts.
    pub(crate) transfer: RuntimeClient,
    /// The connection of the parameter transactions.
    pub(crate) parameters: RuntimeClient,
}

impl Clients {
    /// Connects to the runtime of `connection` twice.
    ///
    /// # Errors
    /// The runtime cannot be reached.
    pub(crate) fn connect(connection: &RuntimeConnection) -> Result<Self, Failure> {
        Ok(Self {
            transfer: RuntimeClient::connect(connection)?,
            parameters: RuntimeClient::connect(connection)?,
        })
    }
}

/// What the review read from the live runtime (B6.1, B6.2; E6.3, E7.2): the runtime's facts and target, the law and its transfer binding,
/// the parameter binding, the parameter review of the candidate's record, the first transfer reading, the mode the loop was found in,
/// and the review identity.
pub(crate) struct Prepared {
    /// The runtime's facts (E1.3).
    pub(crate) runtime: RuntimeStatus,
    /// Where the writes go (A2.4).
    pub(crate) source: ApplySource,
    /// The law the adapter commissions.
    pub(crate) kind: LawKind,
    /// The transfer binding the run writes through.
    pub(crate) binding: TransferBinding,
    /// The binding of the parameter record (E7.1).
    pub(crate) runtime_binding: RuntimeBinding,
    /// The review of the candidate's record against the record the runtime reports (E7.2, E7.3).
    pub(crate) review: ParameterReview,
    /// The first transfer reading (E6.3).
    pub(crate) first: TransferStatus,
    /// The mode the loop was found in (A6.2).
    pub(crate) found_mode: Option<FoundMode>,
    /// The review identity (§0.1b Confirmation).
    pub(crate) identity: String,
    /// An earlier Apply's unresolved checkpoint of a replaced runtime, which the run retires before it journals its own (B6.4).
    pub(crate) retired: Option<CommissioningCheckpoint>,
}

/// Reads what the review shows from the live runtime, writing nothing (B6.1, B6.2, A2.4; E11.8): the target fact, which the review
/// shows so that the engineer approves a physical write on it (A2.5, §0.1b: the approval is given on the review); the adapter and its
/// initialization; the parameter record, against which an earlier Apply's checkpoint is decided ([`checkpoint_to_retire`]), and the
/// candidate's record written over it; the law's transfer binding and its first reading; and the identity of all that the confirmation
/// approves.
///
/// # Errors
/// The runtime cannot be reached or refuses; the adapter or its initialization does not fit; the journal holds an unresolved checkpoint
/// of the running program (Restore); the record does not take the candidate's values; or the record holds a pending transaction.
pub(crate) fn prepare(
    context: &ApplyContext<'_>,
    inputs: &ApplyInputs,
    clients: &mut Clients,
) -> Result<Prepared, Failure> {
    let loop_definition = context.loop_definition;
    let runtime = clients.transfer.status()?;
    let source = match runtime.source() {
        EvidenceSource::SimulatedPlant => ApplySource::Simulation,
        EvidenceSource::PhysicalPlant => ApplySource::Physical,
    };
    let adapter = controller_adapter(loop_definition)?;
    adapter.check_initialization()?;
    let runtime_binding = runtime_binding_of(loop_definition)?;
    let observed =
        ParameterTransactions::new(&mut clients.parameters, runtime_binding.clone(), source)
            .status()?;
    let retired = checkpoint_to_retire(&*context.journal, &observed)?;
    let values = adapter.materialize(&inputs.gains, &observed.values)?;
    let review = review_of(
        review_input(&runtime_binding, &values)?,
        observed,
        new_transaction_id(),
    )?;
    let kind = LawKind::of(&adapter, &inputs.model, &review.values)?;
    let binding = kind.binding(&adapter, loop_definition)?;
    let first = TransferSession::new(
        &mut clients.transfer,
        binding.clone(),
        review.observed.program_revision.clone(),
    )
    .status()?;
    let found_mode = found_mode_of(
        match first.values.get(&binding.mode.symbol) {
            Some(TransferValue::Boolean(mode)) => Some(*mode),
            _ => None,
        },
        binding.mode.manual_value,
    );
    let identity = review_identity(&apply_identity_parts(
        &review.observed,
        found_mode,
        &review.values,
        &first.binding_revision,
        &inputs.validation_identity,
    ));
    Ok(Prepared {
        runtime,
        source,
        kind,
        binding,
        runtime_binding,
        review,
        first,
        found_mode,
        identity,
        retired,
    })
}

/// Each bound parameter with the value it holds in the running PLC and the value the confirmation writes (§0.1b `apply`), in the order
/// the binding declares them.
#[must_use]
pub(crate) fn reviewed_parameters(
    runtime_binding: &RuntimeBinding,
    review: &ParameterReview,
) -> Vec<ReviewedParameter> {
    runtime_binding
        .parameters
        .iter()
        .filter_map(|parameter| {
            let running = review.observed.values.get(&parameter.symbol)?;
            let next = review.values.get(&parameter.symbol)?;
            Some(ReviewedParameter {
                symbol: parameter.symbol.clone(),
                unit: parameter.unit.clone(),
                running: *running,
                next: *next,
            })
        })
        .collect()
}
