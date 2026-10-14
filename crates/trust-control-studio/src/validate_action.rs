//! The Validate method `controlStudio/validate` (specification 35, §0.1a, §0.1b, A8.1, A8.4 to A8.6 and B5; the Validate page
//! specification): the operation that decides whether the candidate may be applied.
//!
//! `validate` answers at once with its operation's identifier, once the refusals that need no operation hold: the loop opens, no
//! operation of this engine works on it, its journal holds no unfinished operation, a runtime is connected, the study holds a
//! current model and a current candidate (B5.5, A8.4), and the isolated simulation is ready (E8.2). The operation claims the loop
//! (A8.5) and runs the two parts of B5 in their order (B5.1): the independent model check, when the model is provisional (B5.2),
//! then the actual-ST validation of the current and the candidate settings in isolated simulations (B5.3). Its final update
//! carries trust-control's Validate result, and the study keeps the Validate section (A8.1). Validate never writes to the
//! controller (B5.1).
//!
//! Every rule is trust-control's: the checks before the start, the requests of the isolated runs, the assessment of each run and
//! the section it keeps. This module reads the loop, its study and the runtime, and makes the calls in their order.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use trust_control::commissioning::adapters::controller_adapter;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::connected_run::runtime_required;
use trust_control::identification::connected_run::unfinished_operation_refusal;
use trust_control::identification::independent_validation::RequirementStatus;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{ModelCheck as CheckRecord, ValidateRequest};
use trust_control::model::{GeneralModel, SimulationOverride};
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::study::identify::IdentifySection;
use trust_control::study::inputs::{held_loop, held_scenarios, tune_as_held, InputsSection};
use trust_control::study::tune::TuneSection;
use trust_control::study::validate::{
    check_before_start, stage_evidence, validate_outcome, CaseJobs, EntryJob, ModelQualification,
    ValidateInputs, ValidateOutcome, ValidationInput,
};
use trust_control::study::Study;
use trust_control::tuning::candidate::ControlPidCandidate;
use trust_control::validation::configured_plant_verification::{
    decode_configured_plant_binding, verification_plant, VerificationPlant,
};
use trust_control::validation::engineer_verification_campaign::{
    engineer_verification_campaign, CampaignCase,
};
use trust_control::validation::initialization_case::{
    entry_start_model, plan_entry_case, EntryCase,
};
use trust_control::validation::isolated_run_protocol::{
    IsolatedRunRecord, IsolatedRunRequest, IsolatedRunStatus,
};
use trust_control::validation::model_route::{model_route, ModelRoute};
use trust_control::validation::model_validation_session::maximum_normalized_error;
use trust_control::validation::plant_choice::model_replacement;
use trust_control::validation::run_requests::{isolated_run_request, RunRequestInput, RunSettings};

use crate::busy_loops::{LoopClaim, LoopClaims};
use crate::commands::to_value;
use crate::identify_adapters::{RuntimeSampler, StudyRetention};
use crate::journal::{Journal, JournalEntry};
use crate::loops;
use crate::operation::{cancelled, OperationContext, Operations};
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::runtime_client::RuntimeClient;
use crate::storage;
use crate::workflow::apply::{candidate_gains, measured_noise};
use crate::workflow::identify::IdentifyPorts;
use crate::workflow::validate::{
    new_job_id, run_job, run_scenarios, JobTiming, RunIdentity, RunSession, ScenarioRuns, Settings,
};
use crate::workflow::validate_model_check::{run_model_check, ModelCheck, ModelCheckRequest};

/// The command of the operation that `validate` starts (§0.1a).
const VALIDATE_COMMAND: &str = "validate";
/// The name of the operation that holds the loop, as a refusal names the operation that holds it (A8.5).
const OPERATION_NAME: &str = "Validate";
// The sentences of the operation. They belong with the other sentences of Validate in `validation::texts`.
const CHECKING_MODEL: &str = "Checking the model against its own recording…";
const PREPARING_RUNS: &str = "Preparing the isolated simulations…";
const ASSESSING_RUNS: &str = "Assessing the isolated runs…";
const ENTRY_JOB_MISSING: &str =
    "The Manual-to-Auto entry case was planned, but its job is missing.";
const MODEL_CHECK_PASSED: &str = "The independent model check passed: the frozen model predicted the recorded run within the identification error limit.";

