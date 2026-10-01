//! Guarded, task-bound controller transfer transactions.

use super::{parameters, DebugControl};
use crate::io::{IoBinding, IoTarget};
use crate::memory::{IoArea, VariableStorage};
use crate::runtime::RuntimeMetadata;
use crate::value::{Duration, Value};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// A transfer staged at the task boundary: the storage to commit, the values it applies, the values found before it (kept in
/// the transfer record) and the outputs held while it starts.
type StagedTransfer = (
    VariableStorage,
    BTreeMap<String, f64>,
    JsonValue,
    BTreeMap<String, f64>,
);
#[path = "transfer/tracking.rs"]
mod tracking;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NumberMember {
    symbol: String,
    iec_type: String,
    unit: String,
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
    mode: Mode,
    /// Further controllers' modes switched with `mode` at the same task boundary, for example a cascade's outer
    /// controller: Auto on commissioning, Manual on a Manual transfer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    additional_modes: Vec<Mode>,
    states: Vec<NumberMember>,
    guards: Vec<NumberMember>,
    output: NumberMember,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    additional_outputs: Vec<NumberMember>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tracking_outputs: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusRequest {
    binding: Binding,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelRequest {
    loop_id: String,
    transaction_id: String,
    expected_program_revision: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyRequest {
    transaction_id: String,
    binding: Binding,
    expected_program_revision: String,
    expected_binding_revision: String,
    expected_values: JsonValue,
    values: BTreeMap<String, f64>,
    manual: bool,
    maximum_output_change: f64,
    source: String,
    physical_approved: bool,
    tracking: Option<tracking::Tracking>,
}

#[derive(Clone, Debug)]
struct Record {
    binding_revision: String,
    program_revision: String,
    status: String,
    reason: Option<String>,
    transaction_id: Option<String>,
    transaction_program_revision: Option<String>,
    transaction_binding_revision: Option<String>,
    observed_values: Option<JsonValue>,
    confirmation: Option<String>,
    initial_values: Option<JsonValue>,
    applied_values: Option<BTreeMap<String, f64>>,
}

#[derive(Debug)]
struct Pending {
    request: ApplyRequest,
    committed: bool,
    post_st_verified: bool,
    initial_outputs: BTreeMap<String, f64>,
    /// Output members as they stood at the task boundary, restored if the first output is rejected.
    boundary_outputs: BTreeMap<String, f64>,
    applied_values: BTreeMap<String, f64>,
}

/// Runtime-owned transfer transaction state.  It deliberately never uses the
/// ordinary debugger write queues: a transfer is committed as one staged set.
#[derive(Debug, Default)]
pub(super) struct TransferRegistry {
    records: BTreeMap<String, Record>,
    pending: Option<Pending>,
    used_transactions: BTreeSet<(String, String)>,
    cancelled_before_dispatch: BTreeSet<(String, String)>,
    epoch_revision: Option<String>,
}

impl TransferRegistry {
    pub(super) fn interrupt(&mut self) {
        self.used_transactions.clear();
        self.cancelled_before_dispatch.clear();
        self.epoch_revision = None;
        let Some(pending) = self.pending.take() else {
            return;
        };
        if let Some(record) = self.records.get_mut(&pending.request.binding.loop_id) {
            record.status = "failed".into();
            record.reason = Some("runtime interrupted before transfer confirmation".into());
        }
    }

    pub(super) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
}

fn hash(value: impl std::fmt::Debug) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(format!("{value:?}").as_bytes())
    )
}

fn binding_revision(binding: &Binding) -> Result<String, String> {
    Ok(hash(
        serde_json::to_string(binding).map_err(|error| error.to_string())?,
    ))
}

fn root(symbol: &str) -> Result<&str, String> {
    let root = symbol.split('.').next().unwrap_or_default();
    if root.is_empty() || symbol.split('.').any(str::is_empty) {
        return Err("transfer symbol path is empty".into());
    }
    Ok(root)
}

fn declared_member(
    metadata: &RuntimeMetadata,
    symbol: &str,
) -> Result<crate::runtime::metadata::ControlBindingLeaf, String> {
    root(symbol)?;
    metadata
        .control_binding_leaf(symbol)
        .ok_or_else(|| "transfer member is not a declared scalar leaf".into())
}

fn check_owner(binding: &Binding, metadata: &RuntimeMetadata, symbol: &str) -> Result<(), String> {
    let root = root(symbol)?;
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
        if owners.is_empty() {
            Ok(())
        } else {
            Err("transfer program is task-owned".into())
        }
    } else if owners.len() == 1 && owners[0].name.eq_ignore_ascii_case(&binding.task_name) {
        Ok(())
    } else {
        Err("transfer members must have exactly the declared task owner".into())
    }
}

