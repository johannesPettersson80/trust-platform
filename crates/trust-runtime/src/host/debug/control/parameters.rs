//! Typed custom-controller parameter bindings and task-boundary transactions.
use super::program_facts;
use super::symbol_path::{self, ResolvedSymbol};
use super::transfer::{check_physical_review, check_target_source};
use super::{DebugControl, PendingVarTarget};
use crate::runtime::{Located, ProgramRead, ProgramWrite};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use trust_runtime_core::native::image::Image;

#[path = "parameters/control_pid.rs"]
mod control_pid;
#[path = "parameters/operations.rs"]
mod operations;
#[path = "parameters/session.rs"]
mod session;
pub(crate) use control_pid::{ControlPidOutcome, ControlPidView, ControlPidWrite};

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
    /// Each parameter's place and the value it held before the commit, in the order of the binding's parameters.
    restore_points: Vec<(Located, Value)>,
    /// What the owning task's end found (E7.4). The record takes it once the scan that ran the task is published
    /// ([`ParameterRegistry::settle`]): until then the published program still holds the values from before the
    /// commit, which status reports with the set pending (E7.2).
    confirmation: Option<Confirmation>,
    /// Whose set it is, which decides how its members are checked and written.
    kind: SetKind,
}

/// Whose set a pending set is (E7.1, E7.7).
#[derive(Clone, Debug)]
enum SetKind {
    /// A client's binding (`control.parameters.*`, E7.1): REAL and LREAL members within their declared bounds, written
    /// as numbers, and the binding's BOOL mode signal.
    Binding,
    /// A `Control.PID`'s parameter record (`control.pid.*`, E7.7): each field written in its own type, the mode the
    /// controller observed, and the controller's operating domain.
    ControlPid(control_pid::Set),
}

impl SetKind {
    /// The set's binding still holds at its task's boundary (E7.4): a client's is the binding its status reviewed
    /// and valid against the program of `image` (E7.1); a controller's record still belongs to the task.
    fn check_binding(
        &self,
        registry: &ParameterRegistry,
        request: &ApplyRequest,
        image: &Image<'_>,
    ) -> Result<(), String> {
        match self {
            Self::Binding => {
                let record = registry
                    .records
                    .get(&request.binding.loop_id)
                    .ok_or("binding unavailable")?;
                if record.binding_revision != request.expected_binding_revision {
                    return Err("binding changed before task boundary".into());
                }
                check_binding(&request.binding, image)
            }
            Self::ControlPid(set) => set.check_binding(&request.binding, image),
        }
    }

    /// Manual is observed now in `program` (E7.3 step 7, E7.7), `registry` knowing which tasks ran in the program.
    fn check_mode(
        &self,
        registry: &ParameterRegistry,
        binding: &Binding,
        program: &dyn ProgramRead,
    ) -> Result<(), String> {
        match self {
            Self::Binding => manual(binding, program),
            Self::ControlPid(set) => set.check_mode(
                program,
                &binding.task_name,
                registry.task_ran(&binding.task_name),
            ),
        }
    }

    /// The values the set writes are within their bounds (E7.3 step 10; specification 45 §2.4).
    fn validate(&self, binding: &Binding, candidate: &BTreeMap<String, f64>) -> Result<(), String> {
        match self {
            Self::Binding => validate_candidate(binding, candidate),
            Self::ControlPid(set) => set.validate(),
        }
    }

    /// The value `parameter` takes, `number` being its candidate number: a client's member in its REAL or LREAL type,
    /// a controller's field as the write sets it.
    fn value(&self, parameter: &Parameter, number: f64) -> Result<Value, String> {
        match self {
            Self::Binding => Ok(if parameter.iec_type == "REAL" {
                Value::Real(number as f32)
            } else {
                Value::LReal(number)
            }),
            Self::ControlPid(set) => set.value(&parameter.symbol),
        }
    }
}

