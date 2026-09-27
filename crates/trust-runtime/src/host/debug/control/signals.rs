//! Bounded selections captured coherently from the runtime's scan publisher.
use super::recording::{numeric, overlap, snapshot_value};
use super::{DebugControl, ForcedVar, ForcedVarTarget};
use crate::io::{IoAddress, IoSnapshot};
use crate::memory::{InstanceId, VariableStorage};
use crate::runtime::RuntimeMetadata;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::collections::HashSet;
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SignalBinding {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub iec_type: String,
}
#[derive(Debug, Clone)]
pub(super) enum Target {
    Address(IoAddress),
    Symbol {
        path: String,
        id: InstanceId,
        member: String,
    },
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Good,
    Bad,
    Forced,
    Missing,
}
#[derive(Debug, Clone)]
pub(super) struct Captured {
    pub values: Vec<Option<f64>>,
    pub forced: Vec<bool>,
    pub quality: Vec<Quality>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inspect {
    loop_revision: String,
    bindings: Vec<SignalBinding>,
    expected_program_revision: Option<String>,
}
#[derive(Debug)]
struct Watch {
    loop_revision: String,
    program_revision: String,
    bindings: Vec<SignalBinding>,
    targets: Vec<Target>,
    latest: Option<JsonValue>,
}
#[derive(Debug, Default)]
pub(super) struct SignalRegistry {
    watch: Option<Watch>,
}
pub(super) fn resolve_bindings(
    bindings: &[SignalBinding],
    snapshot: &IoSnapshot,
    storage: Option<&VariableStorage>,
) -> Result<Vec<Target>, String> {
    if bindings.is_empty() || bindings.len() > 32 {
        return Err("select 1 to 32 channels".into());
    }
    let mut ids = HashSet::new();
    let mut selectors = HashSet::new();
    let mut addresses = Vec::new();
    let mut targets = Vec::new();
    for binding in bindings {
        if binding.id.is_empty() || binding.id.len() > 128 || !ids.insert(&binding.id) {
            return Err("signal IDs must be bounded and unique".into());
        }
        let target = match (&binding.address, &binding.symbol) {
            (Some(address), None) => {
                let address = IoAddress::parse(address).map_err(|e| e.to_string())?;
                if address.wildcard
                    || address.path.len() != 1
                    || addresses.iter().any(|other| overlap(&address, other))
                {
                    return Err("addresses must be explicit and non-overlapping".into());
                }
                let value =
                    snapshot_value(snapshot, &address).ok_or("addressed signal unavailable")?;
                if numeric(value).is_none()
                    || crate::debug::dap::value_type_name(value).as_deref()
                        != Some(binding.iec_type.as_str())
                {
                    return Err("addressed signal type mismatch".into());
                }
                addresses.push(address.clone());
                Target::Address(address)
            }
            (None, Some(path)) => {
                if path.len() > 256 {
                    return Err("symbolic member path exceeds supported length".into());
                }
                let (id, member, value) = super::parameters::resolve(
                    storage.ok_or("current symbolic storage unavailable")?,
                    path,
                )?;
                if !selectors.insert((id, member.to_ascii_uppercase())) {
                    return Err("duplicate symbolic target".into());
                }
                if numeric(&value).is_none()
                    || crate::debug::dap::value_type_name(&value).as_deref()
                        != Some(binding.iec_type.as_str())
                {
                    return Err("symbolic signal type mismatch".into());
                }
                Target::Symbol {
                    path: path.clone(),
                    id,
                    member,
                }
            }
            _ => return Err("each channel requires exactly one address or symbol selector".into()),
        };
        targets.push(target);
    }
    Ok(targets)
}
pub(super) fn capture_values(
    targets: &[Target],
    bindings: &[SignalBinding],
    snapshot: &IoSnapshot,
    storage: Option<&VariableStorage>,
    forced_vars: &[ForcedVar],
) -> Captured {
    let mut captured = Captured {
        values: Vec::new(),
        forced: Vec::new(),
        quality: Vec::new(),
    };
    for (target, binding) in targets.iter().zip(bindings) {
        let (value, forced) = match target {
            Target::Address(address) => (
                snapshot_value(snapshot, address).cloned(),
                snapshot.forced.iter().any(|other| overlap(address, other)),
            ),
            Target::Symbol { path, id, member } => {
                let value = storage
                    .and_then(|s| super::parameters::resolve(s, path).ok())
                    .filter(|(actual_id, actual_member, _)| {
                        actual_id == id && actual_member.eq_ignore_ascii_case(member)
                    })
                    .map(|(_, _, value)| value);
                let forced=forced_vars.iter().any(|force|matches!(&force.target,ForcedVarTarget::Instance(actual_id,actual_member) if actual_id==id&&actual_member.eq_ignore_ascii_case(member)))
                ||snapshot.inputs.iter().chain(&snapshot.outputs).chain(&snapshot.memory).filter(|entry|entry.name.as_ref().is_some_and(|name|name.eq_ignore_ascii_case(path))).any(|entry|snapshot.forced.iter().any(|address|overlap(&entry.address,address)));
                (value, forced)
            }
        };
        let (number, quality) = match value {
            None => (None, Quality::Missing),
            Some(value) => match numeric(&value).filter(|_| {
                crate::debug::dap::value_type_name(&value).as_deref()
                    == Some(binding.iec_type.as_str())
            }) {
                Some(number) => (
                    Some(number),
                    if forced {
                        Quality::Forced
                    } else {
                        Quality::Good
                    },
                ),
                None => (None, Quality::Bad),
            },
        };
        captured.values.push(number);
        captured.forced.push(forced);
        captured.quality.push(quality);
    }
    captured
}
impl SignalRegistry {
    pub(super) fn clear(&mut self) {
        self.watch = None;
    }
    pub(super) fn ready_for(
        &self,
        loop_revision: &str,
        program_revision: &str,
        bindings: &[SignalBinding],
        scan: Option<u64>,
    ) -> bool {
        self.watch.as_ref().is_some_and(|watch| {
            watch.loop_revision == loop_revision
                && watch.program_revision == program_revision
                && watch.bindings == bindings
                && watch.latest.as_ref().is_some_and(|latest| {
                    latest["available"] == true && latest["scan"].as_u64() == scan
                })
        })
    }
    pub(super) fn capture(
        &mut self,
        snapshot: &IoSnapshot,
        time: i64,
        storage: &VariableStorage,
        forced: &[ForcedVar],
    ) {
        let Some(watch) = self.watch.as_mut() else {
            return;
        };
        let captured = capture_values(
            &watch.targets,
            &watch.bindings,
            snapshot,
            Some(storage),
            forced,
        );
        let wall = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as u64);
        watch.latest = Some(
            json!({"program_revision":watch.program_revision,"loop_revision":watch.loop_revision,"bindings":watch.bindings,"available":snapshot.scan.is_some()&&time>=0,"scan":snapshot.scan,"runtime_time_ns":time,"captured_at_ms":wall,"values":captured.values,"forced":captured.forced,"quality":captured.quality}),
        );
    }
}
impl DebugControl {
    pub(crate) fn inspect_control_signals(
        &self,
        params: JsonValue,
        metadata: &RuntimeMetadata,
    ) -> Result<JsonValue, String> {
        let request: Inspect = serde_json::from_value(params).map_err(|e| e.to_string())?;
        if request.loop_revision.is_empty() || request.loop_revision.len() > 256 {
            return Err("loop revision required".into());
        }
        let mut state = self.state.0.lock().map_err(|_| "debug state unavailable")?;
        let revision = state.parameters.program_revision(metadata);
        if request
            .expected_program_revision
            .as_ref()
            .is_some_and(|expected| expected != &revision)
        {
            return Err("program revision changed".into());
        }
        if let Some(watch) = state.signals.watch.as_ref() {
            if watch.loop_revision == request.loop_revision
                && watch.program_revision == revision
                && watch.bindings == request.bindings
            {
                return Ok(watch.latest.clone().unwrap_or_else(||json!({"program_revision":revision,"loop_revision":request.loop_revision,"bindings":request.bindings,"available":false,"reason":"Waiting for the next coherent runtime scan."})));
            }
        }
        let snapshot = state
            .latest_io_snapshot
            .as_ref()
            .ok_or("current runtime scan unavailable")?;
        let targets = resolve_bindings(
            &request.bindings,
            snapshot,
            state.snapshot.as_ref().map(|s| &s.storage),
        )?;
        let result = json!({"program_revision":revision,"loop_revision":request.loop_revision,"bindings":request.bindings,"available":false,"reason":"Waiting for the next coherent runtime scan."});
        state.signals.watch = Some(Watch {
            loop_revision: request.loop_revision,
            program_revision: revision,
            bindings: request.bindings,
            targets,
            latest: None,
        });
        Ok(result)
    }
}
