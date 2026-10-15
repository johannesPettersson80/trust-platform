//! The analysis parameter writes of Control Studio, `controlStudio/parameters` (specification 35, C12 and §0.1a; annex E7, E11.2 and
//! E11.8). A loop whose declared analysis maps parameters to program symbols lets the engine review the values of those symbols, then
//! write them, and restore the values from before, through the runtime's parameter transaction. The rules are trust-control's
//! (`analysis::parameter_writes`, `commissioning::parameter_application`). This module reads the loop and the runtime, journals each
//! identifier before its write (E11.2) and answers the page. A write starts as an operation that holds the loop (A8.5).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use trust_control::analysis::parameter_writes::{
    selected_values, writable_binding, writable_parameters, write_identity, written_changes,
    written_record, ParameterWriteAction, WritableParameter, WrittenValue,
};
use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::parameter_application::{
    review_input, review_of, rollback_review, ParameterReview, ReviewKind, RuntimeBinding,
};
use trust_control::commissioning::parameter_status::{ParameterStatus, RecordState};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::connected_run::runtime_required;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{ParameterWorkAction, ParameterWorkAnswer, ParameterWorkRequest};
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;

use crate::apply_action::started;
use crate::busy_loops::LoopClaims;
use crate::commands::to_value;
use crate::loops;
use crate::operation::{OperationContext, Operations};
use crate::parameter_write_journal::WriteJournal;
use crate::runtime_client::parameters::{runtime_binding, ParameterTransactions};
use crate::runtime_client::{new_transaction_id, RuntimeClient};

/// The name of the write's operation in the operation journal.
const WRITE_COMMAND: &str = "parameters";
/// The name of the write's operation that holds the loop, as a refusal names it (A8.5).
const WRITE_OPERATION: &str = "Analysis parameter write";
/// The refusal of a write while the runtime's record holds a pending transaction (E7.4).
const PENDING: &str =
    "A parameter write is still pending. Wait for it to finish before another write.";
/// The refusal of a write whose record changed since its review; the review is asked for again (A2.5, E11.8).
const REVIEW_CHANGED: &str =
    "The parameter record or the loop changed since the review. Review the values again before the write.";
/// The refusal of a loop without a parameter binding, which the runtime's record needs.
const NO_BINDING: &str = "Declare the ST parameter binding before application.";

/// Answers `controlStudio/parameters` (C12): `draft`, `inspect`, `review` and `reviewRestore` at once, and `apply` and `restore` as
/// the operation they start.
///
/// # Errors
/// The loop cannot be opened or is not served by the engine; its declared analysis does not map its parameters; no runtime is
/// connected for an action that reads or writes it; the runtime refuses or cannot be read; the review no longer matches the record;
/// or an operation of this engine works on the loop.
pub fn answer(
    operations: &Operations,
    claims: &LoopClaims,
    request: ParameterWorkRequest,
) -> Result<Value, Failure> {
    let loop_definition = served_loop(&request)?;
    match request.action {
        ParameterWorkAction::Draft => draft(&loop_definition),
        ParameterWorkAction::Inspect => inspect(&loop_definition, &request),
        ParameterWorkAction::Review => review_apply(&loop_definition, &request),
        ParameterWorkAction::ReviewRestore => review_restore(&loop_definition, &request),
        ParameterWorkAction::Apply => start(
            operations,
            claims,
            loop_definition,
            request,
            ParameterWriteAction::Apply,
        ),
        ParameterWorkAction::Restore => start(
            operations,
            claims,
            loop_definition,
            request,
            ParameterWriteAction::Restore,
        ),
    }
}

/// The loop the request names, opened and served by the engine: the parameter writes belong to a loop whose declared analysis the
/// engine serves (C12), and a parked loop stays on the `TypeScript` engine.
fn served_loop(request: &ParameterWorkRequest) -> Result<LoopDefinition, Failure> {
    let loop_definition = loops::open(
        Path::new(&request.project_root),
        &request.runtime_id,
        &request.loop_id,
    )?;
    loop_definition.rust_engine_support()?;
    Ok(loop_definition)
}

/// The values that the declared analysis selects, by symbol (`draft`). No runtime is read.
fn draft(loop_definition: &LoopDefinition) -> Result<Value, Failure> {
    let writable = writable_parameters(loop_definition)?;
    to_value(&ParameterWorkAnswer {
        values: Some(selected_values(&writable)),
        binding: writable_binding(loop_definition, &writable),
        ..ParameterWorkAnswer::default()
    })
}

/// The mapped values and the record that the runtime holds now (`inspect`). Nothing is written.
fn inspect(
    loop_definition: &LoopDefinition,
    request: &ParameterWorkRequest,
) -> Result<Value, Failure> {
    let writable = writable_parameters(loop_definition)?;
    let mut target = connect(request.connection.as_ref())?;
    let binding = binding_of(loop_definition)?;
    let observed =
        ParameterTransactions::new(&mut target.client, binding, target.source).status()?;
    to_value(&ParameterWorkAnswer {
        values: Some(mapped_values(&writable, &observed.values)),
        observed: Some(observed),
        binding: writable_binding(loop_definition, &writable),
        ..ParameterWorkAnswer::default()
    })
}

/// The review of an Apply of the request's values (`review`). Nothing is written; the answer names the identity that the start confirms.
fn review_apply(
    loop_definition: &LoopDefinition,
    request: &ParameterWorkRequest,
) -> Result<Value, Failure> {
    let writable = writable_parameters(loop_definition)?;
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
        &request.values,
        String::new(),
    )?;
    review_answer(planned, source)
}