/// What the Validate operation works with: owned, so that it moves to the operation's thread.
struct ValidateWork {
    project_root: PathBuf,
    storage_folder: PathBuf,
    pending: Arc<PendingReleases>,
    loop_definition: LoopDefinition,
}

/// Answers `controlStudio/validate`: refuses at once what needs no operation (B5.5, A8.4, E8.2), then starts the Validate
/// operation and answers with its identifier. The loop is claimed in `claims` until the operation ends (A8.5); a refused request
/// lets it go at once. The recordings the operation stops are released through `pending`, the engine's one list (E4.7).
///
/// # Errors
/// The request cannot be read, or the loop cannot be opened; an operation of this engine works on the loop; the journal cannot
/// be read or holds an unfinished operation (A8.5); the study cannot be read, or it has no current model or no current candidate
/// (trust-control's `check_before_start` names the next action); or the isolated simulation is not ready, with the runtime's
/// reason (E8.2).
pub fn answer(
    operations: &Operations,
    pending: &Arc<PendingReleases>,
    claims: &LoopClaims,
    request: ValidateRequest,
) -> Result<Value, Failure> {
    let ValidateRequest {
        project_root,
        runtime_id,
        loop_id,
        storage_folder,
        connection,
    } = request;
    let project_root = PathBuf::from(project_root);
    let storage_folder = PathBuf::from(storage_folder);
    let loop_definition = loops::open(&project_root, &runtime_id, &loop_id)?;
    let claim = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        OPERATION_NAME,
    )?;
    let journal = Journal::open(
        &storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    refuse_while_journaled(&journal)?;
    let study = storage::open_study(
        &project_root,
        &loop_definition.id,
        &loop_definition.revision,
    )?;
    // The model and the candidate are refused before the runtime (B5.5's order); a request without a connection is then
    // refused as Identify refuses it.
    let readiness = connection
        .as_ref()
        .map_or_else(|| Err(runtime_required()), readiness_of);
    validated_sections(&study, &loop_definition, &readiness)?;
    let connection = connection.ok_or_else(runtime_required)?;
    let work = ValidateWork {
        project_root,
        storage_folder,
        pending: Arc::clone(pending),
        loop_definition,
    };
    to_value(&operations.start(
        VALIDATE_COMMAND,
        claimed(claim, move |context| {
            run_validate(context, &work, &connection)
        }),
    ))
}

/// Starts `work` as the Validate operation, which holds `claim` until it ends.
fn claimed(
    claim: LoopClaim,
    work: impl FnOnce(&OperationContext) -> Result<Value, Failure> + Send + 'static,
) -> impl FnOnce(&OperationContext) -> Result<Value, Failure> + Send + 'static {
    move |context| {
        let worked = work(context);
        drop(claim);
        worked
    }
}

/// The Validate operation (B5.1): the model check of a provisional model, then the actual-ST validation of every scenario with the
/// current and the candidate settings, then the Validate section the study keeps (A8.1). A cancelled operation keeps nothing.
fn run_validate(
    context: &OperationContext,
    work: &ValidateWork,
    connection: &RuntimeConnection,
) -> Result<Value, Failure> {
    let loop_definition = &work.loop_definition;
    // The readiness of the isolated simulation (E8.2) gives the project revision every run is reviewed on.
    let readiness = readiness_of(connection);
    let study = working_study(work)?;
    let (identify, tune) = validated_sections(&study, loop_definition, &readiness)?;
    let project_revision = readiness.clone()?;
    // Part one (B5.1 (a), B5.2): the model check of a provisional model. A check that does not pass stops Validate (B5.5).
    let qualification = model_check_part(context, work, connection, &study, &identify, &tune)?;
    // Part two (B5.3) runs on the study as the check left it: a passing check qualifies the model.
    let study = working_study(work)?;
    let (identify, tune) = validated_sections(&study, loop_definition, &readiness)?;
    let outcome = actual_part(
        context,
        work,
        connection,
        &project_revision,
        &identify,
        &tune,
        qualification,
    )?;
    // The failure that stops Apply (B5.5) is not part of the operation's update, whose result is this whole answer.
    let ValidateOutcome {
        section,
        result,
        failure: _,
    } = outcome;
    // The study is read again for the write, so that a page the engineer chose while the runs ran is kept (A8.1).
    let mut study = working_study(work)?;
    study.set_section(&section)?;
    storage::save_study(&work.project_root, &study, &[])?;
    to_value(&result)
}

