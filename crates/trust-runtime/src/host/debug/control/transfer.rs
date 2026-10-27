//! Guarded, task-bound controller transfer transactions.

use super::program_facts::{self, BindingLeaf, ProgramTask};
use super::{parameters, DebugControl};
use crate::io::IoAddress;
use crate::memory::IoArea;
use crate::runtime::{Located, ProgramRead, ProgramWrite};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::memory::Place;

/// A transfer's planned writes (destination, value before, value to write, refusal text),
/// applied values, initial values, held outputs and task-boundary output values.
type StagedTransfer = (
    Vec<(Located, Value, Value, String)>,
    BTreeMap<String, f64>,
    JsonValue,
    BTreeMap<String, f64>,
    BTreeMap<String, f64>,
);
#[path = "transfer/restore.rs"]
mod restore;
#[path = "transfer/review.rs"]
mod review;
#[path = "transfer/tracking.rs"]
mod tracking;

/// Longest identifier a transfer accepts, in characters (E6.2, E6.5).
const MAX_IDENTIFIER_CHARS: usize = 128;
/// Most states, and most guards, that one transfer names (E6.2).
const MAX_MEMBERS: usize = 64;
/// Most additional outputs that one transfer names (E6.2).
const MAX_ADDITIONAL_OUTPUTS: usize = 15;
/// Loops that one session reviews at once (the loop-record capacity).
const MAX_LOOP_RECORDS: usize = 32;
/// Transaction identities that one session epoch holds (E6.5 step 13).
const MAX_TRANSACTION_IDENTITIES: usize = 1024;

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
    /// The factor of the error at entry in the first output's allowance, `|Ki|·Δt` (E6.9): the
    /// error the controller enters Auto with is known only in the transfer's own scan.
    #[serde(default)]
    entry_error_gain: Option<f64>,
    /// The factor of the measurement's change in the first output's allowance, `|Kp|` (E6.9): an
    /// entry that carries the controller's states responds to the measurement's change since the
    /// scan before the transfer, which is known only in the transfer's own scan.
    #[serde(default)]
    entry_measurement_gain: Option<f64>,
    source: String,
    physical_approved: bool,
    tracking: Option<tracking::Tracking>,
}

/// The transaction record of one loop. A status read never writes it: only apply, commit, confirmation, cancel and
/// a fault do.
#[derive(Clone, Debug, Default)]
struct Record {
    status: String,
    reason: Option<String>,
    transaction_id: Option<String>,
    transaction_program_revision: Option<String>,
    transaction_binding_revision: Option<String>,
    observed_values: Option<JsonValue>,
    confirmation: Option<String>,
    initial_values: Option<JsonValue>,
    applied_values: Option<BTreeMap<String, f64>>,
    /// Whether a rejected transfer was reverted before the output commit.
    restored: bool,
}

/// The revisions that the last status read of a loop reviewed (E6.10, E6.5 step 16).
#[derive(Clone, Debug)]
struct Review {
    binding_revision: String,
    program_revision: String,
}

#[derive(Debug)]
struct Pending {
    request: ApplyRequest,
    committed: bool,
    post_st_verified: bool,
    initial_outputs: BTreeMap<String, f64>,
    /// Output members as they stood at the task boundary, restored if the first output is rejected.
    boundary_outputs: BTreeMap<String, f64>,
    /// The place and the pre-commit value of every member the commit wrote: restored if the transfer is rejected.
    restore_points: Vec<(Located, Value)>,
    applied_values: BTreeMap<String, f64>,
    /// The tracking's process variable as the scan before the transfer left it, read with the
    /// apply, when the apply has an `entry_measurement_gain`.
    measurement_before: Option<f64>,
    /// `entry_error_gain` times the error the controller entered Auto with, plus
    /// `entry_measurement_gain` times the measurement's change since `measurement_before`, both taken
    /// at the commit: the first output may move this much more than `maximum_output_change` (E6.9).
    entry_allowance: f64,
}