fn valid_number(member: &NumberMember) -> bool {
    matches!(member.iec_type.as_str(), "REAL" | "LREAL" | "BOOL")
        && (member.iec_type != "BOOL"
            || member.unit == "1" && member.minimum == 0.0 && member.maximum == 1.0)
        && !member.unit.trim().is_empty()
        && member.minimum.is_finite()
        && member.maximum.is_finite()
        && member.minimum <= member.maximum
}

fn check_binding(binding: &Binding, metadata: &RuntimeMetadata) -> Result<(), String> {
    if binding.loop_id.trim().is_empty()
        || binding.loop_id.chars().count() > 128
        || binding.states.is_empty()
        || binding.states.len() > 64
        || binding.guards.is_empty()
        || binding.guards.len() > 64
        || binding.additional_outputs.len() > 15
    {
        return Err("transfer needs 1 to 64 states/guards and at most 16 outputs".into());
    }
    for (output, feedback) in &binding.tracking_outputs {
        let command = std::iter::once(&binding.output)
            .chain(binding.additional_outputs.iter())
            .find(|member| &member.symbol == output)
            .ok_or("actuator tracking must name a declared output")?;
        let measured = binding
            .guards
            .iter()
            .find(|member| &member.symbol == feedback)
            .ok_or("actuator tracking feedback must be a declared guard")?;
        if command.unit != measured.unit {
            return Err("actuator tracking coordinates do not match".into());
        }
    }
    let mut symbols = HashSet::new();
    let program_root = root(&binding.mode.symbol)?;
    for member in binding
        .states
        .iter()
        .chain(binding.guards.iter())
        .chain(std::iter::once(&binding.output))
        .chain(binding.additional_outputs.iter())
    {
        if !valid_number(member) {
            return Err("invalid transfer member type, unit, or bounds".into());
        }
        if !symbols.insert(member.symbol.to_ascii_uppercase()) {
            return Err("duplicate transfer member".into());
        }
        if !root(&member.symbol)?.eq_ignore_ascii_case(program_root) {
            return Err("transfer members must belong to one compiled program".into());
        }
        declared_member(metadata, &member.symbol)?;
        check_owner(binding, metadata, &member.symbol)?;
    }
    if binding.additional_modes.len() > 8 {
        return Err("transfer switches at most 8 additional modes".into());
    }
    for mode in std::iter::once(&binding.mode).chain(binding.additional_modes.iter()) {
        if !symbols.insert(mode.symbol.to_ascii_uppercase()) {
            return Err("duplicate transfer mode member".into());
        }
        if !root(&mode.symbol)?.eq_ignore_ascii_case(program_root) {
            return Err("transfer members must belong to one compiled program".into());
        }
        let variable = declared_member(metadata, &mode.symbol)?;
        check_owner(binding, metadata, &mode.symbol)?;
        if variable.constant || variable.external || variable.in_out || variable.address.is_some() {
            return Err("mode must be mutable, nonexternal and unaddressed".into());
        }
    }
    let mut state_addresses = Vec::new();
    for member in &binding.states {
        let variable = declared_member(metadata, &member.symbol)?;
        if variable.constant
            || variable.external
            || variable.in_out
            || variable
                .address
                .as_ref()
                .is_some_and(|address| address.area != IoArea::Memory)
        {
            return Err("state must be mutable and unaddressed or uniquely memory-bound".into());
        }
        if let Some(address) = &variable.address {
            if state_addresses
                .iter()
                .any(|other| super::recording::overlap(other, address))
            {
                return Err("transfer states cannot overlap the same memory address".into());
            }
            state_addresses.push(address.clone());
        }
    }
    Ok(())
}

