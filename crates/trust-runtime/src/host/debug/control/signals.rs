//! Bounded selections captured coherently from the runtime's scan publisher.
use super::recording::{numeric, overlap, snapshot_value};
use super::{DebugControl, ForcedVar};
use crate::io::{IoAddress, IoSnapshot};
use crate::memory::Generation;
use crate::runtime::{Located, ProgramRead};
use crate::value::Value;
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
    /// A program variable by its path (Specification 35, control signal reads): a member of an
    /// instance or one it inherits (`EXTENDS`), a structure field below it, read at the place the
    /// path resolved to when the selection was made.
    Symbol {
        path: String,
        at: Located,
    },
}

/// Whether a value is of a channel's declared type: its elementary type name, or for an
/// enumeration the enumeration's name, with or without its namespace on either side; an
/// enumeration is read as its numeric value.
pub(super) fn of_declared_type(value: &Value, iec_type: &str) -> bool {
    match value {
        Value::Enum(value) => {
            let last = |name: &str| name.rsplit('.').next().unwrap_or(name).to_ascii_uppercase();
            last(value.type_name()) == last(iec_type)
        }
        _ => crate::debug::dap::value_type_name(value).as_deref() == Some(iec_type),
    }
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
impl Inspect {
    /// Refuses a request pinned to a program revision other than `revision` (E3, rule 8).
    fn check_revision(&self, revision: &str) -> Result<(), String> {
        if self
            .expected_program_revision
            .as_deref()
            .is_some_and(|expected| expected != revision)
        {
            return Err("program revision changed".into());
        }
        Ok(())
    }

    /// The answer that waits for the next coherent scan of the selection (E3).
    fn waiting(&self, revision: &str) -> JsonValue {
        json!({"program_revision":revision,"loop_revision":self.loop_revision,"bindings":self.bindings,"available":false,"reason":"Waiting for the next coherent runtime scan."})
    }
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
    program: Option<&dyn ProgramRead>,
) -> Result<Vec<Target>, String> {
    if bindings.is_empty() || bindings.len() > 32 {
        return Err("select 1 to 32 channels".into());
    }
    let mut ids = HashSet::new();
    let mut selectors: Vec<Located> = Vec::new();
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
                if numeric(value).is_none() || !of_declared_type(value, &binding.iec_type) {
                    return Err("addressed signal type mismatch".into());
                }
                addresses.push(address.clone());
                Target::Address(address)
            }
            (None, Some(path)) => {
                if path.len() > 256 {
                    return Err("symbolic member path exceeds supported length".into());
                }
                let resolved = super::symbol_path::resolve(
                    program.ok_or("current symbolic storage unavailable")?,
                    path,
                )?;
                if selectors.contains(&resolved.at) {
                    return Err("duplicate symbolic target".into());
                }
                selectors.push(resolved.at);
                if numeric(&resolved.value).is_none()
                    || !of_declared_type(&resolved.value, &binding.iec_type)
                {
                    return Err("symbolic signal type mismatch".into());
                }
                Target::Symbol {
                    path: path.clone(),
                    at: resolved.at,
                }
            }
            _ => return Err("each channel requires exactly one address or symbol selector".into()),
        };
        targets.push(target);
    }
    Ok(targets)
}
/// The values of `targets` in one scan: an address from its I/O snapshot, a variable from
/// `program`, the program as the scan left it, whose forces `forces` holds with the program
/// generation they apply to.
pub(super) fn capture_values(
    targets: &[Target],
    bindings: &[SignalBinding],
    snapshot: &IoSnapshot,
    program: Option<&dyn ProgramRead>,
    (forced_vars, generation): (&[ForcedVar], Generation),
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
            Target::Symbol { path, at } => {
                // Resolved again each scan: a reloaded program that moved the variable reads
                // as missing rather than as another variable's value.
                let value = program
                    .and_then(|program| super::symbol_path::resolve(program, path).ok())
                    .filter(|resolved| resolved.at == *at)
                    .map(|resolved| resolved.value);
                let forced = program.is_some_and(|program| {
                    forced_vars.iter().any(|force| {
                        super::symbol_path::forced_holds(program, generation, &force.target, at)
                    })
                }) || snapshot
                    .inputs
                    .iter()
                    .chain(&snapshot.outputs)
                    .chain(&snapshot.memory)
                    .filter(|entry| {
                        entry
                            .name
                            .as_ref()
                            .is_some_and(|name| name.eq_ignore_ascii_case(path))
                    })
                    .any(|entry| {
                        snapshot
                            .forced
                            .iter()
                            .any(|address| overlap(&entry.address, address))
                    });
                (value, forced)
            }
        };
        let (number, quality) = match value {
            None => (None, Quality::Missing),
            Some(value) => {
                match numeric(&value).filter(|_| of_declared_type(&value, &binding.iec_type)) {
                    Some(number) => (
                        Some(number),
                        if forced {
                            Quality::Forced
                        } else {
                            Quality::Good
                        },
                    ),
                    None => (None, Quality::Bad),
                }
            }
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
    /// Captures a published scan for the watch, `program` being the program as the scan left it
    /// in the program generation `generation`.
    pub(super) fn capture(
        &mut self,
        snapshot: &IoSnapshot,
        time: i64,
        program: &dyn ProgramRead,
        forced: &[ForcedVar],
        generation: Generation,
    ) {
        let Some(watch) = self.watch.as_mut() else {
            return;
        };
        let captured = capture_values(
            &watch.targets,
            &watch.bindings,
            snapshot,
            Some(program),
            (forced, generation),
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
    /// Selects the signals Control Studio inspects, or answers the latest coherent scan of the
    /// selection already made (protocol annex E3), reading the program as the last scan left it.
    /// Before the program's first complete scan the answer waits for one.
    pub(crate) fn inspect_control_signals(&self, params: JsonValue) -> Result<JsonValue, String> {
        let request: Inspect = serde_json::from_value(params).map_err(|e| e.to_string())?;
        if request.loop_revision.is_empty() || request.loop_revision.len() > 256 {
            return Err("loop revision required".into());
        }
        let Some(program) = self.published_program() else {
            return self.inspect_before_first_scan(&request);
        };
        let mut state = self.lock();
        let revision = state.parameters.program_revision(&program.program_image());
        request.check_revision(&revision)?;
        if let Some(watch) = state.signals.watch.as_ref()
            && watch.loop_revision == request.loop_revision
            && watch.program_revision == revision
            && watch.bindings == request.bindings
        {
            return Ok(watch
                .latest
                .clone()
                .unwrap_or_else(|| request.waiting(&revision)));
        }
        let Some(snapshot) = state.latest_io_snapshot.as_ref() else {
            return Ok(request.waiting(&revision));
        };
        let targets = resolve_bindings(&request.bindings, snapshot, Some(&*program))?;
        let result = request.waiting(&revision);
        state.signals.watch = Some(Watch {
            loop_revision: request.loop_revision,
            program_revision: revision,
            bindings: request.bindings,
            targets,
            latest: None,
        });
        Ok(result)
    }

    /// The answer while the program set has completed no scan, at start or after a new program
    /// (annex E3): it waits for one, under the revision of that program. A runtime with debugging
    /// disabled publishes no scan to wait for, and refuses.
    fn inspect_before_first_scan(&self, request: &Inspect) -> Result<JsonValue, String> {
        let image = self
            .program_image()
            .filter(|_| self.is_enabled())
            .ok_or("current runtime scan unavailable")?;
        let revision = self.lock().parameters.program_revision(&image.image());
        request.check_revision(&revision)?;
        Ok(request.waiting(&revision))
    }
}
