//! The parameter writes of Control Studio, `controlStudio/parameters` (specification 35, C12 and §0.1a; annex E7, E11.2 and E11.8). A
//! loop has one of two tools ([`parameter_tool`]): the parameters its declared analysis maps to program symbols, or the settings of its
//! experiment (`parameterPurpose: "experiment"`), which are the loop's own binding. The engine reviews the values of the tool's
//! symbols, writes them, and restores the values from before, through the runtime's parameter transaction. The rules are trust-control's
//! (`analysis::parameter_writes`, `commissioning::parameter_application`, and the tool's offer and outcome in `messages`). This module
//! reads the loop and the runtime, journals each identifier before its write (E11.2) and answers the page. A write starts as an
//! operation that holds the loop (A8.5); its final answer says how it ended. Each answer names the tool's offer: the step the page's
//! button runs next and the actions the engine takes now.
//!
//! The settings are frozen while a project test holds them (C12, Parameter purpose): a review or a write is refused while an operation
//! of this engine works on the loop (the test runs), and while the loop's journal holds an unfinished operation (the test's
//! restoration is not confirmed). They are released once the test's terminal restoration is confirmed, and `released` says so.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use trust_control::analysis::parameter_writes::{
    selected_values, writable_binding, writable_parameters, write_identity, written_changes,
    written_record, ParameterWriteAction, WritableParameter, WrittenValue,
};
use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::parameter_application::{
    review_input, review_of, rollback_review, values_of_text, ParameterReview, ReviewKind,
    RuntimeBinding,
};
use trust_control::commissioning::parameter_status::{ParameterStatus, RecordState};
use trust_control::commissioning::texts;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::connected_run::runtime_required;
use trust_control::identification::project_test::{
    parameter_tool, ParameterTool, SETTINGS_RELEASED,
};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{
    parameter_tool_offer, write_outcome, ParameterWorkAction, ParameterWorkAnswer,
    ParameterWorkRequest,
};
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;

use crate::apply_action::started;
use crate::busy_loops::{LoopClaim, LoopClaims};
use crate::commands::to_value;
use crate::identify_action::refuse_while_journaled;
use crate::journal::Journal;
use crate::loops;
use crate::operation::{OperationContext, Operations};
use crate::parameter_write_journal::WriteJournal;
use crate::runtime_client::parameters::{runtime_binding, ParameterTransactions};
use crate::runtime_client::{new_transaction_id, RuntimeClient};

/// The name of the write's operation in the operation journal.
const WRITE_COMMAND: &str = "parameters";
/// The name of the write's operation that holds the loop, as a refusal names it (A8.5).
const WRITE_OPERATION: &str = "Parameter write";
/// The name of a review that holds the loop while it is made, as a refusal names it (A8.5).
const REVIEW_OPERATION: &str = "Parameter review";
/// The name of the question whether the settings are released, as a refusal names it (A8.5).
const RELEASE_OPERATION: &str = "Settings release check";
/// The name of the question whether the settings are free, which the tool's offer follows, as a refusal names it (A8.5).
const OFFER_OPERATION: &str = "Parameter tool check";
/// The refusal of a write while the runtime's record holds a pending transaction (E7.4).
const PENDING: &str =
    "A parameter write is still pending. Wait for it to finish before another write.";
/// The refusal of a write whose record changed since its review; the review is asked for again (A2.5, E11.8).
const REVIEW_CHANGED: &str =
    "The parameter record or the loop changed since the review. Review the values again before the write.";
/// The refusal of a loop without a parameter binding, which the runtime's record needs.
const NO_BINDING: &str = "Declare the ST parameter binding before application.";
/// The refusal of a draft of the experiment's settings: no analysis selects a value for them.
const NO_DRAFT: &str =
    "The experiment settings have no analysis to select values. Inspect the settings, then edit them.";