fn number(storage: &VariableStorage, member: &NumberMember) -> Result<f64, String> {
    let value = parameters::resolve_path(storage, &member.symbol)?.value;
    let value = match (member.iec_type.as_str(), value) {
        ("REAL", Value::Real(value)) => f64::from(value),
        ("LREAL", Value::LReal(value)) => value,
        ("BOOL", Value::Bool(value)) => f64::from(u8::from(value)),
        _ => return Err("transfer IEC type does not match runtime member".into()),
    };
    if !value.is_finite() {
        return Err("transfer member is nonfinite".into());
    }
    Ok(value)
}

fn observed(binding: &Binding, storage: &VariableStorage) -> Result<JsonValue, String> {
    let mut values = Map::new();
    for mode in std::iter::once(&binding.mode).chain(binding.additional_modes.iter()) {
        let Value::Bool(value) = parameters::resolve_path(storage, &mode.symbol)?.value else {
            return Err("transfer mode must be BOOL".into());
        };
        values.insert(mode.symbol.clone(), JsonValue::Bool(value));
    }
    for member in binding
        .states
        .iter()
        .chain(binding.guards.iter())
        .chain(std::iter::once(&binding.output))
        .chain(binding.additional_outputs.iter())
    {
        values.insert(member.symbol.clone(), json!(number(storage, member)?));
    }
    Ok(JsonValue::Object(values))
}

fn same_values(
    binding: &Binding,
    expected: &JsonValue,
    actual: &JsonValue,
    tracking: Option<&tracking::Tracking>,
) -> bool {
    let (Some(expected), Some(actual)) = (expected.as_object(), actual.as_object()) else {
        return false;
    };
    expected.len() == actual.len()
        && std::iter::once(&binding.mode)
            .chain(binding.additional_modes.iter())
            .all(|mode| {
                expected.get(&mode.symbol).and_then(JsonValue::as_bool)
                    == actual.get(&mode.symbol).and_then(JsonValue::as_bool)
            })
        && binding
            .states
            .iter()
            .chain(&binding.guards)
            .chain(std::iter::once(&binding.output))
            .chain(binding.additional_outputs.iter())
            .all(|member| {
                match (
                    expected.get(&member.symbol).and_then(JsonValue::as_f64),
                    actual.get(&member.symbol).and_then(JsonValue::as_f64),
                ) {
                    (Some(left), Some(right)) => {
                        left.is_finite()
                            && right.is_finite()
                            && if tracking.is_some_and(|policy| {
                                member.symbol == policy.process_variable
                                    || member.symbol == binding.output.symbol
                                    || binding
                                        .additional_outputs
                                        .iter()
                                        .any(|output| output.symbol == member.symbol)
                                    || tracking::accepts_live_measurement(policy, &member.symbol)
                                    || binding
                                        .tracking_outputs
                                        .values()
                                        .any(|symbol| symbol == &member.symbol)
                                    || tracking::accepts_live_state(policy, &member.symbol)
                            }) {
                                right >= member.minimum && right <= member.maximum
                            } else {
                                left == right
                            }
                    }
                    _ => false,
                }
            })
}

fn write_value(member: &NumberMember, value: f64) -> Result<Value, String> {
    if !value.is_finite() || value < member.minimum || value > member.maximum {
        return Err(format!("{} outside declared bounds", member.symbol));
    }
    match member.iec_type.as_str() {
        "BOOL" if value == 0.0 || value == 1.0 => Ok(Value::Bool(value == 1.0)),
        "LREAL" => Ok(Value::LReal(value)),
        "REAL" if (value as f32).is_finite() && f64::from(value as f32) == value => {
            Ok(Value::Real(value as f32))
        }
        "REAL" => {
            Err("REAL values must be explicitly rounded to their runtime representation".into())
        }
        _ => Err("invalid transfer IEC type".into()),
    }
}

