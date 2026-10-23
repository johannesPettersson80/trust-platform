//! Stepped scans: the control request `cycle.step` (specification 11 §6.8.1, "Stepped scans"),
//! which every runtime answers alike, a computer and a chip (specification 34 STATE-08).
//!
//! [`request`] reads the request and checks the resource; the platform runs the step
//! ([`ResourcePort::step_cycle`]): a chip's loop through its cycle, trust-runtime on its scan
//! thread. A step writes the conformance suite's value texts and reads values in the suite's
//! encoding ([`parse_typed_value`], [`encode_value`]), so that the suite runs on a device and its
//! trace compares with the suite's expected artifacts (specification 44 §14.2).

mod values;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::{json, Map, Value as JsonValue};
use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::{Duration, Value};

pub use values::{encode_value, parse_typed_value};

use crate::ports::ResourcePort;
use crate::protocol::ControlResponse;
use crate::resource::replies::{
    busy_refusal, hung_refusal, no_installed_program, resource_not_running,
};
use crate::status::{invalid_params, validate_object_fields};

/// The members a `cycle.step` request's parameters may have.
const MEMBERS: &[&str] = &[
    "time_ns",
    "restart",
    "inputs",
    "direct_inputs",
    "watch",
    "watch_direct",
    "run",
];

/// One stepped scan as its request asks for it.
#[derive(Debug, Clone, PartialEq)]
pub struct StepRequest {
    /// The logical time of the scan.
    pub time: Duration,
    /// The restart before the scan.
    pub restart: Option<RestartMode>,
    /// The values written before the scan, each into the variable its name names, in the order of
    /// the names.
    pub inputs: Vec<(SmolStr, Value)>,
    /// The values written into the process images before the scan, by address, in the order of the
    /// addresses' texts.
    pub direct_inputs: Vec<(SmolStr, IoAddress, Value)>,
    /// The names whose values are read after the scan.
    pub watch: Vec<SmolStr>,
    /// The process-image addresses read after the scan, with their texts.
    pub watch_direct: Vec<(SmolStr, IoAddress)>,
    /// The program instance whose body runs once in place of the scheduler's cycle, as a test
    /// program runs; none for a cycle of the scheduler.
    pub run: Option<SmolStr>,
}

/// What a stepped scan read after it ran.
#[derive(Debug, Clone, PartialEq)]
pub struct StepOutcome {
    /// The value of each watched name, in the request's order.
    pub values: Vec<(SmolStr, Value)>,
    /// The value at each watched address, in the request's order.
    pub direct: Vec<(SmolStr, Value)>,
    /// The error the scan ended with; none for a scan that completed.
    pub error: Option<RuntimeError>,
}

/// Why a step ran no scan, or why its result cannot be given.
#[derive(Debug, Clone, PartialEq)]
pub enum StepRefusal {
    /// The first step since the resource stopped carries no restart. Nothing changed.
    RestartRequired,
    /// The request's time is before the time of the previous step. Nothing changed.
    TimeBefore,
    /// A name names no variable, or names one in several programs. Nothing changed.
    Name {
        /// The name.
        name: SmolStr,
        /// Why it names no single variable.
        reason: String,
    },
    /// The restart before the scan failed; the program is as the failed restart left it.
    Restart(RuntimeError),
    /// A variable or an address does not take its value. The values written before it stay
    /// written, and no scan ran.
    Write {
        /// The name or the address.
        name: SmolStr,
        /// Why the value was refused.
        reason: String,
    },
    /// A watched value cannot be read after the scan ran.
    Read {
        /// The name or the address.
        name: SmolStr,
        /// Why it cannot be read.
        reason: String,
    },
    /// No program is installed, for this reason.
    NoProgram(String),
    /// The resource is not Stopped.
    NotStopped(ResourceState),
    /// The resource's command queue is full; the step was not queued.
    Busy,
    /// The step was queued, but no answer came, for this reason.
    Unavailable(String),
    /// The platform runs no stepped scans.
    Unsupported,
    /// Not a refusal: the step's cold restart loads a program installed since the program was
    /// loaded, which the platform does after the request's pass, and it writes the step's reply then.
    /// The request is answered `{"status": "loading"}` for its audit record only.
    Deferred,
}