/// What an `inspect` answered, and what the engineer does next (C12).
const INSPECTED: &str = "Current values loaded. Edit them, then review the exact change.";
/// What the review of an Apply answered, and what the engineer does next (C12).
const APPLY_REVIEWED: &str = "Review the exact values before applying them.";
/// What the review of a Restore answered, and what the engineer does next (C12, E7.5).
const RESTORE_REVIEWED: &str =
    "Review the values from before the last write before restoring them.";

/// Answers `controlStudio/parameters` (C12): `draft`, `inspect`, `review`, `reviewRestore`, `invalidate` and `released` at once, and
/// `confirm` as the operation it starts.
///
/// # Errors
/// The loop cannot be opened or is not served by the engine; it has no tool, or its declared analysis does not map its parameters; no
/// runtime is connected for an action that reads or writes it; the runtime refuses or cannot be read; the review no longer matches the
/// record; an operation of this engine works on the loop, or its journal holds an unfinished operation, so the settings are frozen
/// (C12); or the experiment's settings are asked for a draft.
pub fn answer(
    operations: &Operations,
    claims: &LoopClaims,
    request: ParameterWorkRequest,
) -> Result<Value, Failure> {
    let loop_definition = served_loop(&request)?;
    match request.action {
        ParameterWorkAction::Draft => draft(&loop_definition),
        ParameterWorkAction::Inspect => inspect(claims, &loop_definition, &request),
        ParameterWorkAction::Review => review_apply(claims, &loop_definition, &request),
        ParameterWorkAction::ReviewRestore => review_restore(claims, &loop_definition, &request),
        ParameterWorkAction::Confirm => start(operations, claims, loop_definition, request),
        ParameterWorkAction::Invalidate => invalidate(claims, &loop_definition, &request),
        ParameterWorkAction::Released => released(claims, &loop_definition, &request),
    }
}

/// The loop the request names, opened and served by the engine: the parameter writes belong to a loop whose tool the engine serves
/// (C12), and a parked loop stays on the `TypeScript` engine.
fn served_loop(request: &ParameterWorkRequest) -> Result<LoopDefinition, Failure> {
    let loop_definition = loops::open(
        Path::new(&request.project_root),
        &request.runtime_id,
        &request.loop_id,
    )?;
    loop_definition.rust_engine_support()?;
    Ok(loop_definition)
}

/// The parameters the loop's tool lets a write set, in the order the page lists them (C12): the parameters a declared analysis maps,
/// with the bounds the analysis allows, or the experiment's settings, which are every parameter of the loop's binding with the bounds it
/// declares. A loop with no tool is refused as the analysis's check refuses it.
///
/// # Errors
/// The loop has no tool, or the tool's parameters are not valid.
fn tool_parameters(loop_definition: &LoopDefinition) -> Result<Vec<WritableParameter>, Failure> {
    match parameter_tool(loop_definition) {
        Some(ParameterTool::Experiment) => experiment_settings(loop_definition),
        Some(ParameterTool::Analysis) | None => writable_parameters(loop_definition),
    }
}

/// The experiment's settings (C12, Parameter purpose): every parameter of the loop's binding, with its own type, unit and bounds. No
/// analysis selects a value for them, so `selected` holds none and a `draft` is refused before it reads it.
fn experiment_settings(
    loop_definition: &LoopDefinition,
) -> Result<Vec<WritableParameter>, Failure> {
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .ok_or_else(missing_binding)?;
    Ok(binding
        .parameters
        .iter()
        .map(|parameter| WritableParameter {
            name: parameter.symbol.clone(),
            symbol: parameter.symbol.clone(),
            iec_type: parameter.iec_type,
            unit: parameter.unit.clone(),
            minimum: parameter.minimum,
            maximum: parameter.maximum,
            selected: f64::NAN,
        })
        .collect())
}

/// The refusal of a loop without a parameter binding.
fn missing_binding() -> Failure {
    Failure::new(NO_BINDING, Cause::Settings, NextAction::OpenLoopSetup)
}

