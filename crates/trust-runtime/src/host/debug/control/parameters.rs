//! Typed custom-controller parameter bindings and task-boundary transactions.
use super::symbol_path::{self, ResolvedSymbol};
use super::transfer::{check_physical_review, check_target_source};
use super::{DebugControl, PendingVarTarget};
use crate::memory::VariableStorage;
use crate::runtime::RuntimeMetadata;
use crate::value::{Value, ValueRef};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};

#[path = "parameters/operations.rs"]
mod operations;
#[path = "parameters/session.rs"]
mod session;

/// Longest identifier a parameter transaction accepts, in characters (E7.3).
const MAX_IDENTIFIER_CHARS: usize = 128;
/// Transaction identities that one session epoch holds (E7.3 step 4).
const MAX_TRANSACTION_IDENTITIES: usize = 1024;
/// Most parameters that one binding names (E7.1).
const MAX_PARAMETERS: usize = 64;
/// Refusal of a live write while the debug snapshot still holds the program from before a reload or restart.
pub(super) const STALE_SNAPSHOT: &str =
    "runtime snapshot is stale: wait for a scan of the current session before a live write";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Parameter {
    symbol: String,
    iec_type: String,
    unit: String,
    convention: String,
    minimum: f64,
    maximum: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Mode {
    symbol: String,
    manual_value: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    loop_id: String,
    task_name: String,
    parameters: Vec<Parameter>,
    mode: Mode,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusRequest {
    binding: Binding,
}
/// The transaction record of one loop. A status read never writes it: only apply, commit, confirmation, cancel and a
/// restart do.
#[derive(Clone, Debug)]
struct Record {
    binding_revision: String,
    transaction_id: Option<String>,
    status: String,
    active: BTreeMap<String, f64>,
    previous: Option<BTreeMap<String, f64>>,
    reason: Option<String>,
}
impl Record {
    fn new(binding_revision: String, active: BTreeMap<String, f64>) -> Self {
        Self {
            binding_revision,
            transaction_id: None,
            status: "idle".into(),
            active,
            previous: None,
            reason: None,
        }
    }
}
/// The revisions that the last status read of a loop reviewed (E7.3 step 9).
#[derive(Clone, Debug)]
struct Review {
    binding_revision: String,
    program_revision: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyRequest {
    transaction_id: String,
    binding: Binding,
    expected_program_revision: String,
    expected_binding_revision: String,
    expected_active_revision: String,
    expected_values: BTreeMap<String, f64>,
    values: Option<BTreeMap<String, f64>>,
    previous_revision: Option<String>,
    source: String,
    physical_approved: bool,
}
#[derive(Clone, Debug)]
struct Pending {
    request: ApplyRequest,
    candidate: BTreeMap<String, f64>,
    rollback: bool,
    committed: bool,
    /// Each parameter's reference and the value it held before the commit, in the order of the binding's parameters.
    restore_points: Vec<(ValueRef, Value)>,
}
#[derive(Debug)]
pub(super) struct ParameterRegistry {
    used_operations: HashSet<String>,
    cancelled_operations: HashSet<String>,
    session: session::Session,
    records: HashMap<String, Record>,
    reviews: HashMap<String, Review>,
    pending: Option<Pending>,
    /// Set by a program reload and kept across the restart that usually follows it, until a control experiment
    /// starts: an experiment running across the reload stops with `program_reloaded`.
    reloaded: bool,
    /// Until a scan of the current epoch completes, the debug snapshot still holds the program before the boundary.
    snapshot_stale: bool,
}
impl Default for ParameterRegistry {
    fn default() -> Self {
        Self {
            session: session::new_session(),
            used_operations: HashSet::new(),
            cancelled_operations: HashSet::new(),
            records: HashMap::new(),
            reviews: HashMap::new(),
            pending: None,
            reloaded: false,
            snapshot_stale: false,
        }
    }
}
fn hash(value: impl std::fmt::Debug) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(format!("{value:?}").as_bytes())
    )
}
fn check_binding(binding: &Binding, metadata: &RuntimeMetadata) -> Result<(), String> {
    if binding.loop_id.trim().is_empty()
        || binding.parameters.is_empty()
        || binding.parameters.len() > MAX_PARAMETERS
    {
        return Err(format!(
            "a named loop needs 1 to {MAX_PARAMETERS} parameters"
        ));
    }
    let mut symbols = HashSet::new();
    for parameter in &binding.parameters {
        if !metadata.control_parameter_writable(&parameter.symbol) {
            return Err("parameter must be a writable, unaddressed, non-aliased member".into());
        }
        if !symbols.insert(parameter.symbol.to_ascii_uppercase())
            || parameter.symbol.eq_ignore_ascii_case(&binding.mode.symbol)
        {
            return Err("duplicate parameter or mode symbol".into());
        }
        if !matches!(parameter.iec_type.as_str(), "REAL" | "LREAL")
            || parameter.unit.trim().is_empty()
            || parameter.convention.trim().is_empty()
            || !parameter.minimum.is_finite()
            || !parameter.maximum.is_finite()
            || parameter.minimum > parameter.maximum
        {
            return Err("invalid parameter type, units, convention or bounds".into());
        }
    }
    for symbol in binding
        .parameters
        .iter()
        .map(|p| p.symbol.as_str())
        .chain(std::iter::once(binding.mode.symbol.as_str()))
    {
        let root = symbol.split('.').next().unwrap_or_default();
        if !metadata
            .programs()
            .keys()
            .any(|name| name.eq_ignore_ascii_case(root))
        {
            return Err("parameter root is not a compiled program".into());
        }
        let owners: Vec<_> = metadata
            .tasks()
            .iter()
            .filter(|task| {
                task.programs
                    .iter()
                    .any(|program| program.eq_ignore_ascii_case(root))
            })
            .collect();
        if binding.task_name == "$background" {
            if !owners.is_empty() {
                return Err("program is task-owned".into());
            }
        } else if owners.len() != 1 || !owners[0].name.eq_ignore_ascii_case(&binding.task_name) {
            return Err("parameter program must have exactly the declared owning task".into());
        }
    }
    Ok(())
}
fn values(binding: &Binding, storage: &VariableStorage) -> Result<BTreeMap<String, f64>, String> {
    binding
        .parameters
        .iter()
        .map(|p| Ok((p.symbol.clone(), parameter_number(storage, p)?)))
        .collect()
}

/// A parameter's runtime value as a finite number of its declared IEC type.
fn parameter_number(storage: &VariableStorage, parameter: &Parameter) -> Result<f64, String> {
    let value = symbol_path::resolve(storage, &parameter.symbol)?.value;
    let number = match (parameter.iec_type.as_str(), value) {
        ("LREAL", Value::LReal(v)) => v,
        ("REAL", Value::Real(v)) => f64::from(v),
        _ => return Err("parameter IEC type does not match runtime".into()),
    };
    if !number.is_finite() {
        return Err(format!(
            "{} has bad quality: runtime value is not a finite number",
            parameter.symbol
        ));
    }
    Ok(number)
}

pub(crate) fn resolve_path(
    storage: &VariableStorage,
    symbol: &str,
) -> Result<ResolvedSymbol, String> {
    symbol_path::resolve(storage, symbol)
}

/// The outcome of a loop as a status reports it.
struct Outcome {
    status: String,
    reason: Option<String>,
    transaction_id: Option<String>,
    previous: Option<BTreeMap<String, f64>>,
}

impl DebugControl {
    /// Reads the parameters of a loop. It changes only the review marker of the loop (E7.2): the transaction record is
    /// only read, so a status never resets a rejection, a retained previous value or a stopped transaction's reason.
    pub(crate) fn parameter_status(
        &self,
        params: JsonValue,
        metadata: &RuntimeMetadata,
    ) -> Result<JsonValue, String> {
        let request: StatusRequest = serde_json::from_value(params).map_err(|e| e.to_string())?;
        check_binding(&request.binding, metadata)?;
        let (lock, _) = &*self.state;
        let mut state = lock.lock().map_err(|_| "debug state unavailable")?;
        let snapshot = state
            .snapshot
            .as_ref()
            .ok_or("runtime snapshot unavailable")?;
        let current = values(&request.binding, &snapshot.storage)?;
        if !matches!(
            resolve_path(&snapshot.storage, &request.binding.mode.symbol)?.value,
            Value::Bool(_)
        ) {
            return Err("mode must be BOOL".into());
        }
        let program_revision = state.parameters.program_revision(metadata);
        let binding_revision =
            hash(serde_json::to_string(&request.binding).map_err(|e| e.to_string())?);
        state.parameters.reviews.insert(
            request.binding.loop_id.clone(),
            Review {
                binding_revision: binding_revision.clone(),
                program_revision: program_revision.clone(),
            },
        );
        let outcome = state
            .parameters
            .outcome(&request.binding.loop_id, &binding_revision);
        Ok(
            json!({"supports_request_id":true,"transaction_id":outcome.transaction_id,"program_revision":program_revision,"binding_revision":binding_revision,"active_revision":hash(&current),"values":current,"status":outcome.status,"previous_revision":outcome.previous.as_ref().map(hash),"previous_values":outcome.previous,"reason":outcome.reason}),
        )
    }
}

fn validate_candidate(binding: &Binding, candidate: &BTreeMap<String, f64>) -> Result<(), String> {
    if candidate.len() != binding.parameters.len() {
        return Err("exact parameter set required".into());
    }
    for parameter in &binding.parameters {
        let value = *candidate
            .get(&parameter.symbol)
            .ok_or("exact parameter set required")?;
        if !value.is_finite()
            || value < parameter.minimum
            || value > parameter.maximum
            || (parameter.iec_type == "REAL" && !(value as f32).is_finite())
        {
            return Err(format!(
                "{} outside declared bounds or IEC range: {value} is not within [{}, {}]",
                parameter.symbol, parameter.minimum, parameter.maximum
            ));
        }
        if parameter.iec_type == "REAL" && f64::from(value as f32) != value {
            return Err(
                "REAL values must be explicitly rounded to their runtime representation".into(),
            );
        }
    }
    Ok(())
}
fn manual(binding: &Binding, storage: &VariableStorage) -> Result<(), String> {
    if resolve_path(storage, &binding.mode.symbol)?.value != Value::Bool(binding.mode.manual_value)
    {
        return Err("confirmed Manual mode required".into());
    }
    Ok(())
}

/// The first parameter whose expected value differs from the value now, named with both values (A11.1).
fn first_value_mismatch(
    expected: &BTreeMap<String, f64>,
    current: &BTreeMap<String, f64>,
) -> Option<String> {
    for (symbol, value) in current {
        match expected.get(symbol) {
            Some(expected_value) if expected_value == value => {}
            Some(expected_value) => {
                return Some(format!(
                    "{symbol} expected {expected_value}, observed {value}"
                ))
            }
            None => return Some(format!("{symbol} has no expected value, observed {value}")),
        }
    }
    expected
        .keys()
        .find(|symbol| !current.contains_key(*symbol))
        .map(|symbol| format!("{symbol} is not a declared parameter"))
}

/// The apply's expected revisions and values must be the current ones (E7.3 step 8). A mismatch names the stale value.
fn check_expected(
    request: &ApplyRequest,
    program_revision: &str,
    binding_revision: &str,
    current: &BTreeMap<String, f64>,
) -> Result<(), String> {
    if request.expected_program_revision != program_revision {
        return Err(format!(
            "stale program, binding, or expected values: program revision expected {}, current {program_revision}",
            request.expected_program_revision
        ));
    }
    if request.expected_binding_revision != binding_revision {
        return Err(format!(
            "stale program, binding, or expected values: binding revision expected {}, current {binding_revision}",
            request.expected_binding_revision
        ));
    }
    let active = hash(current);
    if request.expected_active_revision != active {
        return Err(format!(
            "stale program, binding, or expected values: active revision expected {}, current {active}",
            request.expected_active_revision
        ));
    }
    match first_value_mismatch(&request.expected_values, current) {
        Some(mismatch) => Err(format!(
            "stale program, binding, or expected values: {mismatch}"
        )),
        None => Ok(()),
    }
}

/// The values a rollback restores. The reviewed binding must own the retained values, and the parameters must still
/// hold the active values (E7.5).
fn rollback_target(
    registry: &ParameterRegistry,
    request: &ApplyRequest,
    binding_revision: &str,
    current: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, String> {
    let record = registry
        .records
        .get(&request.binding.loop_id)
        .filter(|record| record.binding_revision == binding_revision)
        .ok_or("rollback unavailable")?;
    let previous = record.previous.clone().ok_or("rollback unavailable")?;
    if record.active != *current {
        return Err(
            "stale rollback revision: the values no longer match the active revision".into(),
        );
    }
    let retained = hash(&previous);
    if request.previous_revision.as_ref() != Some(&retained) {
        return Err(format!(
            "stale rollback revision: previous revision expected {}, retained {retained}",
            request.previous_revision.as_deref().unwrap_or("none")
        ));
    }
    Ok(previous)
}

impl ParameterRegistry {
    /// The identifier of the pending parameter transaction, when one is pending (A6.6).
    pub(super) fn pending_id(&self) -> Option<&str> {
        self.pending
            .as_ref()
            .map(|pending| pending.request.transaction_id.as_str())
    }
    pub(super) fn program_revision(&self, metadata: &RuntimeMetadata) -> String {
        hash((self.session, metadata.control_program_revision()))
    }
    pub(super) fn snapshot_is_stale(&self) -> bool {
        self.snapshot_stale
    }

    /// Starts a new session at a reload or a restart. The identities and the reviews of the old session end with it.
    /// A transaction that was still pending ends rejected with its reason, and every record keeps its status and
    /// reason, so status reports why it stopped (A6.7). Previous values end with the session that retained them.
    pub(super) fn interrupt(&mut self) {
        self.session = session::new_session();
        self.used_operations.clear();
        self.cancelled_operations.clear();
        self.reviews.clear();
        if let Some(pending) = self.pending.take() {
            if let Some(record) = self.records.get_mut(&pending.request.binding.loop_id) {
                record.status = "rejected".into();
                record.reason = Some("runtime interrupted before parameter confirmation".into());
            }
        }
        for record in self.records.values_mut() {
            record.previous = None;
        }
        self.snapshot_stale = true;
    }

    /// The outcome of a loop, shown only for the binding that owns the record.
    fn outcome(&self, loop_id: &str, binding_revision: &str) -> Outcome {
        match self
            .records
            .get(loop_id)
            .filter(|record| record.binding_revision == binding_revision)
        {
            Some(record) => Outcome {
                status: record.status.clone(),
                reason: record.reason.clone(),
                transaction_id: record.transaction_id.clone(),
                previous: record.previous.clone(),
            },
            None => Outcome {
                status: "idle".into(),
                reason: None,
                transaction_id: None,
                previous: None,
            },
        }
    }

    pub(super) fn epoch(&self) -> u64 {
        self.session.2
    }

    pub(super) fn mark_program_reloaded(&mut self) {
        self.reloaded = true;
    }

    fn clear_reloaded(&mut self) {
        self.reloaded = false;
    }

    /// Called once the epoch's first scan has refreshed the debug snapshot.
    pub(super) fn mark_snapshot_current(&mut self) {
        self.snapshot_stale = false;
    }
}
impl DebugControl {
    /// Accepts a parameter transaction for the next task boundary of its owning task. `physical_target` is the
    /// runtime's target fact (A2.4): the request's source label must agree with it, and a physical target needs approval.
    pub(crate) fn parameter_apply(
        &self,
        params: JsonValue,
        metadata: &RuntimeMetadata,
        rollback: bool,
        physical_target: bool,
    ) -> Result<JsonValue, String> {
        let request: ApplyRequest = serde_json::from_value(params).map_err(|e| e.to_string())?;
        if !matches!(request.source.as_str(), "simulation" | "physical") {
            return Err("invalid parameter source".into());
        }
        check_target_source(&request.source, physical_target)?;
        check_physical_review(physical_target, request.physical_approved)?;
        check_binding(&request.binding, metadata)?;
        let (lock, _) = &*self.state;
        let mut state = lock.lock().map_err(|_| "debug state unavailable")?;
        let id = &request.transaction_id;
        if id.is_empty()
            || id.chars().count() > MAX_IDENTIFIER_CHARS
            || state.parameters.used_operations.contains(id)
        {
            return Err("parameter identity is invalid, already used, or cancelled".into());
        }
        if state.parameters.used_operations.len() >= MAX_TRANSACTION_IDENTITIES {
            return Err(format!(
                "parameter identity capacity reached ({MAX_TRANSACTION_IDENTITIES})"
            ));
        }
        if let Some(pending) = state.parameters.pending_id() {
            return Err(format!("a parameter transaction {pending} is pending"));
        }
        if let Some(pending) = state.transfers.pending_id() {
            return Err(format!("a controller transfer {pending} is pending"));
        }
        if !state.forced_vars.is_empty() || !state.forced_io.is_empty() {
            return Err("release active forcing before parameter application".into());
        }
        if state.parameters.snapshot_is_stale() {
            return Err(STALE_SNAPSHOT.into());
        }
        let snapshot = state
            .snapshot
            .as_ref()
            .ok_or("runtime snapshot unavailable")?;
        let current = values(&request.binding, &snapshot.storage)?;
        manual(&request.binding, &snapshot.storage)?;
        let program_revision = state.parameters.program_revision(metadata);
        let binding_revision =
            hash(serde_json::to_string(&request.binding).map_err(|e| e.to_string())?);
        check_expected(&request, &program_revision, &binding_revision, &current)?;
        let reviewed = state
            .parameters
            .reviews
            .get(&request.binding.loop_id)
            .ok_or("read parameter status before apply")?;
        if reviewed.binding_revision != binding_revision
            || reviewed.program_revision != program_revision
        {
            return Err("binding changed since review".into());
        }
        let candidate = if rollback {
            if request.values.is_some() {
                return Err("rollback cannot supply replacement values".into());
            }
            rollback_target(&state.parameters, &request, &binding_revision, &current)?
        } else {
            if request.previous_revision.is_some() {
                return Err("apply cannot supply previous_revision".into());
            }
            request.values.clone().ok_or("candidate values required")?
        };
        validate_candidate(&request.binding, &candidate)?;
        let record = state
            .parameters
            .records
            .entry(request.binding.loop_id.clone())
            .or_insert_with(|| Record::new(binding_revision.clone(), current.clone()));
        if record.binding_revision != binding_revision {
            *record = Record::new(binding_revision.clone(), current.clone());
        }
        record.status = "pending".into();
        record.reason = None;
        record.transaction_id = Some(request.transaction_id.clone());
        state
            .parameters
            .used_operations
            .insert(request.transaction_id.clone());
        state.parameters.pending = Some(Pending {
            request,
            candidate,
            rollback,
            committed: false,
            restore_points: Vec::new(),
        });
        Ok(json!({"status":"pending","confirmed":false}))
    }
    pub(crate) fn invalidate_control_program(&self) {
        let mut state = self.state.0.lock().expect("debug state poisoned");
        state.parameters.interrupt();
        state.parameters.mark_program_reloaded();
        state.transfers.interrupt();
        state.recordings.interrupt();
        state.signals.clear();
        // A queued write resolved against the program that ended names its variable only in that program, so a
        // reference, an instance or a frame is dropped. Global and retained names are looked up again when applied.
        state.pending_var_writes.retain(|write| {
            matches!(
                write.target,
                PendingVarTarget::Global(_) | PendingVarTarget::Retain(_)
            )
        });
        state
            .pending_lvalue_writes
            .retain(|write| write.frame_id.is_none());
    }

    /// The runtime epoch the current program runs in; a reload or a restart ends it.
    pub(crate) fn runtime_epoch(&self) -> u64 {
        self.state
            .0
            .lock()
            .expect("debug state poisoned")
            .parameters
            .epoch()
    }

    /// The program revision that status reports and that every family result compares (A2.2). The caller holds the
    /// metadata lock first; this takes the debug lock second, the only order in which the two are taken.
    pub(crate) fn program_revision(&self, metadata: &RuntimeMetadata) -> String {
        self.state
            .0
            .lock()
            .expect("debug state poisoned")
            .parameters
            .program_revision(metadata)
    }

    /// Whether the debug snapshot still holds the program before the last reload or restart.
    pub(crate) fn snapshot_is_stale(&self) -> bool {
        self.state
            .0
            .lock()
            .expect("debug state poisoned")
            .parameters
            .snapshot_stale
    }

    /// Starts a control experiment in `epoch`, unless the runtime has moved on since the request read it.
    pub(crate) fn begin_experiment(&self, epoch: u64) -> bool {
        let mut state = self.state.0.lock().expect("debug state poisoned");
        if state.parameters.epoch() != epoch {
            return false;
        }
        state.parameters.clear_reloaded();
        true
    }

    /// Why an experiment started in `epoch` must stop, or `None` while that epoch lasts.
    pub(crate) fn experiment_boundary(&self, epoch: u64) -> Option<&'static str> {
        let state = self.state.0.lock().expect("debug state poisoned");
        if state.parameters.epoch() == epoch {
            None
        } else if state.parameters.reloaded {
            Some("program_reloaded")
        } else {
            Some("runtime_restarted")
        }
    }
    /// The live write that is pending, named by kind and identifier (A6.6), or `None` when none is pending.
    pub(crate) fn pending_transaction_label(&self) -> Option<String> {
        let state = self.state.0.lock().expect("debug state poisoned");
        state
            .transfers
            .pending_id()
            .map(|id| format!("a controller transfer {id}"))
            .or_else(|| {
                state
                    .parameters
                    .pending_id()
                    .map(|id| format!("a parameter transaction {id}"))
            })
    }
    pub(crate) fn has_parameter_transaction(&self) -> bool {
        self.state
            .0
            .lock()
            .expect("debug state poisoned")
            .parameters
            .pending
            .is_some()
    }
    /// Commits a pending parameter set at its owning task's boundary, before the task runs (E7.4). `physical_target` is
    /// the runtime's target fact: a physical target needs the approval that the request carries.
    pub(crate) fn commit_parameters(
        &self,
        task: &str,
        storage: &mut VariableStorage,
        metadata: &RuntimeMetadata,
        physical_target: bool,
        io_bindings: &[crate::io::IoBinding],
    ) {
        let mut state = self.state.0.lock().expect("debug state poisoned");
        let Some(mut pending) = state.parameters.pending.take() else {
            return;
        };
        if !pending.request.binding.task_name.eq_ignore_ascii_case(task) {
            state.parameters.pending = Some(pending);
            return;
        }
        let request = &pending.request;
        let validate = (|| -> Result<(VariableStorage, Vec<(ValueRef, Value)>), String> {
            if request.expected_program_revision
                != hash((
                    state.parameters.session,
                    metadata.control_program_revision(),
                ))
            {
                return Err("program changed before task boundary".into());
            }
            let record = state
                .parameters
                .records
                .get(&request.binding.loop_id)
                .ok_or("binding unavailable")?;
            if record.binding_revision != request.expected_binding_revision {
                return Err("binding changed before task boundary".into());
            }
            check_binding(&request.binding, metadata)?;
            if physical_target && !request.physical_approved {
                return Err("attached I/O requires explicit physical review".into());
            }
            if !state.forced_vars.is_empty() || !state.forced_io.is_empty() {
                return Err("forcing changed before task boundary".into());
            }
            for parameter in &request.binding.parameters {
                let reference = resolve_path(storage, &parameter.symbol)?.reference;
                if io_bindings.iter().any(|binding| match &binding.target {
                    crate::io::IoTarget::Name(name) => name.eq_ignore_ascii_case(&parameter.symbol),
                    crate::io::IoTarget::Reference(value) => value == &reference,
                }) {
                    return Err("addressed parameters require the I/O parameter adapter".into());
                }
            }
            manual(&request.binding, storage)?;
            let observed = values(&request.binding, storage)?;
            if let Some(mismatch) = first_value_mismatch(&request.expected_values, &observed) {
                return Err(format!(
                    "parameter values changed before task boundary: {mismatch}"
                ));
            }
            validate_candidate(&request.binding, &pending.candidate)?;
            let mut staged = storage.clone();
            let mut restore_points = Vec::new();
            for parameter in &request.binding.parameters {
                let ResolvedSymbol {
                    reference,
                    value: previous,
                } = resolve_path(&staged, &parameter.symbol)?;
                restore_points.push((reference.clone(), previous));
                let value = pending.candidate[&parameter.symbol];
                if !staged.write_by_ref(
                    reference,
                    if parameter.iec_type == "REAL" {
                        Value::Real(value as f32)
                    } else {
                        Value::LReal(value)
                    },
                ) {
                    return Err(format!("parameter {} is not writable", parameter.symbol));
                }
            }
            Ok((staged, restore_points))
        })();
        match validate {
            Ok((staged, restore_points)) => {
                *storage = staged;
                pending.restore_points = restore_points;
                pending.committed = true;
                state.parameters.pending = Some(pending);
            }
            Err(reason) => {
                if let Some(record) = state.parameters.records.get_mut(&request.binding.loop_id) {
                    record.status = "rejected".into();
                    record.reason = Some(reason);
                }
            }
        }
    }
    /// Confirms a committed parameter set after the task's programs ran. A set the program changed is rejected before
    /// any output is published: each member still holding Studio's value returns to the value it had before the apply,
    /// and each member the program changed keeps the program's value, which the rejection names.
    pub(crate) fn confirm_parameters(
        &self,
        task: &str,
        storage: &mut VariableStorage,
        now: crate::value::Duration,
    ) {
        let mut state = self.state.0.lock().expect("debug state poisoned");
        let Some(pending) = state.parameters.pending.take() else {
            return;
        };
        if !pending.committed || !pending.request.binding.task_name.eq_ignore_ascii_case(task) {
            state.parameters.pending = Some(pending);
            return;
        }
        let mut program_changed = Vec::new();
        for parameter in &pending.request.binding.parameters {
            if parameter_number(storage, parameter) != Ok(pending.candidate[&parameter.symbol]) {
                program_changed.push(parameter.symbol.as_str());
            }
        }
        if program_changed.is_empty() {
            if let Some(record) = state
                .parameters
                .records
                .get_mut(&pending.request.binding.loop_id)
            {
                record.previous = if pending.rollback {
                    None
                } else {
                    Some(pending.request.expected_values.clone())
                };
                record.active = pending.candidate.clone();
                record.status = "applied".into();
                record.reason = None;
            }
        } else {
            for (parameter, (reference, previous)) in pending
                .request
                .binding
                .parameters
                .iter()
                .zip(&pending.restore_points)
            {
                if parameter_number(storage, parameter) == Ok(pending.candidate[&parameter.symbol])
                {
                    storage.write_by_ref_ref(reference, previous.clone());
                }
            }
            if let Some(record) = state
                .parameters
                .records
                .get_mut(&pending.request.binding.loop_id)
            {
                record.previous = None;
                record.status = "rejected".into();
                record.reason = Some(format!(
                    "controller changed parameter values before confirmation: {}",
                    program_changed.join(", ")
                ));
            }
        }
        state.snapshot = Some(crate::debug::DebugSnapshot {
            storage: storage.clone(),
            now,
        });
    }
}