/// Part two of Validate (B5.3): the cases of the campaign run as isolated test runs with the current settings and with the candidate,
/// on the plant that B5.3 chooses (`run_on_plant`); then trust-control composes the result from the jobs (B5.4). The runs are
/// journaled and read by their identity (E11.2, E11.4); a cancelled operation stops at its next step and composes nothing.
fn actual_part(
    context: &OperationContext,
    work: &ValidateWork,
    connection: &RuntimeConnection,
    project_revision: &str,
    identify: &IdentifySection,
    tune: &TuneSection,
    qualification: ModelQualification,
) -> Result<ValidateOutcome, Failure> {
    // Validate runs on the loop with the scenarios the study holds in place of its own (B4.3, A8.4). The campaign, the runs and the
    // validation it records all take this loop, so the scenarios that ran are the ones the section keeps.
    let study_inputs = working_study(work)?.section::<InputsSection>()?;
    let held = held_loop(&work.loop_definition, study_inputs.as_ref())?;
    let loop_definition = &held;
    context.progress(PREPARING_RUNS);
    let mut client = RuntimeClient::connect(connection)?;
    // The settings the controller runs with now (B5.3): the current run writes them as its record, as the candidate run writes
    // the candidate's, and the campaign checks that every bound parameter is present and within its bounds (E8.4).
    let observed = client.parameter_values(loop_definition)?;
    let plant = configured_plant_of(&mut client, loop_definition);
    let model = &identify.selected_model;
    // The campaign on the configured plant is built first: its refusals (B5.5) hold whichever plant the runs take.
    let configured = engineer_verification_campaign(
        loop_definition,
        model,
        &tune.requirements,
        &observed,
        &BTreeMap::new(),
    )?;
    // The noise of the Manual-to-Auto entry case is read before the runs, and its failure ends the validation (`entry_noise`). The case
    // itself is planned after the runs, from the point its plant starts at (B5.3), so a refusal of its plan is the case's own (B5.4, C1).
    let noise = entry_noise(work, loop_definition, model)?;
    let mut journal = Journal::open(
        &work.storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    let run_identity = RunIdentity {
        project_id: loop_definition.project_id.clone(),
        runtime_id: loop_definition.runtime_id.clone(),
        loop_id: loop_definition.id.clone(),
        project_revision: project_revision.to_string(),
    };
    let is_cancelled = || context.is_cancelled();
    let progress = |text: &str| context.progress(text);
    let mut session = RunSession {
        client: &mut client,
        journal: &mut journal,
        identity: run_identity,
        is_cancelled: &is_cancelled,
        progress: &progress,
        timing: JobTiming::PRODUCTION,
    };
    let inputs = RunInputs {
        loop_definition,
        model,
        tune,
        observed: &observed,
        project_revision,
    };
    let routed = run_on_plant(&mut session, &inputs, &plant, configured)?;
    // The entry case starts where its plant starts (B5.3): on the configured plant, at the first sample of the target-response run that
    // the runs have just recorded, so the case is planned on that start model.
    let start = entry_start_model(loop_definition, model, routed.entry_start_record());
    let planned = plan_entry(loop_definition, &start, tune, &observed, noise);
    let entry_job = planned
        .as_ref()
        .ok()
        .map(|plan| run_entry(&mut session, plan, &inputs, &routed.plant))
        .transpose()?;
    let entry = match (&planned, &entry_job) {
        (Ok(plan), Some(job)) => EntryJob::Ran {
            case: &plan.case,
            job,
            plant: &routed.plant,
        },
        (Err(refusal), _) => EntryJob::Refused(refusal),
        (Ok(_), None) => {
            return Err(Failure::new(
                ENTRY_JOB_MISSING,
                Cause::Implementation,
                NextAction::None,
            ))
        }
    };
    // A cancelled operation keeps nothing: its runs ended, and the study is written only by a finished one.
    if context.is_cancelled() {
        return Err(cancelled());
    }
    context.progress(ASSESSING_RUNS);
    let jobs: Vec<CaseJobs<'_>> = routed
        .runs
        .iter()
        .map(|run| CaseJobs {
            plant: &routed.plant,
            current: &run.current,
            candidate: &run.candidate,
        })
        .collect();
    let study = working_study(work)?;
    let before = stage_evidence(
        loop_definition,
        Some(identify),
        Some(tune),
        None,
        !study.recordings.is_empty(),
        &held_scenarios(loop_definition, study_inputs.as_ref()),
    );
    validate_outcome(&ValidationInput {
        loop_definition,
        model,
        tune,
        before,
        cases: &routed.cases,
        jobs: &jobs,
        qualification,
        plant_reason: routed.reason,
        entry: Some(entry),
    })
}

/// Reads the measured noise that the Manual-to-Auto entry case judges with (A6.4): the noise that Apply's Auto entry uses, so the case
/// judges the same allowance that Apply will. The study and the noise are reads of the validation itself, so a failure here ends
/// Validate (`?`) rather than making the case Inconclusive (B5.4).
fn entry_noise(
    work: &ValidateWork,
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<f64, Failure> {
    let study = working_study(work)?;
    measured_noise(&work.project_root, &study, loop_definition, model)
}

/// Plans the Manual-to-Auto entry case of this validation (B5.3) on `model`, the case's start model (`entry_start_model`). Every Apply
/// enters Auto (B6.1, A6.2), so every Validate plans the case. The candidate's record is materialized as Apply materializes it. The
/// refusals of the controller adapter, of the materialization and of the case's plan are the case's refusal (B5.4, C1), not the
/// validation's.
fn plan_entry(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    tune: &TuneSection,
    observed: &BTreeMap<String, f64>,
    noise: f64,
) -> Result<EntryCase, Failure> {
    let gains = candidate_gains(loop_definition, &tune.candidate);
    let values = controller_adapter(loop_definition)?.materialize(&gains, observed)?;
    plan_entry_case(loop_definition, model, &tune.requirements, &values, noise)
}

/// Runs the Manual-to-Auto entry case as one isolated job (B5.3, E8.4) on the plant that the campaign ran on: the selected model's
/// inline plant when the model is the plant, and the configured plant otherwise. The job carries the candidate's record.
fn run_entry(
    session: &mut RunSession<'_>,
    entry: &EntryCase,
    inputs: &RunInputs<'_>,
    plant: &VerificationPlant,
) -> Result<IsolatedRunStatus, Failure> {
    let simulation = match plant {
        VerificationPlant::Model => {
            Some(model_route(inputs.loop_definition, inputs.model, inputs.observed)?.simulation)
        }
        VerificationPlant::Configured { .. } => None,
    };
    let job = new_job_id();
    let request = entry.request(
        inputs.loop_definition,
        inputs.model,
        &inputs.tune.candidate,
        simulation.as_ref(),
        &job,
        inputs.project_revision,
    )?;
    run_job(session, &request)
}

/// The inputs that every run of one Validate is built from (B5.3): the loop, its selected model, the Tune section, the current settings
/// read from the running controller, and the project revision the runs are reviewed on (E8.2).
struct RunInputs<'a> {
    loop_definition: &'a LoopDefinition,
    model: &'a GeneralModel,
    tune: &'a TuneSection,
    observed: &'a BTreeMap<String, f64>,
    project_revision: &'a str,
}

/// The runs of the campaign and the plant they took (B5.3).
struct RoutedRuns {
    /// The plant that every case ran on.
    plant: VerificationPlant,
    /// The campaign's cases, in their order, on `plant`.
    cases: Vec<CampaignCase>,
    /// Each case's current and candidate runs, in the order of `cases`.
    runs: Vec<ScenarioRuns>,
    /// Why the model replaced the configured plant (B5.3); absent when the configured plant ran the cases, and when the model was the
    /// plant from the start.
    reason: Option<String>,
}

/// Runs every case of the campaign on the plant that B5.3 chooses. The configured plant runs the cases when it drives the loop, unless
/// the target-response run of the current settings, measured first, starts outside the settling band around the model's operating point:
/// then every case runs on the selected model, and the reason is named (trust-control's `model_replacement`). When no configured plant
/// drives the loop, the model is the plant from the start.
fn run_on_plant(
    session: &mut RunSession<'_>,
    inputs: &RunInputs<'_>,
    plant: &VerificationPlant,
    configured: Vec<CampaignCase>,
) -> Result<RoutedRuns, Failure> {
    match plant {
        VerificationPlant::Model => {
            let route = model_route(inputs.loop_definition, inputs.model, inputs.observed)?;
            let (cases, runs) = model_runs(session, inputs, &route)?;
            Ok(RoutedRuns {
                plant: VerificationPlant::Model,
                cases,
                runs,
                reason: None,
            })
        }
        VerificationPlant::Configured { .. } => {
            let (target, rest) = configured.split_first().ok_or_else(|| {
                Failure::new(
                    "The campaign has no target-response case.",
                    Cause::Implementation,
                    NextAction::None,
                )
            })?;
            let shape = RunShape {
                loop_definition: inputs.loop_definition,
                model: inputs.model,
                simulation: None,
                observed: inputs.observed,
                candidate: &inputs.tune.candidate,
                project_revision: inputs.project_revision,
            };
            (session.progress)(&format!("Running scenario {}.", target.id));
            let probe = run_one(session, &shape, target, Settings::Current)?;
            let replacement = model_replacement(
                &probe,
                inputs.loop_definition,
                inputs.model,
                inputs.tune.requirements.settling_band,
            );
            if let Some(reason) = replacement {
                let route = model_route(inputs.loop_definition, inputs.model, inputs.observed)?;
                let (cases, runs) = model_runs(session, inputs, &route)?;
                Ok(RoutedRuns {
                    plant: VerificationPlant::Model,
                    cases,
                    runs,
                    reason: Some(reason),
                })
            } else {
                let runs = configured_runs(session, &shape, target, rest, probe)?;
                Ok(RoutedRuns {
                    plant: plant.clone(),
                    cases: configured,
                    runs,
                    reason: None,
                })
            }
        }
    }
}

impl RoutedRuns {
    /// The run that the Manual-to-Auto entry case starts from (B5.3): on the configured plant, the current settings' target-response run,
    /// which the runs put first. On the model the case starts at the model's operating point, so there is no such run.
    fn entry_start_record(&self) -> Option<&IsolatedRunRecord> {
        match self.plant {
            VerificationPlant::Configured { .. } => self.runs.first()?.current.run.as_ref(),
            VerificationPlant::Model => None,
        }
    }
}

/// The runs of every case of a fresh campaign on the selected model (B5.3): the model's inline plant, and the controller's initial
/// values at the model's operating point (B6.2, `model_route`).
fn model_runs(
    session: &mut RunSession<'_>,
    inputs: &RunInputs<'_>,
    route: &ModelRoute,
) -> Result<(Vec<CampaignCase>, Vec<ScenarioRuns>), Failure> {
    let cases = engineer_verification_campaign(
        inputs.loop_definition,
        inputs.model,
        &inputs.tune.requirements,
        inputs.observed,
        &route.initial_values,
    )?;
    let shape = RunShape {
        loop_definition: inputs.loop_definition,
        model: inputs.model,
        simulation: Some(&route.simulation),
        observed: inputs.observed,
        candidate: &inputs.tune.candidate,
        project_revision: inputs.project_revision,
    };
    let build =
        |case: &CampaignCase, settings: Settings, job: &str| shape.request(case, settings, job);
    let runs = run_scenarios(session, &cases, &cases, &build)?;
    Ok((cases, runs))
}

/// The runs of the campaign on the configured plant, once the probe has shown that the plant can show the response (B5.3): the probe
/// is the target-response case's current run, its candidate run follows, and the other cases run as `run_scenarios` runs them.
fn configured_runs(
    session: &mut RunSession<'_>,
    shape: &RunShape<'_>,
    target: &CampaignCase,
    rest: &[CampaignCase],
    probe: IsolatedRunStatus,
) -> Result<Vec<ScenarioRuns>, Failure> {
    let candidate = run_one(session, shape, target, Settings::Candidate)?;
    let mut runs = vec![ScenarioRuns {
        id: target.id.clone(),
        current: probe,
        candidate,
    }];
    let build =
        |case: &CampaignCase, settings: Settings, job: &str| shape.request(case, settings, job);
    runs.extend(run_scenarios(session, rest, rest, &build)?);
    Ok(runs)
}

/// Runs one case with one settings as an isolated job and waits for its end (B5.3, E8.4): the job takes a new identifier, and its
/// identity is journaled before its start (`run_job`).
fn run_one(
    session: &mut RunSession<'_>,
    shape: &RunShape<'_>,
    case: &CampaignCase,
    settings: Settings,
) -> Result<IsolatedRunStatus, Failure> {
    let job = new_job_id();
    let request = shape.request(case, settings, &job)?;
    run_job(session, &request)
}

/// What every isolated run of one Validate is built from (B5.3): the loop, its selected model, the selected model's inline plant when
/// the runs take the model (`None` for the configured plant), the current settings read from the running controller, the candidate,
/// and the project revision the runs are reviewed on (E8.2).
struct RunShape<'a> {
    loop_definition: &'a LoopDefinition,
    model: &'a GeneralModel,
    simulation: Option<&'a SimulationOverride>,
    observed: &'a BTreeMap<String, f64>,
    candidate: &'a ControlPidCandidate,
    project_revision: &'a str,
}