/// The values that the declared analysis selects, by symbol (`draft`). No runtime is read. The experiment's settings have no analysis.
fn draft(loop_definition: &LoopDefinition) -> Result<Value, Failure> {
    if parameter_tool(loop_definition) == Some(ParameterTool::Experiment) {
        return Err(Failure::new(NO_DRAFT, Cause::Settings, NextAction::None));
    }
    let writable = writable_parameters(loop_definition)?;
    to_value(&ParameterWorkAnswer {
        values: Some(selected_values(&writable)),
        binding: writable_binding(loop_definition, &writable),
        message: Some(texts::parameter_selected_analysis_draft()),
        ..ParameterWorkAnswer::default()
    })
}

/// The page edited the values after a review (`invalidate`, C12): the review is spent, and the answer is the sentence the page shows
/// with the tool's offer, which follows the loop's claim and journal. Nothing is written, and no runtime is read.
///
/// # Errors
/// The loop's journal cannot be read.
fn invalidate(
    claims: &LoopClaims,
    loop_definition: &LoopDefinition,
    request: &ParameterWorkRequest,
) -> Result<Value, Failure> {
    let free = settings_free(claims, loop_definition, &request.storage_folder)?;
    to_value(&ParameterWorkAnswer {
        message: Some(texts::parameter_edited()),
        offer: Some(parameter_tool_offer(free, false)),
        ..ParameterWorkAnswer::default()
    })
}

/// Whether a project test left the loop's settings released (`released`, C12, Parameter purpose). A test holds the settings from its
/// start until its terminal restoration is confirmed, so they are free exactly when no operation of this engine works on the loop and the
/// loop's journal holds no unfinished operation: a test that ended with its restoration confirmed, or with none needed, left neither,
/// whether it finished, was aborted or failed. The answer is the sentence that says the settings are released, or the reason they are
/// still held, with the tool's offer that follows. A loop with no tool has no settings to release and answers neither. Nothing is
/// written, and no runtime is read.
fn released(
    claims: &LoopClaims,
    loop_definition: &LoopDefinition,
    request: &ParameterWorkRequest,
) -> Result<Value, Failure> {
    if parameter_tool(loop_definition).is_none() {
        return to_value(&ParameterWorkAnswer::default());
    }
    let held = hold_settings(
        claims,
        loop_definition,
        &request.storage_folder,
        RELEASE_OPERATION,
    );
    to_value(&match held {
        Ok(_free) => ParameterWorkAnswer {
            settings_released: Some(SETTINGS_RELEASED.to_owned()),
            offer: Some(parameter_tool_offer(true, false)),
            ..ParameterWorkAnswer::default()
        },
        Err(frozen) => ParameterWorkAnswer {
            message: Some(frozen.detail.unwrap_or(frozen.message)),
            offer: Some(parameter_tool_offer(false, false)),
            ..ParameterWorkAnswer::default()
        },
    })
}