/// Runtime-owned transfer transaction state.  It deliberately never uses the
/// ordinary debugger write queues: a transfer is committed as one staged set.
#[derive(Debug, Default)]
pub(super) struct TransferRegistry {
    records: BTreeMap<String, Record>,
    reviews: BTreeMap<String, Review>,
    pending: Option<Pending>,
    used_transactions: BTreeSet<(String, String)>,
    cancelled_before_dispatch: BTreeSet<(String, String)>,
    epoch_revision: Option<String>,
}

impl TransferRegistry {
    pub(super) fn interrupt(&mut self) {
        self.used_transactions.clear();
        self.cancelled_before_dispatch.clear();
        self.reviews.clear();
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

    /// The identifier of the pending transfer, when one is pending (A6.6).
    pub(super) fn pending_id(&self) -> Option<&str> {
        self.pending
            .as_ref()
            .map(|pending| pending.request.transaction_id.as_str())
    }
}

/// Refuses a request whose source label contradicts the runtime's own target fact (A2.4). The label is never trusted.
pub(super) fn check_target_source(source: &str, physical_target: bool) -> Result<(), String> {
    let (expected, target) = if physical_target {
        ("physical", "physical")
    } else {
        ("simulation", "simulated")
    };
    if source == expected {
        Ok(())
    } else {
        Err(format!(
            "The request says {source}, but the runtime's target is {target}."
        ))
    }
}

/// A live write on a physical target needs the engineer's approval (A2.5, E10.2).
pub(super) fn check_physical_review(
    physical_target: bool,
    physical_approved: bool,
) -> Result<(), String> {
    if physical_target && !physical_approved {
        Err("physical review required".into())
    } else {
        Ok(())
    }
}

fn hash(value: impl std::fmt::Debug) -> String {
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(format!("{value:?}").as_bytes()))
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

fn declared_member(image: &Image<'_>, symbol: &str) -> Result<BindingLeaf, String> {
    root(symbol)?;
    program_facts::binding_leaf(image, symbol)
        .ok_or_else(|| "transfer member is not a declared scalar leaf".into())
}

fn check_owner(binding: &Binding, tasks: &[ProgramTask], symbol: &str) -> Result<(), String> {
    let root = root(symbol)?;
    let owners: Vec<_> = tasks
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

/// Checks a binding against the program of `image` (E6.2): its members are declared scalar leaves
/// of one program instance that its declared task runs, its modes are mutable and unaddressed, and
/// its states are mutable and unaddressed or bound to distinct marker addresses.
fn check_binding(binding: &Binding, image: &Image<'_>) -> Result<(), String> {
    if binding.loop_id.trim().is_empty() || binding.loop_id.chars().count() > MAX_IDENTIFIER_CHARS {
        return Err(format!(
            "transfer loop_id must be 1 to {MAX_IDENTIFIER_CHARS} characters"
        ));
    }
    if binding.states.is_empty() || binding.states.len() > MAX_MEMBERS {
        return Err(format!(
            "transfer has {} states; it needs 1 to {MAX_MEMBERS}",
            binding.states.len()
        ));
    }
    if binding.guards.is_empty() || binding.guards.len() > MAX_MEMBERS {
        return Err(format!(
            "transfer has {} guards; it needs 1 to {MAX_MEMBERS}",
            binding.guards.len()
        ));
    }
    if binding.additional_outputs.len() > MAX_ADDITIONAL_OUTPUTS {
        return Err(format!(
            "transfer has {} additional outputs; it allows at most {MAX_ADDITIONAL_OUTPUTS}",
            binding.additional_outputs.len()
        ));
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
    let tasks = program_facts::program_tasks(image);
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
        declared_member(image, &member.symbol)?;
        check_owner(binding, &tasks, &member.symbol)?;
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
        let variable = declared_member(image, &mode.symbol)?;
        check_owner(binding, &tasks, &mode.symbol)?;
        if variable.constant || variable.in_out || variable.direct {
            return Err("mode must be mutable, nonexternal and unaddressed".into());
        }
    }
    let mut state_addresses = Vec::new();
    for member in &binding.states {
        let variable = declared_member(image, &member.symbol)?;
        if variable.constant
            || variable.in_out
            || (variable.direct
                && variable
                    .address
                    .as_ref()
                    .is_none_or(|address| address.area != IoArea::Memory))
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

/// A member's runtime value as a finite number of its declared IEC type.
fn number(program: &dyn ProgramRead, member: &NumberMember) -> Result<f64, String> {
    let value = parameters::resolve_path(program, &member.symbol)?.value;
    let value = match (member.iec_type.as_str(), value) {
        ("REAL", Value::Real(value)) => f64::from(value),
        ("LREAL", Value::LReal(value)) => value,
        ("BOOL", Value::Bool(value)) => f64::from(u8::from(value)),
        _ => {
            return Err(format!(
                "{} is not of IEC type {}",
                member.symbol, member.iec_type
            ))
        }
    };
    if !value.is_finite() {
        return Err(format!(
            "{} has bad quality: not a finite number",
            member.symbol
        ));
    }
    Ok(value)
}

fn observed(binding: &Binding, program: &dyn ProgramRead) -> Result<JsonValue, String> {
    let mut values = Map::new();
    for mode in std::iter::once(&binding.mode).chain(binding.additional_modes.iter()) {
        let Value::Bool(value) = parameters::resolve_path(program, &mode.symbol)?.value else {
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
        values.insert(member.symbol.clone(), json!(number(program, member)?));
    }
    Ok(JsonValue::Object(values))
}

/// Whether the law tracks a member's value, so that the member is checked by its band rather than exactly (A6.4).
fn banded(binding: &Binding, symbol: &str, tracking: Option<&tracking::Tracking>) -> bool {
    tracking.is_some_and(|policy| {
        policy.process_variable == symbol
            || binding.output.symbol == symbol
            || binding
                .additional_outputs
                .iter()
                .any(|output| output.symbol == symbol)
            || tracking::accepts_live_measurement(policy, symbol)
            || binding
                .tracking_outputs
                .values()
                .any(|feedback| feedback.as_str() == symbol)
            || tracking::accepts_live_state(policy, symbol)
            // A state the program sets, such as a manual output that follows the output in Auto (specification 35, §17.1).
            || tracking::bounded_post_scan_state(policy, symbol)
    })
}

fn shown<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "missing".to_owned(), |value| value.to_string())
}

fn stale_value(
    symbol: &str,
    expected: impl std::fmt::Display,
    observed: impl std::fmt::Display,
) -> String {
    format!(
        "stale program, binding, or expected values: {symbol} expected {expected}, observed {observed}"
    )
}

/// Checks the values that a status read reviewed against the values now (A6.4). Modes and written states are exact. A
/// measurement or output that the law tracks must be a finite, good value inside its declared range, and it is never
/// compared exactly.
fn same_values(
    binding: &Binding,
    expected: &JsonValue,
    actual: &JsonValue,
    tracking: Option<&tracking::Tracking>,
) -> Result<(), String> {
    let (Some(expected), Some(actual)) = (expected.as_object(), actual.as_object()) else {
        return Err(
            "stale program, binding, or expected values: the values are not an object".into(),
        );
    };
    if expected.len() != actual.len() {
        return Err("stale program, binding, or expected values: the member sets differ".into());
    }
    for mode in std::iter::once(&binding.mode).chain(binding.additional_modes.iter()) {
        let left = expected.get(&mode.symbol).and_then(JsonValue::as_bool);
        let right = actual.get(&mode.symbol).and_then(JsonValue::as_bool);
        if left != right {
            return Err(stale_value(&mode.symbol, shown(left), shown(right)));
        }
    }
    for member in binding
        .states
        .iter()
        .chain(&binding.guards)
        .chain(std::iter::once(&binding.output))
        .chain(binding.additional_outputs.iter())
    {
        let left = expected.get(&member.symbol).and_then(JsonValue::as_f64);
        let right = actual.get(&member.symbol).and_then(JsonValue::as_f64);
        let (Some(left), Some(right)) = (left, right) else {
            return Err(format!(
                "{} has bad quality: its value is missing or not a number",
                member.symbol
            ));
        };
        if !right.is_finite() {
            return Err(format!(
                "{} has bad quality: not a finite number",
                member.symbol
            ));
        }
        if banded(binding, &member.symbol, tracking) {
            if right < member.minimum || right > member.maximum {
                return Err(format!(
                    "{} = {right} is outside its declared range [{}, {}]",
                    member.symbol, member.minimum, member.maximum
                ));
            }
        } else if left != right {
            return Err(stale_value(&member.symbol, left, right));
        }
    }
    Ok(())
}

fn write_value(member: &NumberMember, value: f64) -> Result<Value, String> {
    if !value.is_finite() || value < member.minimum || value > member.maximum {
        return Err(format!(
            "{} outside declared bounds: {value} is not within [{}, {}]",
            member.symbol, member.minimum, member.maximum
        ));
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

/// The state values of an apply: exactly the declared states, each within its bounds and of its type.
fn check_value_set(request: &ApplyRequest) -> Result<(), String> {
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
        let value = *request
            .values
            .get(&member.symbol)
            .ok_or("exact transfer state set required")?;
        write_value(member, value)?;
    }
    Ok(())
}

/// The revisions of an apply must be the current ones (E6.5 step 15).
fn check_revisions(
    request: &ApplyRequest,
    program_revision: &str,
    binding_revision: &str,
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
    Ok(())
}

/// The process-image addresses bound at `at`, among the program's bindings `bound`, by their
/// positions in `bound`.
fn bindings_at(bound: &[(Place, IoAddress)], at: &Located) -> Vec<usize> {
    bound
        .iter()
        .enumerate()
        .filter(|(_, (place, _))| *place == at.place)
        .map(|(index, _)| index)
        .collect()
}

/// Checks the process-image binding of the member `symbol`, `bound` being the program's bindings
/// (E6.6): at most one, in one of `allowed_areas`, explicit and overlapping no other binding.
fn check_io_target(
    symbol: &str,
    allowed_areas: &[IoArea],
    program: &dyn ProgramRead,
    bound: &[(Place, IoAddress)],
) -> Result<(), String> {
    let at = parameters::resolve_path(program, symbol)?.at;
    let bindings = bindings_at(bound, &at);
    if bindings.len() > 1
        || bindings
            .iter()
            .any(|&index| !allowed_areas.contains(&bound[index].1.area))
    {
        return Err("transfer target has an ambiguous or disallowed I/O binding".into());
    }
    for index in bindings {
        let address = &bound[index].1;
        if address.wildcard
            || address.path.len() != 1
            || bound.iter().enumerate().any(|(other, (_, other_address))| {
                other != index && super::recording::overlap(other_address, address)
            })
        {
            return Err("transfer target overlaps another I/O binding".into());
        }
    }
    Ok(())
}

impl DebugControl {
    /// Reads the transfer of a loop from the program as the last scan left it. It changes only the
    /// review marker of the loop (E6.3, E6.10): the transaction record is only read.
    pub(crate) fn transfer_status(&self, params: JsonValue) -> Result<JsonValue, String> {
        let request: StatusRequest =
            serde_json::from_value(params).map_err(|error| error.to_string())?;
        // The record and the values are read under one lock (E6.3): the scan publishes the program
        // before it marks a transfer applied ([`Self::finish_transfer_cycle`]), so an outcome read
        // here comes with the values of the scan that decided it, or of a later one.
        let mut state = self.lock();
        let program = self
            .published_program()
            .ok_or("runtime snapshot unavailable")?;
        let image = program.program_image();
        check_binding(&request.binding, &image)?;
        let binding_revision = binding_revision(&request.binding)?;
        let program_revision = state.parameters.program_revision(&image);
        if let Some(pending) = &state.transfers.pending
            && pending.request.binding.loop_id == request.binding.loop_id
            && binding_revision != pending.request.expected_binding_revision
        {
            return Err("cannot replace a reviewed binding while its transfer is pending".into());
        }
        let values = observed(&request.binding, &*program)?;
        // The runtime clock of the reported scan lets a client observe durations in runtime time.
        let runtime_time_ns = program.now().as_nanos();
        state.transfers.review(
            &request.binding.loop_id,
            &binding_revision,
            &program_revision,
        )?;
        let outcome = state
            .transfers
            .outcome(&request.binding.loop_id, &binding_revision);
        Ok(
            json!({"program_revision":program_revision,"binding_revision":binding_revision,"values":values,"runtime_time_ns":runtime_time_ns,"status":outcome.status,"reason":outcome.reason,"transaction_id":outcome.transaction_id,"transaction":outcome.transaction,"supported_tracking":["proportional","parallel-pid-v1","declared-state-v1","declared-initialization-formulas"]}),
        )
    }

    /// Accepts a transfer for the next task boundary of its owning task, against the program as
    /// the last scan left it. `physical_target` is the runtime's own target fact (A2.4): the
    /// request's source label must agree with it, and a physical target needs approval.
    pub(crate) fn transfer_apply(
        &self,
        params: JsonValue,
        physical_target: bool,
    ) -> Result<JsonValue, String> {
        let request: ApplyRequest =
            serde_json::from_value(params).map_err(|error| error.to_string())?;
        if request.transaction_id.trim().is_empty()
            || request.transaction_id.chars().count() > MAX_IDENTIFIER_CHARS
            || !matches!(request.source.as_str(), "simulation" | "physical")
        {
            return Err("invalid transfer transaction or source".into());
        }
        check_target_source(&request.source, physical_target)?;
        check_physical_review(physical_target, request.physical_approved)?;
        if !request.maximum_output_change.is_finite() || request.maximum_output_change < 0.0 {
            return Err("invalid maximum output change".into());
        }
        if request.entry_error_gain.is_some_and(|gain| {
            !gain.is_finite() || gain < 0.0 || request.manual || request.tracking.is_none()
        }) {
            return Err("invalid entry error gain".into());
        }
        if request.entry_measurement_gain.is_some_and(|gain| {
            !gain.is_finite() || gain < 0.0 || request.manual || request.tracking.is_none()
        }) {
            return Err("invalid entry measurement gain".into());
        }
        let program = self
            .published_program()
            .ok_or("runtime snapshot unavailable")?;
        let image = program.program_image();
        check_binding(&request.binding, &image)?;
        if let Some(policy) = &request.tracking {
            tracking::validate(&request.binding, policy, request.manual)?;
        }
        check_value_set(&request)?;
        let binding_revision = binding_revision(&request.binding)?;
        let mut state = self.lock();
        if let Some(id) = state.transfers.pending_id() {
            return Err(format!("a controller transfer {id} is pending"));
        }
        if let Some(pending) = state.parameters.pending_label() {
            return Err(format!("{pending} is pending"));
        }
        if !state.live_values.forced_vars.is_empty() || !state.live_values.forced_io.is_empty() {
            return Err("release active forcing before transfer".into());
        }
        let program_revision = state.parameters.program_revision(&image);
        state.transfers.epoch_revision = Some(program_revision.clone());
        let transaction_key = (program_revision.clone(), request.transaction_id.clone());
        if state.transfers.used_transactions.contains(&transaction_key) {
            return Err("transfer transaction identity has already been used".into());
        }
        if state.transfers.used_transactions.len() >= MAX_TRANSACTION_IDENTITIES {
            return Err(format!(
                "transfer replay-identity capacity reached ({MAX_TRANSACTION_IDENTITIES}); start a new runtime epoch"
            ));
        }
        if state.parameters.snapshot_is_stale() {
            return Err(parameters::STALE_SNAPSHOT.into());
        }
        let values = observed(&request.binding, &*program)?;
        // The measurement of the scan before the transfer, from which the entry's first execution
        // responds to its change (E6.9).
        let measurement_before = match (&request.tracking, request.entry_measurement_gain) {
            (Some(policy), Some(_)) => Some(
                values[policy.process_variable.as_str()]
                    .as_f64()
                    .ok_or("tracking guard unavailable")?,
            ),
            _ => None,
        };
        let initial_outputs = std::iter::once(&request.binding.output)
            .chain(request.binding.additional_outputs.iter())
            .map(|output| Ok((output.symbol.clone(), number(&*program, output)?)))
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        check_revisions(&request, &program_revision, &binding_revision)?;
        same_values(
            &request.binding,
            &request.expected_values,
            &values,
            request.tracking.as_ref(),
        )?;
        let reviewed = state
            .transfers
            .reviews
            .get(&request.binding.loop_id)
            .ok_or("read transfer status before apply")?;
        if reviewed.binding_revision != binding_revision
            || reviewed.program_revision != program_revision
        {
            return Err("binding changed since review".into());
        }
        if state
            .transfers
            .records
            .get(&request.binding.loop_id)
            .is_some_and(|record| {
                record.transaction_id.as_deref() == Some(request.transaction_id.as_str())
                    && matches!(
                        record.status.as_str(),
                        "applied" | "failed" | "rejected" | "cancelled"
                    )
            })
        {
            return Err("completed transfer transaction cannot be retried".into());
        }
        let record = state
            .transfers
            .records
            .entry(request.binding.loop_id.clone())
            .or_default();
        record.status = "pending".into();
        record.reason = None;
        record.transaction_id = Some(request.transaction_id.clone());
        record.transaction_program_revision = Some(program_revision);
        record.transaction_binding_revision = Some(binding_revision);
        record.observed_values = None;
        record.confirmation = None;
        record.initial_values = None;
        record.applied_values = None;
        record.restored = false;
        state.transfers.used_transactions.insert(transaction_key);
        let applied_values = request.values.clone();
        state.transfers.pending = Some(Pending {
            request,
            committed: false,
            post_st_verified: false,
            initial_outputs,
            boundary_outputs: BTreeMap::new(),
            restore_points: Vec::new(),
            applied_values,
            measurement_before,
            entry_allowance: 0.0,
        });
        Ok(json!({"status":"pending","confirmed":false}))
    }

    /// Whether a transfer waits for, or was committed at, its task's boundary.
    pub(crate) fn has_transfer_transaction(&self) -> bool {
        self.lock().transfers.has_pending()
    }

    pub(crate) fn transfer_cancel(&self, params: JsonValue) -> Result<JsonValue, String> {
        let request: CancelRequest =
            serde_json::from_value(params).map_err(|error| error.to_string())?;
        let mut state = self.lock();
        if request.transaction_id.is_empty()
            || request.transaction_id.chars().count() > MAX_IDENTIFIER_CHARS
            || state.transfers.epoch_revision.as_ref() != Some(&request.expected_program_revision)
        {
            return Err("read the current transfer epoch before cancelling its identity".into());
        }
        let binding_revision = state
            .transfers
            .reviews
            .get(&request.loop_id)
            .ok_or("transfer unavailable")?
            .binding_revision
            .clone();
        let key = (
            request.expected_program_revision.clone(),
            request.transaction_id.clone(),
        );
        if state.transfers.cancelled_before_dispatch.contains(&key)
            || !state.transfers.used_transactions.contains(&key)
        {
            if !state.transfers.used_transactions.contains(&key)
                && state.transfers.used_transactions.len() >= MAX_TRANSACTION_IDENTITIES
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

    /// Commits a pending transfer at its owning task's start, before the task's programs run
    /// (E6.6): `task` is the task's name, `$background` for the programs outside every task, and
    /// `program` the program at that access point, written through the runtime's checks
    /// (specification 44 §13.3). Its members are written whole or not at all. `physical_target` is
    /// the runtime's target fact: a physical target needs the approval that the request carries.
    pub(crate) fn commit_transfer(
        &self,
        task: &str,
        program: &mut dyn ProgramWrite,
        physical_target: bool,
    ) {
        let mut state = self.lock();
        let Some(mut pending) = state.transfers.pending.take() else {
            return;
        };
        if !pending.request.binding.task_name.eq_ignore_ascii_case(task) {
            state.transfers.pending = Some(pending);
            return;
        }
        let request = &pending.request;
        let measurement_before = pending.measurement_before;
        let mut entry_allowance = 0.0;
        let result = (|| -> Result<StagedTransfer, String> {
            let image = program.program_image();
            if request.expected_program_revision != state.parameters.program_revision(&image) {
                return Err("program changed before task boundary".into());
            }
            let record = state
                .transfers
                .records
                .get(&request.binding.loop_id)
                .ok_or("transfer binding unavailable")?;
            if record.transaction_binding_revision.as_deref()
                != Some(request.expected_binding_revision.as_str())
                || record.transaction_program_revision.as_deref()
                    != Some(request.expected_program_revision.as_str())
            {
                return Err("transfer binding changed before task boundary".into());
            }
            check_binding(&request.binding, &image)?;
            if physical_target && !request.physical_approved {
                return Err("attached I/O requires explicit physical review".into());
            }
            if !state.live_values.forced_vars.is_empty() || !state.live_values.forced_io.is_empty()
            {
                return Err("forcing changed before task boundary".into());
            }
            let program: &dyn ProgramRead = &*program;
            let bound = program_facts::bound_places(&image);
            let mut current = observed(&request.binding, program)?;
            same_values(
                &request.binding,
                &request.expected_values,
                &current,
                request.tracking.as_ref(),
            )?;
            for member in &request.binding.states {
                check_io_target(&member.symbol, &[IoArea::Memory], program, &bound)?;
            }
            for mode in std::iter::once(&request.binding.mode)
                .chain(request.binding.additional_modes.iter())
            {
                check_io_target(&mode.symbol, &[], program, &bound)?;
            }
            let mut held_outputs = BTreeMap::new();
            for output in std::iter::once(&request.binding.output)
                .chain(request.binding.additional_outputs.iter())
            {
                check_io_target(
                    &output.symbol,
                    &[IoArea::Output, IoArea::Memory],
                    program,
                    &bound,
                )?;
                let measured = request.binding.tracking_outputs.get(&output.symbol);
                // An output is held at its committed output image when it is addressed, and at its task-boundary
                // value when it is not (A5.3).
                let held = if let Some(feedback) = measured {
                    let member = request
                        .binding
                        .guards
                        .iter()
                        .find(|member| &member.symbol == feedback)
                        .ok_or("applied feedback is missing")?;
                    tracking::fresh_applied_feedback(member, program, &bound)?
                } else {
                    tracking::held_output(
                        output,
                        program,
                        &bound,
                        state.latest_io_snapshot.as_ref(),
                    )?
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
                    program,
                    &held_outputs,
                    request.manual,
                )?
            } else {
                request.values.clone()
            };
            // The error the controller enters Auto with, and the measurement's change before the
            // entry, are those of this scan (A6.4, E6.9).
            if let Some(policy) = &request.tracking {
                entry_allowance = tracking::entry_allowance(
                    &request.binding,
                    policy,
                    &applied,
                    program,
                    request.entry_error_gain,
                    request.entry_measurement_gain.zip(measurement_before),
                )?;
            }
            // Each written member's value is taken before its write, so a rejected transfer can put it back.
            let mut planned = Vec::new();
            for mode in std::iter::once(&request.binding.mode)
                .chain(request.binding.additional_modes.iter())
            {
                let resolved = parameters::resolve_path(program, &mode.symbol)?;
                planned.push((
                    resolved.at,
                    resolved.value,
                    Value::Bool(if request.manual {
                        mode.manual_value
                    } else {
                        !mode.manual_value
                    }),
                    "transfer mode is not writable".to_string(),
                ));
            }
            for member in &request.binding.states {
                let resolved = parameters::resolve_path(program, &member.symbol)?;
                planned.push((
                    resolved.at,
                    resolved.value,
                    write_value(member, applied[&member.symbol])?,
                    format!("transfer state {} is not writable", member.symbol),
                ));
            }
            let boundary_outputs = std::iter::once(&request.binding.output)
                .chain(request.binding.additional_outputs.iter())
                .filter_map(|member| {
                    number(program, member)
                        .ok()
                        .map(|value| (member.symbol.clone(), value))
                })
                .collect();
            Ok((planned, applied, current, held_outputs, boundary_outputs))
        })();
        // The members are written together: a member the runtime refuses puts back the members
        // already written, so the task runs with the whole transfer or none of it.
        let result = result.and_then(
            |(planned, applied, initial, held_outputs, boundary_outputs)| {
                let mut restore_points: Vec<(Located, Value)> = Vec::with_capacity(planned.len());
                for (at, previous, value, refusal) in planned {
                    if program.write_at(&at, &value).is_err() {
                        for (at, previous) in restore_points.iter().rev() {
                            let _ = program.write_at(at, previous);
                        }
                        return Err(refusal);
                    }
                    restore_points.push((at, previous));
                }
                Ok((
                    applied,
                    initial,
                    held_outputs,
                    boundary_outputs,
                    restore_points,
                ))
            },
        );
        match result {
            Ok((applied, initial, held_outputs, boundary_outputs, restore_points)) => {
                if let Some(record) = state.transfers.records.get_mut(&request.binding.loop_id) {
                    record.initial_values = Some(initial);
                    record.applied_values = Some(applied.clone());
                }
                pending.applied_values = applied;
                pending.initial_outputs = held_outputs;
                pending.boundary_outputs = boundary_outputs;
                pending.restore_points = restore_points;
                pending.entry_allowance = entry_allowance;
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

    /// Checks a committed transfer at its task's end, `program` being the program at that access
    /// point: its modes and states as committed and its first output inside the reviewed tolerance
    /// (E6.7). A failed check reverts the whole transfer in the same scan, before the outputs are
    /// published.
    pub(crate) fn confirm_transfer_task(&self, task: &str, program: &mut dyn ProgramWrite) {
        let mut state = self.lock();
        let Some(mut pending) = state.transfers.pending.take() else {
            return;
        };
        if !pending.committed || !pending.request.binding.task_name.eq_ignore_ascii_case(task) {
            state.transfers.pending = Some(pending);
            return;
        }
        let request = &pending.request;
        let verification = (|| -> Result<JsonValue, String> {
            let program: &dyn ProgramRead = &*program;
            for mode in std::iter::once(&request.binding.mode)
                .chain(request.binding.additional_modes.iter())
            {
                let expected_mode = if request.manual {
                    mode.manual_value
                } else {
                    !mode.manual_value
                };
                if parameters::resolve_path(program, &mode.symbol)?.value
                    != Value::Bool(expected_mode)
                {
                    return Err("controller changed transfer mode before confirmation".into());
                }
            }
            for member in &request.binding.states {
                let actual = number(program, member)?;
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
                        program,
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
            let allowed = request.maximum_output_change + pending.entry_allowance;
            for member in std::iter::once(&request.binding.output)
                .chain(request.binding.additional_outputs.iter())
            {
                let output = number(program, member)?;
                let tolerance = if member.symbol == request.binding.output.symbol {
                    allowed
                } else if primary_span > 0.0 {
                    allowed * span(member) / primary_span
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
            observed(&request.binding, program)
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
                // The whole transaction is reverted before the output commit: the members the commit wrote return to
                // the values they held before it, and the output members to their task-boundary values. The rejected
                // output is never published, and the next scan starts from the state the transfer found.
                let restored = restore::restore(&pending, program);
                if let Some(record) = state
                    .transfers
                    .records
                    .get_mut(&pending.request.binding.loop_id)
                {
                    record.status = "failed".into();
                    record.reason = Some(reason);
                    record.restored = restored;
                }
            }
        }
    }

    /// A scan ended (its debug state published) without a fault (A5.3, E6.7): the epoch's snapshot is
    /// current from now on, so experiments may resolve variables against it; a parameter set its
    /// task's end confirmed or rejected takes that outcome, now that status reads the values it
    /// left (E7.2, E7.4); and a transfer verified at its task's end is applied, since its first
    /// output reached the output image.
    pub(crate) fn finish_transfer_cycle(&self) {
        let mut state = self.lock();
        state.parameters.mark_snapshot_current();
        state.parameters.settle();
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
    }

    /// Fails the pending transfer when the scan that runs its owning task faults, `program` being
    /// the program the fault left, before the fault's action is applied. A committed transfer wrote
    /// its members in this scan, so they are reverted in the same scan and `restored` reports
    /// whether every write-back succeeded (A6.3).
    pub(crate) fn fail_transfer(&self, reason: &str, program: &mut dyn ProgramWrite) {
        let mut state = self.lock();
        let Some(pending) = state.transfers.pending.take() else {
            return;
        };
        let restored = pending.committed && restore::restore(&pending, program);
        if let Some(record) = state
            .transfers
            .records
            .get_mut(&pending.request.binding.loop_id)
        {
            record.status = "failed".into();
            record.reason = Some(reason.into());
            record.restored = restored;
        }
    }
}