impl RunShape<'_> {
    /// The request of one case under one settings (E8.4, B5.3): the current settings and the candidate each run their record as
    /// parameter overrides. The configured plant takes no inline plant; the runs on the selected model pass the model's
    /// `simulation_override`.
    fn request(
        &self,
        case: &CampaignCase,
        settings: Settings,
        job: &str,
    ) -> Result<IsolatedRunRequest, Failure> {
        let settings = match settings {
            Settings::Current => RunSettings::Current(self.observed),
            Settings::Candidate => RunSettings::Candidate(self.candidate),
        };
        isolated_run_request(&RunRequestInput {
            loop_definition: self.loop_definition,
            model: self.model,
            plan: &case.plan,
            case,
            settings,
            simulation: self.simulation,
            request_id: job,
            expected_project_revision: self.project_revision,
        })
    }
}

/// Part one of Validate (B5.1 (a), B5.2): a provisional model is checked against its own live recording, and a check that does not
/// pass fails the operation with its reason (B5.5). A qualified model needs no check; its qualification is the one Apply needs
/// (B5.4).
fn model_check_part(
    context: &OperationContext,
    work: &ValidateWork,
    connection: &RuntimeConnection,
    study: &Study,
    identify: &IdentifySection,
    tune: &TuneSection,
) -> Result<ModelQualification, Failure> {
    if !identify.provisional {
        return Ok(ModelQualification::Qualified);
    }
    let loop_definition = &work.loop_definition;
    let stored = storage::read_identify(&work.project_root, study)?.ok_or_else(study_changed)?;
    // A controller that is not ON/OFF gives the check the settling time of its requirements (B3.4 step 2, A4.2).
    let expected_settling_seconds = if loop_definition.controller.on_off.is_some() {
        None
    } else {
        Some(tune.requirements.settling_time_seconds)
    };
    context.progress(CHECKING_MODEL);
    let source = runtime_source(connection)?;
    let mut sampler = RuntimeSampler::new(connection, loop_definition, source);
    let mut retention = StudyRetention::new(
        &work.project_root,
        &loop_definition.id,
        source,
        study.clone(),
    );
    let request = ModelCheckRequest {
        project_root: &work.project_root,
        loop_definition,
        stored: &stored,
        expected_settling_seconds,
    };
    let check = run_model_check(
        context,
        connection,
        &work.pending,
        &work.storage_folder,
        &request,
        IdentifyPorts {
            sampler: &mut sampler,
            retention: &mut retention,
        },
    )?;
    match check {
        ModelCheck::NotNeeded => Ok(ModelQualification::Qualified),
        ModelCheck::Passed {
            recording_id,
            normalized_error,
        } => Ok(ModelQualification::Checked(passed_check(
            identify,
            loop_definition,
            recording_id,
            normalized_error,
        )?)),
        ModelCheck::Failed(failure) => Err(failure),
    }
}