/// Holds the loop for a review, a write or the question whether the settings are released, when the settings are free (C12, Parameter
/// purpose). They are frozen while an operation of this engine works on the loop, which the claim refuses and names, and while the
/// loop's journal holds an unfinished operation, which a project test leaves until its restoration is confirmed (A8.5, A7). The claim
/// ends when the returned guard is dropped.
///
/// # Errors
/// An operation of this engine works on the loop, or the loop's journal holds an unfinished operation or cannot be read.
fn hold_settings(
    claims: &LoopClaims,
    loop_definition: &LoopDefinition,
    storage_folder: &str,
    operation: &str,
) -> Result<LoopClaim, Failure> {
    let claim = claims.claim(&loop_definition.project_id, &loop_definition.id, operation)?;
    let journal = Journal::open(
        Path::new(storage_folder),
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    refuse_while_journaled(&journal)?;
    Ok(claim)
}

/// Whether the loop's settings are free now, for the tool's offer (C12): no operation of this engine works on the loop, and its journal
/// holds no unfinished operation. Nothing is held: the claim taken to ask ends at once.
///
/// # Errors
/// The loop's journal cannot be read.
fn settings_free(
    claims: &LoopClaims,
    loop_definition: &LoopDefinition,
    storage_folder: &str,
) -> Result<bool, Failure> {
    let Ok(_claim) = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        OFFER_OPERATION,
    ) else {
        return Ok(false);
    };
    let journal = Journal::open(
        Path::new(storage_folder),
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    Ok(refuse_while_journaled(&journal).is_ok())
}

/// The tool's values and the record that the runtime holds now (`inspect`), with the tool's offer, which follows the loop's claim and
/// journal: a review while the settings are free, and only another inspection while a project test holds them (C12). Nothing is
/// written.
fn inspect(
    claims: &LoopClaims,
    loop_definition: &LoopDefinition,
    request: &ParameterWorkRequest,
) -> Result<Value, Failure> {
    let writable = tool_parameters(loop_definition)?;
    let mut target = connect(request.connection.as_ref())?;
    let binding = binding_of(loop_definition)?;
    let observed =
        ParameterTransactions::new(&mut target.client, binding, target.source).status()?;
    let free = settings_free(claims, loop_definition, &request.storage_folder)?;
    to_value(&ParameterWorkAnswer {
        values: Some(mapped_values(&writable, &observed.values)),
        observed: Some(observed),
        binding: writable_binding(loop_definition, &writable),
        message: Some(INSPECTED.to_owned()),
        offer: Some(parameter_tool_offer(free, false)),
        ..ParameterWorkAnswer::default()
    })
}

/// The values that a `review` proposes: the text the engineer typed (`json`), which the engine reads and refuses with its own sentence,
/// else the request's `values`. The page sends the text as it is and keeps no reading of it (C12).
///
/// # Errors
/// The text is longer than 64 kB, is not JSON, or is not an object of numbers.
fn proposed_values(request: &ParameterWorkRequest) -> Result<BTreeMap<String, f64>, Failure> {
    match request.json.as_deref() {
        Some(text) => values_of_text(Some(text)),
        None => Ok(request.values.clone()),
    }
}

/// The review of an Apply of the request's values (`review`). Nothing is written; the answer names the identity that the confirmation
/// names and the values the review read, which the confirmation sends back. The loop is held while the review is made, and a loop
/// whose settings are frozen is refused (C12).
fn review_apply(
    claims: &LoopClaims,
    loop_definition: &LoopDefinition,
    request: &ParameterWorkRequest,
) -> Result<Value, Failure> {
    // The text is read before the runtime is: a text that is no object of numbers needs no connection to be refused.
    let proposed = proposed_values(request)?;
    let _held = hold_settings(
        claims,
        loop_definition,
        &request.storage_folder,
        REVIEW_OPERATION,
    )?;
    let writable = tool_parameters(loop_definition)?;
    let binding = binding_of(loop_definition)?;
    let mut target = connect(request.connection.as_ref())?;
    let source = target.source;
    let mut transactions = ParameterTransactions::new(&mut target.client, binding.clone(), source);
    let journal = WriteJournal::of(Path::new(&request.storage_folder), loop_definition);
    reconcile(&mut transactions, &journal)?;
    let observed = transactions.status()?;
    let planned = plan_apply(
        loop_definition,
        &writable,
        &binding,
        observed,
        &proposed,
        String::new(),
    )?;
    review_answer(planned, source, Some(proposed), APPLY_REVIEWED)
}

/// The review of a Restore of the values that the runtime retains from before the last write (`reviewRestore`, E7.5). The loop is held
/// while the review is made, and a loop whose settings are frozen is refused (C12).
fn review_restore(
    claims: &LoopClaims,
    loop_definition: &LoopDefinition,
    request: &ParameterWorkRequest,
) -> Result<Value, Failure> {
    let _held = hold_settings(
        claims,
        loop_definition,
        &request.storage_folder,
        REVIEW_OPERATION,
    )?;
    let writable = tool_parameters(loop_definition)?;
    let binding = binding_of(loop_definition)?;
    let mut target = connect(request.connection.as_ref())?;
    let source = target.source;
    let mut transactions = ParameterTransactions::new(&mut target.client, binding, source);
    let journal = WriteJournal::of(Path::new(&request.storage_folder), loop_definition);
    reconcile(&mut transactions, &journal)?;
    let observed = transactions.status()?;
    let planned = plan_restore(loop_definition, &writable, observed, String::new())?;
    review_answer(planned, source, None, RESTORE_REVIEWED)
}

/// Starts the write of the review that the request names (`confirm`), as an operation that holds the loop until it ends (A8.5). The
/// write needs the identity of its review, and the runtime. A loop whose settings are frozen is refused (C12).
fn start(
    operations: &Operations,
    claims: &LoopClaims,
    loop_definition: LoopDefinition,
    request: ParameterWorkRequest,
) -> Result<Value, Failure> {
    let claim = hold_settings(
        claims,
        &loop_definition,
        &request.storage_folder,
        WRITE_OPERATION,
    )?;
    let identity = request.review.ok_or_else(|| {
        Failure::new(
            "Review the parameter values before the write starts.",
            Cause::Settings,
            NextAction::None,
        )
    })?;
    let connection = request.connection.ok_or_else(runtime_required)?;
    let write = Write {
        loop_definition,
        identity,
        values: request.values,
        storage_folder: PathBuf::from(request.storage_folder),
        connection,
        physical_approved: request.physical_approved,
    };
    started(operations, claim, WRITE_COMMAND, write, run_write)
}

/// One write to confirm: the loop, the identity of the review it confirms, the values an Apply's review read, and the runtime it goes to.
struct Write {
    loop_definition: LoopDefinition,
    identity: String,
    values: BTreeMap<String, f64>,
    storage_folder: PathBuf,
    connection: RuntimeConnection,
    physical_approved: bool,
}

/// The write of the review that the start names (E7.3 to E7.5, C12). It reports its progress in the engine's words while it waits for
/// the task boundary, and its final answer says how it ended: applied, with the record the runtime confirmed, or failed, with the
/// failure's sentence (`write_outcome`); and it names the tool's offer after it: the review is spent, and the claim the write holds
/// ends with it.
fn run_write(context: &OperationContext, write: &Write) -> Result<Value, Failure> {
    context.progress(&texts::parameter_applying_reason());
    let offer = Some(parameter_tool_offer(true, false));
    let answer = match confirmed_write(write) {
        Ok(confirmed) => {
            let (status, message) = write_outcome(confirmed.rollback, Ok(()));
            ParameterWorkAnswer {
                values: Some(mapped_values(&confirmed.writable, &confirmed.record.values)),
                observed: Some(confirmed.record),
                physical: Some(confirmed.source == ApplySource::Physical),
                status: Some(status),
                message: Some(message),
                offer,
                ..ParameterWorkAnswer::default()
            }
        }
        Err(failure) => {
            let (status, message) = write_outcome(false, Err(&failure));
            ParameterWorkAnswer {
                status: Some(status),
                message: Some(message),
                offer,
                ..ParameterWorkAnswer::default()
            }
        }
    };
    to_value(&answer)
}

/// A write the runtime confirmed: whether it restored the values from before, the record it was confirmed on, the tool's parameters and
/// the target it went to.
struct Confirmed {
    rollback: bool,
    record: ParameterStatus,
    writable: Vec<WritableParameter>,
    source: ApplySource,
}

/// The write of the review that the start names, confirmed (E7.3 to E7.5): the record is read and reviewed again, and the write is the
/// one whose review gives the named identity now ([`named_write`], E11.8). The identifier is journaled before the write is dispatched
/// (E11.2), and the write is confirmed on the record that the runtime reports. A refused or unconfirmed write keeps its journal, which
/// the next request reconciles.
///
/// # Errors
/// The loop has no tool or binding; the runtime cannot be reached or holds a pending write; no review gives the named identity now;
/// the journal cannot be written; or the runtime refuses the write or does not report the written values after the task executed.
fn confirmed_write(write: &Write) -> Result<Confirmed, Failure> {
    let writable = tool_parameters(&write.loop_definition)?;
    let binding = binding_of(&write.loop_definition)?;
    let mut target = connect(Some(&write.connection))?;
    let source = target.source;
    let mut transactions = ParameterTransactions::new(&mut target.client, binding.clone(), source);
    let journal = WriteJournal::of(&write.storage_folder, &write.loop_definition);
    reconcile(&mut transactions, &journal)?;
    let observed = transactions.status()?;
    let planned = named_write(write, &writable, &binding, observed, &new_transaction_id())?;
    journal.record(&planned.review.transaction_id, planned.action)?;
    let rollback = planned.review.is_rollback();
    let record = if rollback {
        transactions.rollback(&planned.review, write.physical_approved)?
    } else {
        transactions.apply(&planned.review, write.physical_approved)?
    };
    journal.clear()?;
    Ok(Confirmed {
        rollback,
        record,
        writable,
        source,
    })
}

/// The write that the review a confirmation names stands for now (C12, E11.8): the Apply of the values that an Apply's review read,
/// when its review of the record gives the named identity, else the Restore of the values that the runtime retains (E7.5), when its
/// review does. The identity binds the action, so the two never give the same one.
///
/// # Errors
/// Neither review gives the named identity: the record or the loop changed since the review (`REVIEW_CHANGED`), or the Apply's own
/// review is refused, with its refusal (a pending record, a value outside its bounds).
fn named_write(
    write: &Write,
    writable: &[WritableParameter],
    binding: &RuntimeBinding,
    observed: ParameterStatus,
    transaction_id: &str,
) -> Result<Planned, Failure> {
    let mut apply_refusal = None;
    if !write.values.is_empty() {
        match plan_apply(
            &write.loop_definition,
            writable,
            binding,
            observed.clone(),
            &write.values,
            transaction_id.to_owned(),
        ) {
            Ok(planned) if planned.identity == write.identity => return Ok(planned),
            Ok(_) => {}
            Err(refusal) => apply_refusal = Some(refusal),
        }
    }
    if let Ok(planned) = plan_restore(
        &write.loop_definition,
        writable,
        observed,
        transaction_id.to_owned(),
    ) {
        if planned.identity == write.identity {
            return Ok(planned);
        }
    }
    Err(apply_refusal.unwrap_or_else(review_changed))
}

/// What a write would do, read from the record that the runtime reports: the review the runtime confirms, what it does, its identity
/// (A2.5, E11.8) and the values it changes.
struct Planned {
    review: ParameterReview,
    action: ParameterWriteAction,
    identity: String,
    changes: Vec<WrittenValue>,
}

/// The review of an Apply of `proposed`: the whole record it would set, checked by the analysis bounds and the binding (E7.3, C0). The
/// experiment's settings are the whole binding, so `proposed` is the record itself, and the binding's own check refuses a value that
/// does not name exactly the declared symbols, or lies outside its declared bounds after IEC rounding.
fn plan_apply(
    loop_definition: &LoopDefinition,
    writable: &[WritableParameter],
    binding: &RuntimeBinding,
    observed: ParameterStatus,
    proposed: &BTreeMap<String, f64>,
    transaction_id: String,
) -> Result<Planned, Failure> {
    let record = match parameter_tool(loop_definition) {
        Some(ParameterTool::Experiment) => proposed.clone(),
        Some(ParameterTool::Analysis) | None => {
            written_record(writable, &observed.values, proposed)?
        }
    };
    let checked = review_input(binding, &record)?;
    let review = review_of(checked, observed, transaction_id)?;
    Ok(planned(loop_definition, writable, review))
}

/// The review of a Restore: the values that the runtime retains from before the last write (E7.5).
fn plan_restore(
    loop_definition: &LoopDefinition,
    writable: &[WritableParameter],
    observed: ParameterStatus,
    transaction_id: String,
) -> Result<Planned, Failure> {
    let review = rollback_review(observed, transaction_id)?;
    Ok(planned(loop_definition, writable, review))
}

/// The identity and the changes of a review (A2.5): the write's action and the revision a Restore returns to decide the identity.
fn planned(
    loop_definition: &LoopDefinition,
    writable: &[WritableParameter],
    review: ParameterReview,
) -> Planned {
    let (action, previous) = match &review.kind {
        ReviewKind::Apply => (ParameterWriteAction::Apply, None),
        ReviewKind::Rollback { previous_revision } => (
            ParameterWriteAction::Restore,
            Some(previous_revision.as_str()),
        ),
    };
    let identity = write_identity(
        loop_definition,
        action,
        &review.observed,
        previous,
        writable,
        &review.values,
    );
    let changes = written_changes(writable, &review.observed.values, &review.values);
    Planned {
        review,
        action,
        identity,
        changes,
    }
}

/// The answer of a review: the identity that the confirmation names, the values the review read (an Apply's; a Restore has none), what
/// the write changes, whether the target is physical, so that the write needs the engineer's approval (A2.5), the sentence the page
/// shows, and the tool's offer: the confirmation of this review is the next step (C12).
fn review_answer(
    planned: Planned,
    source: ApplySource,
    values: Option<BTreeMap<String, f64>>,
    message: &str,
) -> Result<Value, Failure> {
    to_value(&ParameterWorkAnswer {
        values,
        identity: Some(planned.identity),
        changes: planned.changes,
        physical: Some(source == ApplySource::Physical),
        message: Some(message.to_owned()),
        offer: Some(parameter_tool_offer(true, true)),
        ..ParameterWorkAnswer::default()
    })
}

/// The refusal of a write that no review of the record gives the identity of now: the record or the loop changed since the review, which
/// is asked for again (E11.8).
fn review_changed() -> Failure {
    Failure::new(REVIEW_CHANGED, Cause::Approval, NextAction::None)
}

/// Reads the journal of the loop before a review or a write (E11.4). A journaled write whose record is no longer pending is over, so its
/// journal is cleared; a pending record refuses the request, because the runtime holds one write at a time (E7.3).
fn reconcile(
    transactions: &mut ParameterTransactions<'_>,
    journal: &WriteJournal,
) -> Result<(), Failure> {
    if journal.read()?.is_none() {
        return Ok(());
    }
    if transactions.status()?.status == RecordState::Pending {
        return Err(Failure::new(PENDING, Cause::Runtime, NextAction::Wait));
    }
    journal.clear()
}

/// The target of the request's runtime: its client, and where its writes go (E7.3, A2.4).
struct Target {
    client: RuntimeClient,
    source: ApplySource,
}

/// Connects to the runtime of the request, and reads whether it is a simulated or a physical target.
fn connect(connection: Option<&RuntimeConnection>) -> Result<Target, Failure> {
    let connection = connection.ok_or_else(runtime_required)?;
    let mut client = RuntimeClient::connect(connection)?;
    let source = match client.status()?.source() {
        EvidenceSource::SimulatedPlant => ApplySource::Simulation,
        EvidenceSource::PhysicalPlant => ApplySource::Physical,
    };
    Ok(Target { client, source })
}

/// The loop's parameter binding, in the runtime's form (E7.1).
fn binding_of(loop_definition: &LoopDefinition) -> Result<RuntimeBinding, Failure> {
    loop_definition
        .parameter_binding
        .as_ref()
        .map(runtime_binding)
        .ok_or_else(missing_binding)
}

/// The values of the tool's symbols in `values`, in the order of the writable parameters.
fn mapped_values(
    writable: &[WritableParameter],
    values: &BTreeMap<String, f64>,
) -> BTreeMap<String, f64> {
    writable
        .iter()
        .filter_map(|parameter| {
            values
                .get(&parameter.symbol)
                .map(|value| (parameter.symbol.clone(), *value))
        })
        .collect()
}

#[cfg(test)]
#[path = "analysis_parameters_action_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "parameter_release_tests.rs"]
mod release_tests;

#[cfg(test)]
#[path = "parameter_tool_tests.rs"]
mod tool_tests;