fn check_io_target(
    symbol: &str,
    allowed_areas: &[IoArea],
    storage: &VariableStorage,
    io_bindings: &[IoBinding],
) -> Result<(), String> {
    let reference = parameters::resolve_path(storage, symbol)?.reference;
    let bindings: Vec<_> = io_bindings
        .iter()
        .filter(|binding| match &binding.target {
            IoTarget::Name(name) => name.eq_ignore_ascii_case(symbol),
            IoTarget::Reference(value) => value == &reference,
        })
        .collect();
    if bindings.len() > 1
        || bindings
            .iter()
            .any(|binding| !allowed_areas.contains(&binding.address.area))
    {
        return Err("transfer target has an ambiguous or disallowed I/O binding".into());
    }
    for binding in bindings {
        if binding.address.wildcard
            || binding.address.path.len() != 1
            || io_bindings.iter().any(|other| {
                !std::ptr::eq(other, binding)
                    && super::recording::overlap(&other.address, &binding.address)
            })
        {
            return Err("transfer target overlaps another I/O binding".into());
        }
    }
    Ok(())
}

impl DebugControl {
    pub(crate) fn transfer_status(
        &self,
        params: JsonValue,
        metadata: &RuntimeMetadata,
    ) -> Result<JsonValue, String> {
        let request: StatusRequest =
            serde_json::from_value(params).map_err(|error| error.to_string())?;
        check_binding(&request.binding, metadata)?;
        let binding_revision = binding_revision(&request.binding)?;
        let (lock, _) = &*self.state;
        let mut state = lock.lock().map_err(|_| "debug state unavailable")?;
        let program_revision = state.parameters.program_revision(metadata);
        state.transfers.epoch_revision = Some(program_revision.clone());
        if let Some(pending) = &state.transfers.pending {
            if pending.request.binding.loop_id == request.binding.loop_id
                && binding_revision != pending.request.expected_binding_revision
            {
                return Err(
                    "cannot replace a reviewed binding while its transfer is pending".into(),
                );
            }
        }
        let snapshot = state
            .snapshot
            .as_ref()
            .ok_or("runtime snapshot unavailable")?;
        let values = observed(&request.binding, &snapshot.storage)?;
        // The runtime clock of the reported scan lets a client observe durations in runtime time.
        let runtime_time_ns = snapshot.now.as_nanos();
        if state.transfers.records.len() >= 32
            && !state
                .transfers
                .records
                .contains_key(&request.binding.loop_id)
        {
            return Err("transfer loop-record capacity reached (32)".into());
        }
        let record = state
            .transfers
            .records
            .entry(request.binding.loop_id.clone())
            .or_insert_with(|| Record {
                binding_revision: binding_revision.clone(),
                program_revision: program_revision.clone(),
                status: "idle".into(),
                reason: None,
                transaction_id: None,
                transaction_program_revision: None,
                transaction_binding_revision: None,
                observed_values: None,
                confirmation: None,
                initial_values: None,
                applied_values: None,
            });
        if record.binding_revision != binding_revision
            || record.program_revision != program_revision
        {
            record.binding_revision = binding_revision.clone();
            record.program_revision = program_revision.clone();
        }
        let transaction = record.transaction_id.as_ref().map(|id| {
            json!({
                "id": id,
                "program_revision": record.transaction_program_revision.clone(),
                "binding_revision": record.transaction_binding_revision.clone(),
                "status": record.status.clone(),
                "reason": record.reason.clone(),
                "observed_values": record.observed_values.clone(),
                "confirmation": record.confirmation.clone(),
                "initial_values": record.initial_values.clone(),
                "applied_values": record.applied_values.clone(),
            })
        });
        Ok(
            json!({"program_revision":program_revision,"binding_revision":binding_revision,"values":values,"runtime_time_ns":runtime_time_ns,"status":record.status.clone(),"reason":record.reason.clone(),"transaction_id":record.transaction_id.clone(),"transaction":transaction,"supported_tracking":["proportional","parallel-pid-v1","declared-state-v1","declared-initialization-formulas"]}),
        )
    }

