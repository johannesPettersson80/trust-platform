use serde_json::{json, Value as JsonValue};
use std::collections::{HashMap, HashSet};
use trust_runtime::debug::DebugSnapshot;
use trust_runtime::memory::InstanceId;
use trust_runtime::value::Value;
use trust_runtime::RuntimeMetadata;

/// Compiled controller identity and symbolic leaves for explicit Control Studio binding.
/// The existing HMI catalog retains its own operator-facing selection rules.
pub(super) fn catalog(
    metadata: &RuntimeMetadata,
    snapshot: &DebugSnapshot,
    addressed: &[JsonValue],
) -> Result<JsonValue, String> {
    let addresses = addressed
        .iter()
        .filter_map(|entry| {
            Some((
                entry["symbol"].as_str()?.to_ascii_uppercase(),
                entry["address"].clone(),
            ))
        })
        .collect::<HashMap<_, _>>();
    let mut collector = Collector {
        snapshot,
        addresses,
        symbols: Vec::new(),
        controllers: Vec::new(),
        ancestors: HashSet::new(),
        visited: 0,
    };
    for (name, value) in snapshot.storage.globals() {
        let tasks = metadata.tasks().iter()
            .filter(|task| task.programs.iter().any(|program| program.eq_ignore_ascii_case(name)))
            .map(|task| json!({ "name": task.name.as_str(), "intervalMs": task.interval.as_nanos() as f64 / 1_000_000.0 }))
            .collect::<Vec<_>>();
        collector.walk(name.as_str(), value, &tasks, 0)?;
    }
    collector
        .symbols
        .sort_by(|a, b| a["symbol"].as_str().cmp(&b["symbol"].as_str()));
    collector
        .controllers
        .sort_by(|a, b| a["instance"].as_str().cmp(&b["instance"].as_str()));
    Ok(json!({ "symbols": collector.symbols, "controllers": collector.controllers }))
}

struct Collector<'a> {
    snapshot: &'a DebugSnapshot,
    addresses: HashMap<String, JsonValue>,
    symbols: Vec<JsonValue>,
    controllers: Vec<JsonValue>,
    ancestors: HashSet<InstanceId>,
    visited: usize,
}

impl Collector<'_> {
    fn walk(
        &mut self,
        path: &str,
        value: &Value,
        tasks: &[JsonValue],
        depth: usize,
    ) -> Result<(), String> {
        self.visited += 1;
        if depth > 32 || self.visited > 16_384 {
            return Err(
                "The compiled control symbol catalog exceeds its bounded traversal limit.".into(),
            );
        }
        match value {
            Value::Instance(id) => {
                if !self.ancestors.insert(*id) {
                    return Err(format!("Recursive controller instance at {path}."));
                }
                let instance = self
                    .snapshot
                    .storage
                    .get_instance(*id)
                    .ok_or_else(|| format!("Compiled instance {path} is unavailable."))?;
                self.controllers.push(json!({ "instance": path, "type": instance.type_name.as_str(), "tasks": tasks }));
                for (name, value) in &instance.variables {
                    self.walk(&format!("{path}.{name}"), value, tasks, depth + 1)?;
                }
                self.ancestors.remove(id);
            }
            Value::Struct(value) => {
                for (name, field) in value.fields() {
                    self.walk(&format!("{path}.{name}"), field, tasks, depth + 1)?;
                }
            }
            Value::Array(value) => {
                for (offset, element) in value.elements().iter().enumerate() {
                    let mut remaining = offset as i64;
                    let mut indices = Vec::new();
                    for &(lower, upper) in value.dimensions().iter().rev() {
                        let width = upper
                            .checked_sub(lower)
                            .and_then(|value| value.checked_add(1))
                            .filter(|width| *width > 0)
                            .ok_or("Invalid compiled array bounds.")?;
                        indices.push((lower + remaining % width).to_string());
                        remaining /= width;
                    }
                    indices.reverse();
                    self.walk(
                        &format!("{path}[{}]", indices.join(",")),
                        element,
                        tasks,
                        depth + 1,
                    )?;
                }
            }
            Value::Reference(_) | Value::Null => {}
            _ => {
                if let Some(iec_type) = trust_runtime::debug::dap::value_type_name(value) {
                    let mut entry = json!({ "symbol": path, "type": iec_type });
                    if let Some(address) = self.addresses.get(&path.to_ascii_uppercase()) {
                        entry["address"] = address.clone();
                    }
                    self.symbols.push(entry);
                }
            }
        }
        Ok(())
    }
}