/// What a committed set's task end found (E7.4).
#[derive(Clone, Debug)]
enum Confirmation {
    /// Every parameter holds the value Studio wrote.
    Applied,
    /// The program changed the parameters named, in the order of the binding.
    ChangedByProgram(String),
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
    /// How the controllers' writes ended (E7.7), which the controllers' status reports.
    control_pid: control_pid::Writes,
    /// The tasks that ran since the epoch began, by their names in upper case: a controller's record of its mode is the
    /// current program's only once its task ran in it (E7.7).
    tasks_run: HashSet<String>,
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
            control_pid: control_pid::Writes::default(),
            tasks_run: HashSet::new(),
        }
    }
}
fn hash(value: impl std::fmt::Debug) -> String {
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(format!("{value:?}").as_bytes()))
    )
}
/// Checks a binding against the program of `image` (E7.1): its parameters are writable members, and
/// every member belongs to a program instance that its declared task runs, or that no task runs for
/// `$background`.
fn check_binding(binding: &Binding, image: &Image<'_>) -> Result<(), String> {
    if binding.loop_id.trim().is_empty()
        || binding.parameters.is_empty()
        || binding.parameters.len() > MAX_PARAMETERS
    {
        return Err(format!(
            "a named loop needs 1 to {MAX_PARAMETERS} parameters"
        ));
    }
    let tasks = program_facts::program_tasks(image);
    let mut symbols = HashSet::new();
    for parameter in &binding.parameters {
        if !program_facts::parameter_writable(image, &parameter.symbol) {
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
        if !program_facts::is_program_instance(image, root) {
            return Err("parameter root is not a compiled program".into());
        }
        let owners: Vec<_> = tasks
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
fn values(binding: &Binding, program: &dyn ProgramRead) -> Result<BTreeMap<String, f64>, String> {
    binding
        .parameters
        .iter()
        .map(|p| Ok((p.symbol.clone(), parameter_number(program, p)?)))
        .collect()
}

/// A parameter's runtime value as a finite number of its declared IEC type; an enumeration member of a controller's
/// record (E7.7) as its variant's number.
fn parameter_number(program: &dyn ProgramRead, parameter: &Parameter) -> Result<f64, String> {
    let value = symbol_path::resolve(program, &parameter.symbol)?.value;
    let number = match (parameter.iec_type.as_str(), value) {
        ("LREAL", Value::LReal(v)) => v,
        ("REAL", Value::Real(v)) => f64::from(v),
        (control_pid::ENUMERATION, Value::Enum(v)) => v.numeric_value() as f64,
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

/// The variable a Control Studio symbol path names in `program` (`Main.Controller.Gain`), with its
/// place and its value there.
pub(crate) fn resolve_path(
    program: &dyn ProgramRead,
    symbol: &str,
) -> Result<ResolvedSymbol, String> {
    symbol_path::resolve(program, symbol)
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
    pub(crate) fn parameter_status(&self, params: JsonValue) -> Result<JsonValue, String> {
        let request: StatusRequest = serde_json::from_value(params).map_err(|e| e.to_string())?;
        // The record and the values are read under one lock (E7.2): the scan publishes the program
        // before it settles a confirmed set ([`Self::finish_transfer_cycle`]), so an outcome read
        // here comes with the values of the scan that decided it, or of a later one.
        let mut state = self.lock();
        let program = self
            .published_program()
            .ok_or("runtime snapshot unavailable")?;
        let image = program.program_image();
        check_binding(&request.binding, &image)?;
        let current = values(&request.binding, &*program)?;
        if !matches!(
            resolve_path(&*program, &request.binding.mode.symbol)?.value,
            Value::Bool(_)
        ) {
            return Err("mode must be BOOL".into());
        }
        let program_revision = state.parameters.program_revision(&image);
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
fn manual(binding: &Binding, program: &dyn ProgramRead) -> Result<(), String> {
    if resolve_path(program, &binding.mode.symbol)?.value != Value::Bool(binding.mode.manual_value)
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
    /// The pending set named by its kind and identifier, as a refusal names the live write that is running (A6.6,
    /// E9.1): `a parameter transaction <id>`, or `a controller write to <instance>`.
    pub(super) fn pending_label(&self) -> Option<String> {
        self.pending.as_ref().map(|pending| match &pending.kind {
            SetKind::Binding => {
                format!("a parameter transaction {}", pending.request.transaction_id)
            }
            SetKind::ControlPid(set) => format!("a controller write to {}", set.instance()),
        })
    }

    /// Records why `pending` ended without being applied: a client's record turns rejected with the reason (E7.4,
    /// A6.7); a controller's write keeps it for the controller's status (E7.7).
    fn reject(&mut self, pending: &Pending, reason: String) {
        match &pending.kind {
            SetKind::Binding => {
                if let Some(record) = self.records.get_mut(&pending.request.binding.loop_id) {
                    record.status = "rejected".into();
                    record.reason = Some(reason);
                }
            }
            SetKind::ControlPid(set) => self.control_pid.record(
                set,
                pending.request.transaction_id.clone(),
                ControlPidOutcome::Rejected(reason),
            ),
        }
    }
    /// The program revision (annex E1.1): from the session, which a reload or a restart ends and
    /// which started from a value fixed at the process's start, and the revision of the sources of
    /// the program of `image`.
    pub(super) fn program_revision(&self, image: &Image<'_>) -> String {
        hash((self.session, program_facts::source_revision(image)))
    }
    pub(super) fn snapshot_is_stale(&self) -> bool {
        self.snapshot_stale
    }

    /// Records the outcome of a parameter set that its task's end decided, once the scan that ran the task is
    /// published (E7.2, E7.4): applied, with the values it replaced retained for a rollback unless it was one, or
    /// rejected with the parameters the program changed. A set its task has not confirmed stays pending.
    pub(super) fn settle(&mut self) {
        let Some(Pending {
            request,
            candidate,
            rollback,
            confirmation: Some(confirmation),
            kind,
            ..
        }) = self
            .pending
            .take_if(|pending| pending.confirmation.is_some())
        else {
            return;
        };
        if let SetKind::ControlPid(set) = &kind {
            let outcome = match confirmation {
                Confirmation::Applied => ControlPidOutcome::Applied,
                Confirmation::ChangedByProgram(symbols) => ControlPidOutcome::Rejected(format!(
                    "controller changed parameter values before confirmation: {symbols}"
                )),
            };
            self.control_pid
                .record(set, request.transaction_id, outcome);
            return;
        }
        let Some(record) = self.records.get_mut(&request.binding.loop_id) else {
            return;
        };
        match confirmation {
            Confirmation::Applied => {
                record.previous = if rollback {
                    None
                } else {
                    Some(request.expected_values)
                };
                record.active = candidate;
                record.status = "applied".into();
                record.reason = None;
            }
            Confirmation::ChangedByProgram(symbols) => {
                record.previous = None;
                record.status = "rejected".into();
                record.reason = Some(format!(
                    "controller changed parameter values before confirmation: {symbols}"
                ));
            }
        }
    }

    /// Starts a new session at a reload or a restart. The identities and the reviews of the old session end with it.
    /// A set its task's end already confirmed keeps that outcome; a transaction that was still pending ends rejected
    /// with its reason, and every record keeps its status and reason, so status reports why it stopped (A6.7).
    /// Previous values end with the session that retained them.
    pub(super) fn interrupt(&mut self) {
        self.settle();
        self.session = session::new_session();
        self.used_operations.clear();
        self.cancelled_operations.clear();
        self.reviews.clear();
        if let Some(pending) = self.pending.take() {
            self.reject(
                &pending,
                "runtime interrupted before parameter confirmation".into(),
            );
        }
        for record in self.records.values_mut() {
            record.previous = None;
        }
        self.snapshot_stale = true;
        self.tasks_run.clear();
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

    /// Records the tasks that ran in a published scan of the epoch, each with the intervals it missed (E7.7).
    pub(super) fn record_task_runs(&mut self, tasks: &[(smol_str::SmolStr, u64)]) {
        self.tasks_run
            .extend(tasks.iter().map(|(name, _)| name.to_ascii_uppercase()));
    }

    /// Whether `task` ran since the epoch began, after the last reload or restart: a named task once a scan of the
    /// epoch ran it, `$background`, whose programs run in every scan, once a scan of the epoch completed (E7.7).
    pub(super) fn task_ran(&self, task: &str) -> bool {
        if task == "$background" {
            !self.snapshot_stale
        } else {
            self.tasks_run.contains(&task.to_ascii_uppercase())
        }
    }
}
impl DebugControl {
    /// Accepts a parameter transaction for the next task boundary of its owning task, against the
    /// program as the last scan left it. `physical_target` is the runtime's target fact (A2.4): the
    /// request's source label must agree with it, and a physical target needs approval.
    pub(crate) fn parameter_apply(
        &self,
        params: JsonValue,
        rollback: bool,
        physical_target: bool,
    ) -> Result<JsonValue, String> {
        let request: ApplyRequest = serde_json::from_value(params).map_err(|e| e.to_string())?;
        if !matches!(request.source.as_str(), "simulation" | "physical") {
            return Err("invalid parameter source".into());
        }
        check_target_source(&request.source, physical_target)?;
        check_physical_review(physical_target, request.physical_approved)?;
        let program = self.published_program();
        if let Some(program) = &program {
            check_binding(&request.binding, &program.program_image())?;
        }
        let mut state = self.lock();
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
        if let Some(pending) = state.parameters.pending_label() {
            return Err(format!("{pending} is pending"));
        }
        if let Some(pending) = state.transfers.pending_id() {
            return Err(format!("a controller transfer {pending} is pending"));
        }
        if !state.live_values.forced_vars.is_empty() || !state.live_values.forced_io.is_empty() {
            return Err("release active forcing before parameter application".into());
        }
        if state.parameters.snapshot_is_stale() {
            return Err(STALE_SNAPSHOT.into());
        }
        let program = program.ok_or("runtime snapshot unavailable")?;
        let current = values(&request.binding, &*program)?;
        manual(&request.binding, &*program)?;
        let program_revision = state.parameters.program_revision(&program.program_image());
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
            confirmation: None,
            kind: SetKind::Binding,
        });
        Ok(json!({"status":"pending","confirmed":false}))
    }

    /// The program was reloaded (an online change, specification 35 A6.7): Control Studio's runtime
    /// epoch ends as at a restart, the program is marked reloaded, so that an experiment it stops
    /// reports `program_reloaded`, and a queued write resolved against the program that ended is
    /// dropped. Global and retained names are looked up again when applied, so their writes stay.
    pub(crate) fn invalidate_control_program(&self) {
        let mut state = self.lock();
        state.end_control_epoch();
        state.parameters.mark_program_reloaded();
        state.live_values.pending_var_writes.retain(|write| {
            matches!(
                write.target,
                PendingVarTarget::Global(_) | PendingVarTarget::Retain(_)
            )
        });
    }

    /// The runtime epoch the current program runs in; a reload or a restart ends it.
    pub(crate) fn runtime_epoch(&self) -> u64 {
        self.lock().parameters.epoch()
    }

    /// The program revision that status reports and that every family result compares (A2.2), of
    /// the program as the last scan left it; none before a scan ended.
    pub(crate) fn program_revision(&self) -> Option<String> {
        let program = self.published_program()?;
        Some(
            self.lock()
                .parameters
                .program_revision(&program.program_image()),
        )
    }

    /// Whether the debug snapshot still holds the program before the last reload or restart.
    pub(crate) fn snapshot_is_stale(&self) -> bool {
        self.lock().parameters.snapshot_stale
    }

    /// Starts a control experiment in `epoch`, unless the runtime has moved on since the request read it.
    pub(crate) fn begin_experiment(&self, epoch: u64) -> bool {
        let mut state = self.lock();
        if state.parameters.epoch() != epoch {
            return false;
        }
        state.parameters.clear_reloaded();
        true
    }

    /// Why an experiment started in `epoch` must stop, or `None` while that epoch lasts.
    pub(crate) fn experiment_boundary(&self, epoch: u64) -> Option<&'static str> {
        let state = self.lock();
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
        let state = self.lock();
        state
            .transfers
            .pending_id()
            .map(|id| format!("a controller transfer {id}"))
            .or_else(|| state.parameters.pending_label())
    }

    /// Whether a parameter set waits for, or was committed at, its task's boundary.
    pub(crate) fn has_parameter_transaction(&self) -> bool {
        self.lock().parameters.pending.is_some()
    }

    /// Commits a pending parameter set at its owning task's start, before the task's programs run
    /// (E7.4): `task` is the task's name, `$background` for the programs outside every task, and
    /// `program` the program at that access point, written through the runtime's checks
    /// (specification 44 §13.3). The set is written whole or not at all. `physical_target` is the
    /// runtime's target fact: a physical target needs the approval that the request carries.
    pub(crate) fn commit_parameters(
        &self,
        task: &str,
        program: &mut dyn ProgramWrite,
        physical_target: bool,
    ) {
        let mut state = self.lock();
        let Some(mut pending) = state.parameters.pending.take() else {
            return;
        };
        // A set is committed once: one its task already wrote waits for its task's end, or for the
        // end of the scan that confirmed it.
        if pending.committed || !pending.request.binding.task_name.eq_ignore_ascii_case(task) {
            state.parameters.pending = Some(pending);
            return;
        }
        let request = &pending.request;
        let validate = (|| -> Result<Vec<(Located, Value, Value)>, String> {
            let image = program.program_image();
            if request.expected_program_revision != state.parameters.program_revision(&image) {
                return Err("program changed before task boundary".into());
            }
            pending
                .kind
                .check_binding(&state.parameters, request, &image)?;
            if physical_target && !request.physical_approved {
                return Err("attached I/O requires explicit physical review".into());
            }
            if !state.live_values.forced_vars.is_empty() || !state.live_values.forced_io.is_empty()
            {
                return Err("forcing changed before task boundary".into());
            }
            let bound = program_facts::bound_places(&image);
            for parameter in &request.binding.parameters {
                let at = resolve_path(&*program, &parameter.symbol)?.at;
                if bound.iter().any(|(place, _)| *place == at.place) {
                    return Err("addressed parameters require the I/O parameter adapter".into());
                }
            }
            pending
                .kind
                .check_mode(&state.parameters, &request.binding, &*program)?;
            let observed = values(&request.binding, &*program)?;
            if let Some(mismatch) = first_value_mismatch(&request.expected_values, &observed) {
                return Err(format!(
                    "parameter values changed before task boundary: {mismatch}"
                ));
            }
            pending
                .kind
                .validate(&request.binding, &pending.candidate)?;
            let mut planned = Vec::with_capacity(request.binding.parameters.len());
            for parameter in &request.binding.parameters {
                let ResolvedSymbol {
                    at,
                    value: previous,
                } = resolve_path(&*program, &parameter.symbol)?;
                let value = pending
                    .kind
                    .value(parameter, pending.candidate[&parameter.symbol])?;
                planned.push((at, previous, value));
            }
            Ok(planned)
        })();
        // The set is written together: a member the runtime refuses puts back the members
        // already written, so the task runs with the whole set or none of it.
        let committed = validate.and_then(|planned| {
            let mut restore_points: Vec<(Located, Value)> = Vec::with_capacity(planned.len());
            for ((at, previous, value), parameter) in
                planned.into_iter().zip(&request.binding.parameters)
            {
                if program.write_at(&at, &value).is_err() {
                    for (at, previous) in restore_points.iter().rev() {
                        let _ = program.write_at(at, previous);
                    }
                    return Err(format!("parameter {} is not writable", parameter.symbol));
                }
                restore_points.push((at, previous));
            }
            Ok(restore_points)
        });
        match committed {
            Ok(restore_points) => {
                pending.restore_points = restore_points;
                pending.committed = true;
                state.parameters.pending = Some(pending);
            }
            Err(reason) => state.parameters.reject(&pending, reason),
        }
    }

    /// Confirms a committed parameter set at its task's end, after the task's programs ran and
    /// before any output is published, `program` being the program at that access point (E7.4). A
    /// set the program changed is rejected: each member still holding Studio's value returns to the
    /// value it had before the apply, at once, and each member the program changed keeps the
    /// program's value, which the rejection names. The record takes the outcome once the scan is
    /// published ([`Self::finish_transfer_cycle`]), so status never reports an outcome with the
    /// values of the scan before it (E7.2).
    pub(crate) fn confirm_parameters(&self, task: &str, program: &mut dyn ProgramWrite) {
        let mut state = self.lock();
        let Some(mut pending) = state.parameters.pending.take() else {
            return;
        };
        if !pending.committed
            || pending.confirmation.is_some()
            || !pending.request.binding.task_name.eq_ignore_ascii_case(task)
        {
            state.parameters.pending = Some(pending);
            return;
        }
        let mut program_changed = Vec::new();
        for parameter in &pending.request.binding.parameters {
            if parameter_number(&*program, parameter) != Ok(pending.candidate[&parameter.symbol]) {
                program_changed.push(parameter.symbol.as_str());
            }
        }
        let confirmation = if program_changed.is_empty() {
            Confirmation::Applied
        } else {
            for (parameter, (at, previous)) in pending
                .request
                .binding
                .parameters
                .iter()
                .zip(&pending.restore_points)
            {
                if parameter_number(&*program, parameter)
                    == Ok(pending.candidate[&parameter.symbol])
                {
                    let _ = program.write_at(at, previous);
                }
            }
            Confirmation::ChangedByProgram(program_changed.join(", "))
        };
        pending.confirmation = Some(confirmation);
        state.parameters.pending = Some(pending);
    }
}