/// The model check of a provisional model that passed (B5.2), as the Validate result shows it: the status is Pass, the measured
/// normalized error over the whole response, and the identification policy's limit (N3.4).
///
/// # Errors
/// The loop's identification policy does not hold its limit (`maximum_normalized_error`).
fn passed_check(
    identify: &IdentifySection,
    loop_definition: &LoopDefinition,
    recording_id: String,
    normalized_error: Option<f64>,
) -> Result<CheckRecord, Failure> {
    Ok(CheckRecord {
        status: RequirementStatus::Pass,
        model_revision: identify.selected_model.identity().revision.clone(),
        recording_id,
        limit: maximum_normalized_error(loop_definition)?,
        normalized_error,
        reason: MODEL_CHECK_PASSED.to_string(),
    })
}

/// The configured plant the runs validate against (B5.3): the project's configured simulated plant when it drives the loop's
/// signals, otherwise the qualified model. The readiness of E8.2 has shown that the host is simulated, so the target counts as
/// simulated. A runtime that cannot answer `plant.read` leaves no binding, and the model is the plant (B5.3).
fn configured_plant_of(
    client: &mut RuntimeClient,
    loop_definition: &LoopDefinition,
) -> VerificationPlant {
    let binding = loop_definition.plant_id.as_deref().and_then(|plant_id| {
        client
            .plant_read()
            .ok()
            .and_then(|answer| decode_configured_plant_binding(&answer, plant_id))
    });
    verification_plant(loop_definition, true, binding.as_ref())
}