    pub(crate) fn transfer_apply(
        &self,
        params: JsonValue,
        metadata: &RuntimeMetadata,
    ) -> Result<JsonValue, String> {
        let request: ApplyRequest =
            serde_json::from_value(params).map_err(|error| error.to_string())?;
        if request.transaction_id.trim().is_empty()
            || request.transaction_id.chars().count() > 128
            || !matches!(request.source.as_str(), "simulation" | "physical")
        {
            return Err("invalid transfer transaction or source".into());
        }
        if request.source == "physical" && !request.physical_approved {
            return Err("physical review required".into());
        }
        if !request.maximum_output_change.is_finite() || request.maximum_output_change < 0.0 {
            return Err("invalid maximum output change".into());
        }
        check_binding(&request.binding, metadata)?;
        if let Some(policy) = &request.tracking {
            tracking::validate(&request.binding, policy, request.manual)?;
        }
        if request.values.len() != request.binding.states.len()
            || request.values.keys().any(|key| {
                !request
                    .binding
                    .states
                    .iter()
                    .any(|member| member.symbol == *key)
            })
        {
            return Err("exact transfer state set required".into());
        }
        for member in &request.binding.states {
            write_value(
                member,
                *request
                    .values
                    .get(&member.symbol)
                    .ok_or("exact transfer state set required")?,
            )?;
        }
        let binding_revision = binding_revision(&request.binding)?;
        let (lock, _) = &*self.state;
        let mut state = lock.lock().map_err(|_| "debug state unavailable")?;
        if state.transfers.has_pending() || state.parameters.has_pending() {
            return Err("a control transaction is pending".into());
        }
        if !state.forced_vars.is_empty() || !state.forced_io.is_empty() {
            return Err("release active forcing before transfer".into());
        }
        let program_revision = state.parameters.program_revision(metadata);
        state.transfers.epoch_revision = Some(program_revision.clone());
        let transaction_key = (program_revision.clone(), request.transaction_id.clone());
        if state.transfers.used_transactions.contains(&transaction_key) {
            return Err("transfer transaction identity has already been used".into());
        }
        if state.transfers.used_transactions.len() >= 1024 {
            return Err(
                "transfer replay-identity capacity reached (1024); start a new runtime epoch"
                    .into(),
            );
        }
        let snapshot = state
            .snapshot
            .as_ref()
            .ok_or("runtime snapshot unavailable")?;
        let values = observed(&request.binding, &snapshot.storage)?;
        let initial_outputs = std::iter::once(&request.binding.output)
            .chain(request.binding.additional_outputs.iter())
            .map(|output| Ok((output.symbol.clone(), number(&snapshot.storage, output)?)))
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        if request.expected_program_revision != program_revision
            || request.expected_binding_revision != binding_revision
            || !same_values(
                &request.binding,
                &request.expected_values,
                &values,
                request.tracking.as_ref(),
            )
        {
            return Err("stale program, binding, or expected values".into());
        }
        let record = state
            .transfers
            .records
            .get_mut(&request.binding.loop_id)
            .ok_or("read transfer status before apply")?;
        if record.binding_revision != binding_revision
            || record.program_revision != program_revision
        {
            return Err("binding changed since review".into());
        }
        if record.transaction_id.as_deref() == Some(&request.transaction_id)
            && matches!(
                record.status.as_str(),
                "applied" | "failed" | "rejected" | "cancelled"
            )
        {
            return Err("completed transfer transaction cannot be retried".into());
        }
        record.status = "pending".into();
        record.reason = None;
        record.transaction_id = Some(request.transaction_id.clone());
        record.transaction_program_revision = Some(program_revision);
        record.transaction_binding_revision = Some(binding_revision);
        record.observed_values = None;
        record.confirmation = None;
        record.initial_values = None;
        record.applied_values = None;
        state.transfers.used_transactions.insert(transaction_key);
        let applied_values = request.values.clone();
        state.transfers.pending = Some(Pending {
            request,
            committed: false,
            post_st_verified: false,
            initial_outputs,
            boundary_outputs: BTreeMap::new(),
            applied_values,
        });
        Ok(json!({"status":"pending","confirmed":false}))
    }

    pub(crate) fn has_transfer_transaction(&self) -> bool {
        self.state
            .0
            .lock()
            .expect("debug state poisoned")
            .transfers
            .has_pending()
    }

