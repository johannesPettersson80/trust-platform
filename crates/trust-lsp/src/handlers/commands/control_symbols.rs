use serde_json::{json, Value as JsonValue};
use std::collections::{HashMap, HashSet};
use trust_hir::db::GlobalOwner;
use trust_hir::types::TypeRegistry;
use trust_hir::Type;
use trust_runtime::debug::DebugSnapshot;
use trust_runtime::memory::InstanceId;
use trust_runtime::value::Value;
use trust_runtime::RuntimeMetadata;

/// Compiled controller identity and symbolic leaves for explicit Control Studio binding, an
/// instance's inherited members among its own (Specification 35 §17.1). An enumeration lists its
/// type's members, from which setup chooses the ones that mean Manual and Auto (A6.1). A global's
/// leaves carry its owner, from which setup refuses what a task may not write (protocol annex E2.3).
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
        registry: metadata.registry(),
        addresses,
        symbols: Vec::new(),
        controllers: Vec::new(),
        ancestors: HashSet::new(),
        visited: 0,
        owner: None,
    };
    for (name, value) in snapshot.storage.globals() {
        let tasks = metadata.tasks().iter()
            .filter(|task| task.programs.iter().any(|program| program.eq_ignore_ascii_case(name)))
            .map(|task| json!({ "name": task.name.as_str(), "intervalMs": task.interval.as_nanos() as f64 / 1_000_000.0 }))
            .collect::<Vec<_>>();
        collector.owner = metadata.global_owner(name).map(owner_json);
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
    registry: &'a TypeRegistry,
    addresses: HashMap<String, JsonValue>,
    symbols: Vec<JsonValue>,
    controllers: Vec<JsonValue>,
    ancestors: HashSet<InstanceId>,
    visited: usize,
    /// The owner of the global being walked, which each of its leaves carries; `None` under a
    /// program instance.
    owner: Option<JsonValue>,
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
                let snapshot = self.snapshot;
                let instance = snapshot
                    .storage
                    .get_instance(*id)
                    .ok_or_else(|| format!("Compiled instance {path} is unavailable."))?;
                self.controllers.push(json!({ "instance": path, "type": instance.type_name.as_str(), "tasks": tasks }));
                // A block that EXTENDS another keeps the inherited members in its base instance;
                // they are the block's own members by name (`Main.TT101.Value`).
                let mut named = HashSet::new();
                let mut owner = Some(instance);
                let mut bases = 0usize;
                while let Some(current) = owner {
                    for (name, value) in &current.variables {
                        if named.insert(name.to_ascii_uppercase()) {
                            self.walk(&format!("{path}.{name}"), value, tasks, depth + 1)?;
                        }
                    }
                    owner = match current.parent {
                        Some(base) => {
                            bases += 1;
                            if bases > 32 {
                                return Err(format!(
                                    "Compiled instance {path} has too many bases."
                                ));
                            }
                            Some(snapshot.storage.get_instance(base).ok_or_else(|| {
                                format!("The base of compiled instance {path} is unavailable.")
                            })?)
                        }
                        None => None,
                    };
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
                    if let Value::Enum(enumeration) = value {
                        entry["members"] = json!(members(self.registry, enumeration.type_name()));
                    }
                    if let Some(owner) = &self.owner {
                        entry["owner"] = owner.clone();
                    }
                    self.symbols.push(entry);
                }
            }
        }
        Ok(())
    }
}

/// A global's owner as the catalog states it (specification 35 §17.1; protocol annex E2.3):
/// `{"kind": "task", "task"}` for the one task whose programs alone write it, `unwritten` when no
/// program writes it, `background` when only programs outside every task do, and `shared` when
/// several tasks or a function, block, method or class do.
fn owner_json(owner: &GlobalOwner) -> JsonValue {
    match owner {
        GlobalOwner::Task(task) => json!({ "kind": "task", "task": task.as_str() }),
        GlobalOwner::Unwritten => json!({ "kind": "unwritten" }),
        GlobalOwner::Background => json!({ "kind": "background" }),
        GlobalOwner::Shared => json!({ "kind": "shared" }),
    }
}

/// The members of the enumeration type `type_name`, in declaration order, through its aliases.
fn members(registry: &TypeRegistry, type_name: &str) -> Vec<String> {
    let mut current = registry.lookup(type_name);
    for _ in 0..16 {
        match current.and_then(|id| registry.get(id)) {
            Some(Type::Alias { target, .. }) => current = Some(*target),
            Some(Type::Enum { values, .. }) => {
                return values.iter().map(|(name, _)| name.to_string()).collect();
            }
            _ => break,
        }
    }
    Vec::new()
}