impl StepRefusal {
    /// The refusal of a name that names no single variable.
    pub fn name(name: &str, reason: impl Into<String>) -> Self {
        Self::Name {
            name: SmolStr::new(name),
            reason: reason.into(),
        }
    }

    /// The refusal of a value that the variable or the address `name` does not take.
    pub fn write(name: &str, reason: &impl ToString) -> Self {
        Self::Write {
            name: SmolStr::new(name),
            reason: reason.to_string(),
        }
    }

    /// The refusal of a watched value that cannot be read.
    pub fn read(name: &str, reason: &impl ToString) -> Self {
        Self::Read {
            name: SmolStr::new(name),
            reason: reason.to_string(),
        }
    }

    /// The reply that refuses request `id` with this refusal.
    #[must_use]
    pub fn response(&self, id: u64) -> ControlResponse {
        let message = match self {
            Self::RestartRequired => "restart required".to_string(),
            Self::TimeBefore => "time before the last step".to_string(),
            Self::Name { name, reason } => format!("'{name}' {reason}"),
            Self::Restart(error) => format!("restart failed: {error}"),
            Self::Write { name, reason } => format!("set '{name}': {reason}"),
            Self::Read { name, reason } => format!("read '{name}': {reason}"),
            Self::NoProgram(reason) => return no_installed_program(id, reason),
            Self::NotStopped(state) => return resource_not_running(id, *state),
            Self::Busy => return busy_refusal(id),
            Self::Unavailable(reason) => reason.clone(),
            Self::Unsupported => "unsupported request".to_string(),
            Self::Deferred => return ControlResponse::ok(id, json!({"status": "loading"})),
        };
        ControlResponse::error(id, message)
    }
}

/// The time a step's restart happens at, the time of the previous step since the resource stopped
/// (`last`), zero for the first: the logical time goes on across a stepped restart, as the
/// conformance runner's time does. The first step since the resource stopped must restart the
/// program, and no step's time is before the previous step's.
///
/// # Errors
///
/// [`StepRefusal::RestartRequired`] and [`StepRefusal::TimeBefore`].
pub fn restart_time(
    request: &StepRequest,
    last: Option<Duration>,
) -> Result<Duration, StepRefusal> {
    match last {
        None if request.restart.is_none() => Err(StepRefusal::RestartRequired),
        Some(last) if request.time < last => Err(StepRefusal::TimeBefore),
        last => Ok(last.unwrap_or(Duration::ZERO)),
    }
}

/// Answers `cycle.step`. The request pipeline refuses every role below Engineer, and every request
/// while the debug surface is off, before this runs.
///
/// The first failing check is the reply, in this order: the parameters; a hung resource; a resource
/// that is not Stopped, refused as `program.install` is (`resource_not_running` with its state); a
/// device without an installed program; and then what the platform's step refuses.
#[must_use]
pub fn request<H: ResourcePort>(id: u64, params: Option<&JsonValue>, host: &H) -> ControlResponse {
    let request = match parse(params) {
        Ok(request) => request,
        Err(reason) => return invalid_params(id, &reason),
    };
    if host.is_hung() {
        return hung_refusal(id);
    }
    let resource = host.resource_state();
    if !matches!(resource, ResourceState::Stopped) {
        return resource_not_running(id, resource);
    }
    if let Some(reason) = host.missing_program() {
        return no_installed_program(id, reason);
    }
    match host.step_cycle(&request) {
        Ok(outcome) => ControlResponse::ok(id, reply(request.time, &outcome)),
        Err(refusal) => refusal.response(id),
    }
}