    pub(crate) fn transfer_cancel(&self, params: JsonValue) -> Result<JsonValue, String> {
        let request: CancelRequest =
            serde_json::from_value(params).map_err(|error| error.to_string())?;
        let mut state = self.state.0.lock().map_err(|_| "debug state unavailable")?;
        if request.transaction_id.is_empty()
            || request.transaction_id.len() > 128
            || state.transfers.epoch_revision.as_ref() != Some(&request.expected_program_revision)
        {
            return Err("read the current transfer epoch before cancelling its identity".into());
        }
        let record = state
            .transfers
            .records
            .get(&request.loop_id)
            .ok_or("transfer unavailable")?;
        let binding_revision = record.binding_revision.clone();
        let key = (
            request.expected_program_revision.clone(),
            request.transaction_id.clone(),
        );
        if state.transfers.cancelled_before_dispatch.contains(&key)
            || !state.transfers.used_transactions.contains(&key)
        {
            if !state.transfers.used_transactions.contains(&key)
                && state.transfers.used_transactions.len() >= 1024
            {
                return Err(
                    "transfer identity capacity reached; cancellation could not be registered"
                        .into(),
                );
            }
            // Close the persist-before-send race: a delayed Apply with this ID
            // must be rejected even if cancellation arrived before Apply.
            state.transfers.used_transactions.insert(key.clone());
            state.transfers.cancelled_before_dispatch.insert(key);
            return Ok(
                json!({"status":"cancelled","confirmed":true,"before_dispatch":true,
                "transaction_id":request.transaction_id,"program_revision":request.expected_program_revision,
                "binding_revision":binding_revision}),
            );
        }
        let record = state
            .transfers
            .records
            .get(&request.loop_id)
            .ok_or("transfer unavailable")?;
        if record.transaction_id.as_ref() != Some(&request.transaction_id)
            || record.transaction_program_revision.as_ref()
                != Some(&request.expected_program_revision)
        {
            return Err("transfer cancellation identity does not match".into());
        }
        if let Some(pending) = state.transfers.pending.as_ref().filter(|pending| {
            pending.request.binding.loop_id == request.loop_id
                && pending.request.transaction_id == request.transaction_id
        }) {
            if pending.committed {
                return Ok(
                    json!({"status":"pending","confirmed":false,"reason":"transfer already committed; await its actual outcome"}),
                );
            }
            state.transfers.pending = None;
            let record = state
                .transfers
                .records
                .get_mut(&request.loop_id)
                .ok_or("transfer unavailable")?;
            record.status = "cancelled".into();
            record.reason = Some("cancelled before task-boundary commit".into());
        }
        let record = state
            .transfers
            .records
            .get(&request.loop_id)
            .ok_or("transfer unavailable")?;
        Ok(
            json!({"status":record.status,"confirmed":record.status != "pending","reason":record.reason}),
        )
    }

