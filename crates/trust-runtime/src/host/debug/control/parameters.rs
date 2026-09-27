//! Typed custom-controller parameter bindings and task-boundary transactions.
use super::symbol_path::{self, ResolvedSymbol};
use super::DebugControl;
use crate::memory::{InstanceId, VariableStorage};
use crate::runtime::RuntimeMetadata;
use crate::value::Value;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "parameters/operations.rs"]
mod operations;

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
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
#[derive(Clone, Debug)]
struct Record {
    transaction_id: Option<String>,
    binding_revision: String,
    program_revision: String,
    status: String,
    active: BTreeMap<String, f64>,
    previous: Option<BTreeMap<String, f64>>,
    reason: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyRequest {
    transaction_id: Option<String>,
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
}
#[derive(Debug)]
pub(super) struct ParameterRegistry {
    used_operations: HashSet<String>,
    cancelled_operations: HashSet<String>,
    session: (u128, u32, u64),
    records: HashMap<String, Record>,
    pending: Option<Pending>,
}
impl Default for ParameterRegistry {
    fn default() -> Self {
        Self {
            session: (
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_nanos()),
                std::process::id(),
                NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            ),
            used_operations: HashSet::new(),
            cancelled_operations: HashSet::new(),
            records: HashMap::new(),
            pending: None,
        }
    }
}
fn hash(value: impl std::fmt::Debug) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(format!("{value:?}").as_bytes())
    )
}
pub(crate) fn resolve(
    storage: &VariableStorage,
    symbol: &str,
) -> Result<(InstanceId, String, Value), String> {
    let mut parts = symbol.split('.');
    let root = parts.next().ok_or("empty symbol")?;
    let Value::Instance(mut id) = storage
        .get_global(root)
        .ok_or("program instance not found")?
    else {
        return Err("symbol must belong to a program instance".into());
    };
    let mut rest = parts.peekable();
    while let Some(part) = rest.next() {
        let value = storage
            .get_instance_var(id, part)
            .ok_or_else(|| format!("symbol {symbol} not found"))?;
        if rest.peek().is_none() {
            return Ok((id, part.into(), value.clone()));
        }
        let Value::Instance(next) = value else {
            return Err("only instance paths are supported".into());
        };
        id = *next;
    }
    Err("symbol must name an instance member".into())
}
fn check_binding(binding: &Binding, metadata: &RuntimeMetadata) -> Result<(), String> {
    if binding.loop_id.trim().is_empty()
        || binding.parameters.is_empty()
        || binding.parameters.len() > 64
    {
        return Err("a named loop needs 1 to 64 parameters".into());
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
        .map(|p| {
            let value = symbol_path::resolve(storage, &p.symbol)?.value;
            let number = match (p.iec_type.as_str(), value) {
                ("LREAL", Value::LReal(v)) => v,
                ("REAL", Value::Real(v)) => f64::from(v),
                _ => return Err("parameter IEC type does not match runtime".into()),
            };
            if !number.is_finite() {
                return Err("runtime parameter is nonfinite".into());
            }
            Ok((p.symbol.clone(), number))
        })
        .collect()
}

pub(crate) fn resolve_path(
    storage: &VariableStorage,
    symbol: &str,
) -> Result<ResolvedSymbol, String> {
    symbol_path::resolve(storage, symbol)
}
impl DebugControl {
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
        let binding_revision =
            hash(serde_json::to_string(&request.binding).map_err(|e| e.to_string())?);
        let program_revision = hash((
            state.parameters.session,
            metadata.control_program_revision(),
        ));
        let record = state
            .parameters
            .records
            .entry(request.binding.loop_id)
            .or_insert_with(|| Record {
                transaction_id: None,
                binding_revision: binding_revision.clone(),
                program_revision: program_revision.clone(),
                status: "idle".into(),
                active: current.clone(),
                previous: None,
                reason: None,
            });
        if record.binding_revision != binding_revision
            || record.program_revision != program_revision
        {
            *record = Record {
                transaction_id: None,
                binding_revision: binding_revision.clone(),
                program_revision: program_revision.clone(),
                status: "idle".into(),
                active: current.clone(),
                previous: None,
                reason: None,
            };
        }
        if record.active != current && record.status != "pending" {
            record.previous = None;
            record.active = current.clone();
            if record.status != "rejected" {
                record.status = "idle".into();
            }
        }
        Ok(
            json!({"supports_request_id":true,"transaction_id":record.transaction_id,"program_revision":program_revision,"binding_revision":binding_revision,"active_revision":hash(&current),"values":current,"status":record.status,"previous_revision":record.previous.as_ref().map(hash),"previous_values":record.previous,"reason":record.reason}),
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
                "{} outside declared bounds or IEC range",
                parameter.symbol
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
impl ParameterRegistry {
    pub(super) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn program_revision(&self, metadata: &RuntimeMetadata) -> String {
        hash((self.session, metadata.control_program_revision()))
    }

    pub(super) fn interrupt(&mut self) {
        *self = Self::default();
    }
}
impl DebugControl {
    pub(crate) fn parameter_apply(
        &self,
        params: JsonValue,
        metadata: &RuntimeMetadata,
        rollback: bool,
    ) -> Result<JsonValue, String> {
        let request: ApplyRequest = serde_json::from_value(params).map_err(|e| e.to_string())?;
        if !matches!(request.source.as_str(), "simulation" | "physical") {
            return Err("invalid parameter source".into());
        }
        if request.source == "physical" && !request.physical_approved {
            return Err("physical review required".into());
        }
        check_binding(&request.binding, metadata)?;
        let (lock, _) = &*self.state;
        let mut state = lock.lock().map_err(|_| "debug state unavailable")?;
        if let Some(id) = &request.transaction_id {
            if id.is_empty() || id.len() > 128 || state.parameters.used_operations.contains(id) {
                return Err("parameter identity is invalid, already used, or cancelled".into());
            }
            if state.parameters.used_operations.len() >= 1024 {
                return Err("parameter identity capacity reached".into());
            }
        }
        if state.parameters.pending.is_some() || state.transfers.has_pending() {
            return Err("a parameter transaction is pending".into());
        }
        if !state.forced_vars.is_empty() || !state.forced_io.is_empty() {
            return Err("release active forcing before parameter application".into());
        }
        let snapshot = state
            .snapshot
            .as_ref()
            .ok_or("runtime snapshot unavailable")?;
        let current = values(&request.binding, &snapshot.storage)?;
        manual(&request.binding, &snapshot.storage)?;
        let program_revision = hash((
            state.parameters.session,
            metadata.control_program_revision(),
        ));
        let binding_revision =
            hash(serde_json::to_string(&request.binding).map_err(|e| e.to_string())?);
        if request.expected_program_revision != program_revision
            || request.expected_binding_revision != binding_revision
            || request.expected_active_revision != hash(&current)
            || request.expected_values != current
        {
            return Err("stale program, binding, or expected values".into());
        }
        let record = state
            .parameters
            .records
            .get_mut(&request.binding.loop_id)
            .ok_or("read parameter status before apply")?;
        if record.binding_revision != binding_revision
            || record.program_revision != program_revision
        {
            return Err("binding changed since review".into());
        }
        let candidate = if rollback {
            if request.values.is_some() {
                return Err("rollback cannot supply replacement values".into());
            }
            let previous = record.previous.clone().ok_or("rollback unavailable")?;
            if record.active != current
                || request.previous_revision.as_ref() != Some(&hash(&previous))
            {
                return Err("stale rollback revision".into());
            }
            previous
        } else {
            if request.previous_revision.is_some() {
                return Err("apply cannot supply previous_revision".into());
            }
            request.values.clone().ok_or("candidate values required")?
        };
        validate_candidate(&request.binding, &candidate)?;
        record.status = "pending".into();
        record.reason = None;
        record.transaction_id = request.transaction_id.clone();
        if let Some(id) = &request.transaction_id {
            state.parameters.used_operations.insert(id.clone());
        }
        state.parameters.pending = Some(Pending {
            request,
            candidate,
            rollback,
            committed: false,
        });
        Ok(json!({"status":"pending","confirmed":false}))
    }
    pub(crate) fn invalidate_control_program(&self) {
        let mut state = self.state.0.lock().expect("debug state poisoned");
        state.parameters.interrupt();
        state.transfers.interrupt();
        state.recordings.interrupt();
        state.signals.clear();
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
    pub(crate) fn commit_parameters(
        &self,
        task: &str,
        storage: &mut VariableStorage,
        metadata: &RuntimeMetadata,
        has_io_driver: bool,
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
        let validate = (|| -> Result<VariableStorage, String> {
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
            if has_io_driver && !request.physical_approved {
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
            if values(&request.binding, storage)? != request.expected_values {
                return Err("parameter values changed before task boundary".into());
            }
            validate_candidate(&request.binding, &pending.candidate)?;
            let mut staged = storage.clone();
            for parameter in &request.binding.parameters {
                let reference = resolve_path(&staged, &parameter.symbol)?.reference;
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
            Ok(staged)
        })();
        match validate {
            Ok(staged) => {
                *storage = staged;
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
    pub(crate) fn confirm_parameters(
        &self,
        task: &str,
        storage: &VariableStorage,
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
        let observed = values(&pending.request.binding, storage);
        if let Some(record) = state
            .parameters
            .records
            .get_mut(&pending.request.binding.loop_id)
        {
            if observed.as_ref() == Ok(&pending.candidate) {
                record.previous = if pending.rollback {
                    None
                } else {
                    Some(pending.request.expected_values)
                };
                record.active = pending.candidate;
                record.status = "applied".into();
                record.reason = None;
            } else {
                record.previous = None;
                record.status = "rejected".into();
                record.reason =
                    Some("controller changed parameter values before confirmation".into());
            }
        }
        state.snapshot = Some(crate::debug::DebugSnapshot {
            storage: storage.clone(),
            now,
        });
    }
}