/// The result of a step: `{"time_ns", "values", "direct", "errors"}`, the values in the encoding of
/// [`encode_value`] and `errors` the text of the error the scan ended with.
#[must_use]
pub fn reply(time: Duration, outcome: &StepOutcome) -> JsonValue {
    let encoded = |entries: &[(SmolStr, Value)]| {
        entries
            .iter()
            .map(|(name, value)| (name.to_string(), encode_value(value)))
            .collect::<BTreeMap<_, _>>()
    };
    json!({
        "time_ns": time.as_nanos(),
        "values": encoded(&outcome.values),
        "direct": encoded(&outcome.direct),
        "errors": outcome.error.iter().map(ToString::to_string).collect::<Vec<_>>(),
    })
}

/// The step `params` ask for, or why they are refused.
fn parse(params: Option<&JsonValue>) -> Result<StepRequest, String> {
    let params = params.ok_or("missing params")?;
    validate_object_fields(params, MEMBERS, MEMBERS)?;
    let time = params
        .get("time_ns")
        .ok_or("time_ns is required")?
        .as_u64()
        .and_then(|nanos| i64::try_from(nanos).ok())
        .map(Duration::from_nanos)
        .ok_or("time_ns must be a whole number of nanoseconds")?;
    let restart = match params.get("restart") {
        None => None,
        Some(mode) => match mode.as_str().map(str::to_ascii_lowercase).as_deref() {
            Some("cold") => Some(RestartMode::Cold),
            Some("warm") => Some(RestartMode::Warm),
            _ => return Err("restart must be \"cold\" or \"warm\"".to_string()),
        },
    };
    let inputs = texts(params, "inputs")?
        .into_iter()
        .map(|(name, text)| {
            let value =
                parse_typed_value(text).map_err(|reason| format!("inputs.{name}: {reason}"))?;
            Ok((SmolStr::new(name), value))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let direct_inputs = texts(params, "direct_inputs")?
        .into_iter()
        .map(|(address, text)| {
            let value = parse_typed_value(text)
                .map_err(|reason| format!("direct_inputs.{address}: {reason}"))?;
            Ok((SmolStr::new(address), parse_address(address)?, value))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let watch = names(params, "watch")?
        .into_iter()
        .map(SmolStr::new)
        .collect();
    let watch_direct = names(params, "watch_direct")?
        .into_iter()
        .map(|address| Ok((SmolStr::new(address), parse_address(address)?)))
        .collect::<Result<Vec<_>, String>>()?;
    let run = match params.get("run") {
        None => None,
        Some(name) => Some(SmolStr::new(
            name.as_str()
                .filter(|name| !name.is_empty())
                .ok_or("run must name a program instance")?,
        )),
    };
    Ok(StepRequest {
        time,
        restart,
        inputs,
        direct_inputs,
        watch,
        watch_direct,
        run,
    })
}

/// The members of object `member` of `params`, each a name or an address with a value text, in the
/// order of the names; none when `params` has no such member.
fn texts<'p>(params: &'p JsonValue, member: &str) -> Result<Vec<(&'p str, &'p str)>, String> {
    let Some(value) = params.get(member) else {
        return Ok(Vec::new());
    };
    let object: &Map<String, JsonValue> = value
        .as_object()
        .ok_or_else(|| format!("{member} must be an object"))?;
    let mut entries = object
        .iter()
        .map(|(name, text)| {
            let text = text
                .as_str()
                .ok_or_else(|| format!("{member}.{name} must be a value text"))?;
            Ok((name.as_str(), text))
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
    Ok(entries)
}

/// The texts of array `member` of `params`; none when `params` has no such member.
fn names<'p>(params: &'p JsonValue, member: &str) -> Result<Vec<&'p str>, String> {
    let Some(value) = params.get(member) else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or_else(|| format!("{member} must be an array of texts"))?
        .iter()
        .map(|name| {
            name.as_str()
                .ok_or_else(|| format!("{member} must be an array of texts"))
        })
        .collect()
}

/// The process-image address `text` names.
fn parse_address(text: &str) -> Result<IoAddress, String> {
    IoAddress::parse(text).map_err(|error| format!("'{text}': {error}"))
}

#[cfg(test)]
mod contract_tests;