    pub(crate) fn commit_transfer(
        &self,
        task: &str,
        storage: &mut VariableStorage,
        metadata: &RuntimeMetadata,
        has_io_driver: bool,
        io_bindings: &[IoBinding],
    ) {
        let mut state = self.state.0.lock().expect("debug state poisoned");
        let Some(mut pending) = state.transfers.pending.take() else {
            return;
        };
        if !pending.request.binding.task_name.eq_ignore_ascii_case(task) {
            state.transfers.pending = Some(pending);
            return;
        }
        let request = &pending.request;
        let result = (|| -> Result<StagedTransfer, String> {
            if request.expected_program_revision != state.parameters.program_revision(metadata) {
                return Err("program changed before task boundary".into());
            }
            let record = state
                .transfers
                .records
                .get(&request.binding.loop_id)
                .ok_or("transfer binding unavailable")?;
            if record.binding_revision != request.expected_binding_revision
                || record.program_revision != request.expected_program_revision
            {
                return Err("transfer binding changed before task boundary".into());
            }
            check_binding(&request.binding, metadata)?;
            if has_io_driver && !request.physical_approved {
                return Err("attached I/O requires explicit physical review".into());
            }
            if !state.forced_vars.is_empty() || !state.forced_io.is_empty() {
                return Err("forcing changed before task boundary".into());
            }
            let mut current = observed(&request.binding, storage)?;
            if !same_values(
                &request.binding,
                &request.expected_values,
                &current,
                request.tracking.as_ref(),
            ) {
                return Err("transfer values changed before task boundary".into());
            }
            for member in &request.binding.states {
                check_io_target(&member.symbol, &[IoArea::Memory], storage, io_bindings)?;
            }
            for mode in std::iter::once(&request.binding.mode)
                .chain(request.binding.additional_modes.iter())
            {
                check_io_target(&mode.symbol, &[], storage, io_bindings)?;
            }
            let mut held_outputs = BTreeMap::new();
            for output in std::iter::once(&request.binding.output)
                .chain(request.binding.additional_outputs.iter())
            {
                check_io_target(
                    &output.symbol,
                    &[IoArea::Output, IoArea::Memory],
                    storage,
                    io_bindings,
                )?;
                let measured = request.binding.tracking_outputs.get(&output.symbol);
                let held = if let Some(feedback) = measured {
                    let member = request
                        .binding
                        .guards
                        .iter()
                        .find(|member| &member.symbol == feedback)
                        .ok_or("applied feedback is missing")?;
                    tracking::fresh_applied_feedback(member, storage, io_bindings)?
                } else if request.tracking.is_some() {
                    tracking::held_output(
                        output,
                        storage,
                        io_bindings,
                        state.latest_io_snapshot.as_ref(),
                    )?
                } else {
                    number(storage, output)?
                };
                if measured.is_none() {
                    current[&output.symbol] = json!(held);
                }
                held_outputs.insert(output.symbol.clone(), held);
            }
            let applied = if let Some(policy) = &request.tracking {
                tracking::calculate(
                    &request.binding,
                    policy,
                    &request.values,
                    storage,
                    &held_outputs,
                    request.manual,
                )?
            } else {
                request.values.clone()
            };
            let mut staged = storage.clone();
            for mode in std::iter::once(&request.binding.mode)
                .chain(request.binding.additional_modes.iter())
            {
                let mode_reference = parameters::resolve_path(&staged, &mode.symbol)?.reference;
                if !staged.write_by_ref(
                    mode_reference,
                    Value::Bool(if request.manual {
                        mode.manual_value
                    } else {
                        !mode.manual_value
                    }),
                ) {
                    return Err("transfer mode is not writable".into());
                }
            }
            for member in &request.binding.states {
                let reference = parameters::resolve_path(&staged, &member.symbol)?.reference;
                if !staged.write_by_ref(reference, write_value(member, applied[&member.symbol])?) {
                    return Err(format!("transfer state {} is not writable", member.symbol));
                }
            }
            Ok((staged, applied, current, held_outputs))
        })();
        match result {
            Ok((staged, applied, initial, held_outputs)) => {
                let boundary_outputs = std::iter::once(&request.binding.output)
                    .chain(request.binding.additional_outputs.iter())
                    .filter_map(|member| {
                        number(storage, member)
                            .ok()
                            .map(|value| (member.symbol.clone(), value))
                    })
                    .collect();
                *storage = staged;
                if let Some(record) = state.transfers.records.get_mut(&request.binding.loop_id) {
                    record.initial_values = Some(initial);
                    record.applied_values = Some(applied.clone());
                }
                pending.applied_values = applied;
                pending.initial_outputs = held_outputs;
                pending.boundary_outputs = boundary_outputs;
                pending.committed = true;
                state.transfers.pending = Some(pending);
            }
            Err(reason) => {
                if let Some(record) = state.transfers.records.get_mut(&request.binding.loop_id) {
                    record.status = "rejected".into();
                    record.reason = Some(reason);
                }
            }
        }
    }