/// The readiness of the isolated simulation of the runtime of `connection` (E8.2): the project revision a run starts on, or the
/// runtime's reason why it cannot run.
fn readiness_of(connection: &RuntimeConnection) -> Result<String, Failure> {
    RuntimeClient::connect(connection).and_then(|mut client| client.isolated_run_readiness())
}

/// The study's Identify and Tune sections, both of which a Validate needs (B5.5, A8.4). trust-control's `check_before_start` refuses
/// a study without a current model or a current candidate, and an isolated simulation that is not ready, before any of them is
/// returned.
///
/// # Errors
/// The sections cannot be read, or trust-control refuses the study with the next action it names.
fn validated_sections(
    study: &Study,
    loop_definition: &LoopDefinition,
    readiness: &Result<String, Failure>,
) -> Result<(IdentifySection, TuneSection), Failure> {
    let identify = study.section::<IdentifySection>()?;
    let tune = study.section::<TuneSection>()?;
    let inputs = study.section::<InputsSection>()?;
    check_before_start(&ValidateInputs {
        loop_definition,
        identify: identify.as_ref(),
        tune: tune.as_ref(),
        readiness,
    })?;
    match (identify, tune) {
        // Validate judges the candidate against the requirements the study holds (B4.3, A8.4).
        (Some(identify), Some(tune)) => Ok((identify, tune_as_held(&tune, inputs.as_ref()))),
        _ => Err(study_changed()),
    }
}