/// The review of a Restore of the values that the runtime retains from before the last write (`reviewRestore`, E7.5).
fn review_restore(
    loop_definition: &LoopDefinition,
    request: &ParameterWorkRequest,
) -> Result<Value, Failure> {
    let writable = writable_parameters(loop_definition)?;
    let binding = binding_of(loop_definition)?;
    let mut target = connect(request.connection.as_ref())?;
    let source = target.source;
    let mut transactions = ParameterTransactions::new(&mut target.client, binding, source);
    let journal = WriteJournal::of(Path::new(&request.storage_folder), loop_definition);
    reconcile(&mut transactions, &journal)?;
    let observed = transactions.status()?;
    let planned = plan_restore(loop_definition, &writable, observed, String::new())?;
    review_answer(planned, source)
}

/// Starts the confirmed write that the request names, as an operation that holds the loop until it ends (A8.5). The write needs the
/// identity of its review, and the runtime.
fn start(
    operations: &Operations,
    claims: &LoopClaims,
    loop_definition: LoopDefinition,
    request: ParameterWorkRequest,
    action: ParameterWriteAction,
) -> Result<Value, Failure> {
    let claim = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
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
        action,
        identity,
        values: request.values,
        storage_folder: PathBuf::from(request.storage_folder),
        connection,
        physical_approved: request.physical_approved,
    };
    started(operations, claim, WRITE_COMMAND, write, run_write)
}

/// One confirmed write: the loop, what it does, the identity of the review it confirms, the values, and the runtime it goes to.
struct Write {
    loop_definition: LoopDefinition,
    action: ParameterWriteAction,
    identity: String,
    values: BTreeMap<String, f64>,
    storage_folder: PathBuf,
    connection: RuntimeConnection,
    physical_approved: bool,
}

/// The confirmed write (E7.3 to E7.5): the record is read and reviewed again, and the write goes ahead only when that review gives the
/// identity that the start named (E11.8). The identifier is journaled before the write is dispatched (E11.2), and the write is confirmed
/// on the record that the runtime reports. A refused or unconfirmed write keeps its journal, which the next request reconciles.
fn run_write(_context: &OperationContext, write: &Write) -> Result<Value, Failure> {
    let writable = writable_parameters(&write.loop_definition)?;
    let binding = binding_of(&write.loop_definition)?;
    let mut target = connect(Some(&write.connection))?;
    let source = target.source;
    let mut transactions = ParameterTransactions::new(&mut target.client, binding.clone(), source);
    let journal = WriteJournal::of(&write.storage_folder, &write.loop_definition);
    reconcile(&mut transactions, &journal)?;
    let observed = transactions.status()?;
    let transaction_id = new_transaction_id();
    let planned = match write.action {
        ParameterWriteAction::Apply => plan_apply(
            &write.loop_definition,
            &writable,
            &binding,
            observed,
            &write.values,
            transaction_id,
        )?,
        ParameterWriteAction::Restore => {
            plan_restore(&write.loop_definition, &writable, observed, transaction_id)?
        }
    };
    confirm_identity(&planned.identity, &write.identity)?;
    journal.record(&planned.review.transaction_id, write.action)?;
    let confirmed = match write.action {
        ParameterWriteAction::Apply => {
            transactions.apply(&planned.review, write.physical_approved)?
        }
        ParameterWriteAction::Restore => {
            transactions.rollback(&planned.review, write.physical_approved)?
        }
    };
    journal.clear()?;
    to_value(&ParameterWorkAnswer {
        values: Some(mapped_values(&writable, &confirmed.values)),
        observed: Some(confirmed),
        physical: Some(source == ApplySource::Physical),
        ..ParameterWorkAnswer::default()
    })
}

/// What a write would do, read from the record that the runtime reports: the review the runtime confirms, its identity (A2.5, E11.8)
/// and the values it changes.
struct Planned {
    review: ParameterReview,
    identity: String,
    changes: Vec<WrittenValue>,
}

/// The review of an Apply of `proposed`: the whole record it would set, checked by the analysis bounds and the binding (E7.3, C0).
fn plan_apply(
    loop_definition: &LoopDefinition,
    writable: &[WritableParameter],
    binding: &RuntimeBinding,
    observed: ParameterStatus,
    proposed: &BTreeMap<String, f64>,
    transaction_id: String,
) -> Result<Planned, Failure> {
    let record = written_record(writable, &observed.values, proposed)?;
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
        identity,
        changes,
    }
}

/// The answer of a review: the identity that the start names, what the write changes, and whether the target is physical, so that
/// the write needs the engineer's approval (A2.5).
fn review_answer(planned: Planned, source: ApplySource) -> Result<Value, Failure> {
    to_value(&ParameterWorkAnswer {
        identity: Some(planned.identity),
        changes: planned.changes,
        physical: Some(source == ApplySource::Physical),
        ..ParameterWorkAnswer::default()
    })
}

/// Refuses a write whose review no longer describes the record: the identity that the start names must be the one the review gives now
/// (E11.8).
fn confirm_identity(now: &str, named: &str) -> Result<(), Failure> {
    if now == named {
        return Ok(());
    }
    Err(Failure::new(
        REVIEW_CHANGED,
        Cause::Approval,
        NextAction::None,
    ))
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
        .ok_or_else(|| Failure::new(NO_BINDING, Cause::Settings, NextAction::OpenLoopSetup))
}

/// The values of the mapped symbols in `values`, in the order of the writable parameters.
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