    pub(crate) fn confirm_transfer_task(&self, task: &str, storage: &mut VariableStorage) {
        let mut state = self.state.0.lock().expect("debug state poisoned");
        let Some(mut pending) = state.transfers.pending.take() else {
            return;
        };
        if !pending.committed || !pending.request.binding.task_name.eq_ignore_ascii_case(task) {
            state.transfers.pending = Some(pending);
            return;
        }
        let request = &pending.request;
        let verification = (|| -> Result<JsonValue, String> {
            for mode in std::iter::once(&request.binding.mode)
                .chain(request.binding.additional_modes.iter())
            {
                let expected_mode = if request.manual {
                    mode.manual_value
                } else {
                    !mode.manual_value
                };
                if parameters::resolve_path(storage, &mode.symbol)?.value
                    != Value::Bool(expected_mode)
                {
                    return Err("controller changed transfer mode before confirmation".into());
                }
            }
            for member in &request.binding.states {
                let actual = number(storage, member)?;
                if request
                    .tracking
                    .as_ref()
                    .is_some_and(|policy| tracking::bounded_post_scan_state(policy, &member.symbol))
                {
                    if actual < member.minimum || actual > member.maximum {
                        return Err(format!(
                            "controller state {} exceeded its declared post-scan bounds",
                            member.symbol
                        ));
                    }
                    continue;
                }
                let expected = if let Some(policy) = request.tracking.as_ref() {
                    tracking::expected_state_after_scan(
                        &request.binding,
                        policy,
                        &pending.applied_values,
                        storage,
                        member,
                        request.manual,
                    )?
                } else {
                    pending.applied_values[&member.symbol]
                };
                if actual != expected {
                    return Err(format!(
                        "controller changed transfer state {} before confirmation (expected {expected}, observed {actual})",
                        member.symbol
                    ));
                }
            }
            let span = |member: &NumberMember| {
                let basis = request
                    .binding
                    .tracking_outputs
                    .get(&member.symbol)
                    .and_then(|symbol| {
                        request
                            .binding
                            .guards
                            .iter()
                            .find(|guard| &guard.symbol == symbol)
                    })
                    .unwrap_or(member);
                basis.maximum - basis.minimum
            };
            let primary_span = span(&request.binding.output);
            for member in std::iter::once(&request.binding.output)
                .chain(request.binding.additional_outputs.iter())
            {
                let output = number(storage, member)?;
                let tolerance = if member.symbol == request.binding.output.symbol {
                    request.maximum_output_change
                } else if primary_span > 0.0 {
                    request.maximum_output_change * span(member) / primary_span
                } else {
                    0.0
                };
                let initial = pending
                    .initial_outputs
                    .get(&member.symbol)
                    .ok_or("initial actuator output is missing")?;
                if !tolerance.is_finite()
                    || output < member.minimum
                    || output > member.maximum
                    || (output - initial).abs() > tolerance
                {
                    return Err(format!(
                        "first controller output {} violates reviewed transfer tolerance",
                        member.symbol
                    ));
                }
            }
            observed(&request.binding, storage)
        })();
        match verification {
            Ok(observed_values) => {
                pending.post_st_verified = true;
                if let Some(record) = state.transfers.records.get_mut(&request.binding.loop_id) {
                    record.observed_values = Some(observed_values);
                    record.confirmation = Some("post-st".into());
                }
                state.transfers.pending = Some(pending);
            }
            Err(reason) => {
                // The rejected output is never published: before the output commit, the output members return to
                // their task-boundary values. The failed transfer is not a runtime fault, so the separately guarded
                // Manual recovery can run on the next cycle.
                for member in std::iter::once(&request.binding.output)
                    .chain(request.binding.additional_outputs.iter())
                {
                    let Some(&value) = pending.boundary_outputs.get(&member.symbol) else {
                        continue;
                    };
                    let value = match member.iec_type.as_str() {
                        "REAL" => Value::Real(value as f32),
                        _ => Value::LReal(value),
                    };
                    if let Ok(resolved) = parameters::resolve_path(storage, &member.symbol) {
                        storage.write_by_ref(resolved.reference, value);
                    }
                }
                if let Some(record) = state.transfers.records.get_mut(&request.binding.loop_id) {
                    record.status = "failed".into();
                    record.reason = Some(reason);
                }
            }
        }
    }

    pub(crate) fn finish_transfer_cycle(&self, storage: &VariableStorage, now: Duration) {
        let mut state = self.state.0.lock().expect("debug state poisoned");
        let Some(pending) = state.transfers.pending.take() else {
            return;
        };
        if !pending.post_st_verified {
            state.transfers.pending = Some(pending);
            return;
        }
        if let Some(record) = state
            .transfers
            .records
            .get_mut(&pending.request.binding.loop_id)
        {
            record.status = "applied".into();
            record.reason = None;
            record.confirmation = Some("output-image".into());
        }
        state.snapshot = Some(crate::debug::DebugSnapshot {
            storage: storage.clone(),
            now,
        });
    }

    pub(crate) fn fail_transfer(&self, reason: &str) {
        let mut state = self.state.0.lock().expect("debug state poisoned");
        let Some(pending) = state.transfers.pending.take() else {
            return;
        };
        if let Some(record) = state
            .transfers
            .records
            .get_mut(&pending.request.binding.loop_id)
        {
            record.status = "failed".into();
            record.reason = Some(reason.into());
        }
    }
}