/// The Validate stage of a run (B8): the operation of `controlStudio/validate`, on the study as it is, inside the run's own
/// operation. The run holds the loop's claim, so nothing is claimed here, and its Cancel reaches the runs through `context`.
///
/// # Errors
/// The journal holds an operation that runs or is unconfirmed (A8.5); the study has no current model or candidate (B5.5); the
/// isolated simulation is not ready (E8.2); or the validation fails.
pub(crate) fn run_stage(
    context: &OperationContext,
    project_root: &Path,
    storage_folder: &Path,
    pending: &Arc<PendingReleases>,
    loop_definition: &LoopDefinition,
    connection: &RuntimeConnection,
) -> Result<Value, Failure> {
    let journal = Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    refuse_while_journaled(&journal)?;
    let work = ValidateWork {
        project_root: project_root.to_path_buf(),
        storage_folder: storage_folder.to_path_buf(),
        pending: Arc::clone(pending),
        loop_definition: loop_definition.clone(),
    };
    run_validate(context, &work, connection)
}

/// The failure of a study that lost its model or its candidate between the checks and the run (`Cause::Implementation`).
fn study_changed() -> Failure {
    Failure::new(
        "The study changed while Validate started; start Validate again.",
        Cause::Implementation,
        NextAction::None,
    )
}

/// Refuses an operation while the loop's journal holds one that runs or whose outcome is unconfirmed (A8.5).
fn refuse_while_journaled(journal: &Journal) -> Result<(), Failure> {
    let ids: Vec<&str> = journal
        .entries()
        .iter()
        .filter(|entry| entry.holds_unfinished_operation())
        .map(JournalEntry::operation_id)
        .collect();
    unfinished_operation_refusal(&ids).map_or(Ok(()), Err)
}

/// Where the values of the runtime of `connection` come from (A2.4, A9), read when an operation starts.
fn runtime_source(connection: &RuntimeConnection) -> Result<EvidenceSource, Failure> {
    Ok(RuntimeClient::connect(connection)?.status()?.source())
}

/// The loop's working study (A8.2).
fn working_study(work: &ValidateWork) -> Result<Study, Failure> {
    storage::open_study(
        &work.project_root,
        &work.loop_definition.id,
        &work.loop_definition.revision,
    )
}

#[cfg(test)]
#[path = "validate_action_tests.rs"]
mod tests;
