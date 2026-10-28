//! The PLC signals a plant can bind to (specification 47 §8): the compiled numeric process-image
//! bindings of the current project (`trust-lsp.hmiBindings`), commands from `%Q` outputs and `%M`
//! memory and feedback to `%I` inputs, each with its current value when live evidence has one, and
//! the enabled physical I/O drivers that own the process image.

use serde_json::{json, Map, Value};

use super::ports::canonical;

/// One compiled signal a port can bind to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Candidate {
    pub(super) symbol: String,
    pub(super) address: String,
    pub(super) data_type: String,
    pub(super) current_value: Option<String>,
}

/// The signals a plant's ports can bind to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::control::plant_handlers) struct Catalog {
    /// The project compiled; without that no signal is offered.
    pub(super) ready: bool,
    pub(super) commands: Vec<Candidate>,
    pub(super) feedbacks: Vec<Candidate>,
    /// The enabled physical I/O drivers, which block plant signal ownership.
    pub(super) physical_drivers: Vec<String>,
    /// Why no signal is offered.
    pub(super) error: Option<String>,
}

impl Catalog {
    fn unavailable(error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            ..Self::default()
        }
    }

    /// The catalog as the editor holds it (the `plant.signals` answer and the `signals` of a
    /// `plant.edit` request).
    pub(in crate::control::plant_handlers) fn to_json(&self) -> Value {
        let candidate = |candidate: &Candidate| {
            let mut entry = json!({
                "symbol": candidate.symbol,
                "address": candidate.address,
                "dataType": candidate.data_type,
            });
            if let Some(value) = &candidate.current_value {
                entry["currentValue"] = json!(value);
            }
            entry
        };
        let mut catalog = json!({
            "status": if self.ready { "ready" } else { "compile_error" },
            "commands": self.commands.iter().map(candidate).collect::<Vec<_>>(),
            "feedbacks": self.feedbacks.iter().map(candidate).collect::<Vec<_>>(),
        });
        if !self.physical_drivers.is_empty() {
            catalog["physicalDrivers"] = json!(self.physical_drivers);
        }
        if let Some(error) = &self.error {
            catalog["error"] = json!(error);
        }
        catalog
    }

    /// The catalog a `plant.edit` request carries; anything unreadable offers no signal.
    pub(super) fn from_json(value: Option<&Value>) -> Self {
        let Some(catalog) = value.and_then(Value::as_object) else {
            return Self::unavailable("Compiled PLC signals are not available yet.");
        };
        let candidates = |key: &str| {
            catalog
                .get(key)
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_object)
                        .map(|candidate| Candidate {
                            symbol: text(candidate, "symbol"),
                            address: text(candidate, "address"),
                            data_type: text(candidate, "dataType"),
                            current_value: candidate
                                .get("currentValue")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            ready: catalog.get("status").and_then(Value::as_str) == Some("ready"),
            commands: candidates("commands"),
            feedbacks: candidates("feedbacks"),
            physical_drivers: catalog
                .get("physicalDrivers")
                .and_then(Value::as_array)
                .map(|drivers| {
                    drivers
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            error: catalog
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }
}

/// The catalog from the project's compiled bindings (the `trust-lsp.hmiBindings` answer), the live
/// process image (`inputs`, `outputs`, `memory` entries with `address` and `value`) and the
/// communication schema and topology the canvas shows: compiled commands (outputs, then memory) and
/// feedbacks in the order compiled, each with its current value when the live entry at the same
/// canonical address holds one, and the enabled physical I/O drivers. A failed or missing
/// compilation offers no signal and says why.
pub(in crate::control::plant_handlers) fn build(
    compiled: Option<&Value>,
    live: Option<&Value>,
    schema: Option<&Value>,
    topology: Option<&Value>,
) -> Catalog {
    let Some(compiled) = compiled.and_then(Value::as_object) else {
        return Catalog::unavailable("Compiled signal metadata is unavailable.");
    };
    if compiled.get("ok").and_then(Value::as_bool) != Some(true) {
        let error = compiled
            .get("error")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|error| !error.is_empty())
            .unwrap_or("The current project did not compile.");
        return Catalog::unavailable(error);
    }
    let image = compiled.get("processImage").and_then(Value::as_object);
    let entries = |key: &str| image.and_then(|image| image.get(key));
    let live_entries = |key: &str| live.and_then(|live| live.get(key));
    let mut commands = candidates(entries("commands"), live_entries("outputs"));
    commands.extend(candidates(entries("memory"), live_entries("memory")));
    Catalog {
        ready: true,
        commands,
        feedbacks: candidates(entries("feedbacks"), live_entries("inputs")),
        physical_drivers: physical_drivers(schema, topology),
        error: None,
    }
}

fn candidates(compiled: Option<&Value>, live: Option<&Value>) -> Vec<Candidate> {
    let Some(compiled) = compiled.and_then(Value::as_array) else {
        return Vec::new();
    };
    // The last live entry at an address is the current one.
    let mut current: Vec<(String, String)> = Vec::new();
    for entry in live.and_then(Value::as_array).into_iter().flatten() {
        let Some(entry) = entry.as_object() else {
            continue;
        };
        let address = canonical(entry.get("address").and_then(Value::as_str).unwrap_or(""));
        let value = entry
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        current.retain(|(known, _)| *known != address);
        current.push((address, value));
    }
    compiled
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|candidate| {
            let symbol = text(candidate, "symbol");
            let address = text(candidate, "address");
            let mut data_type = text(candidate, "dataType");
            if data_type.is_empty() {
                data_type = text(candidate, "type");
            }
            let data_type = data_type.to_ascii_uppercase();
            if symbol.is_empty() || address.is_empty() || data_type.is_empty() {
                return None;
            }
            let current_value = current
                .iter()
                .find(|(known, _)| *known == canonical(&address))
                .map(|(_, value)| value.clone())
                .filter(|value| is_current(value));
            Some(Candidate {
                symbol,
                address,
                data_type,
                current_value,
            })
        })
        .collect()
}

/// Whether a live value is a current reading: not empty, not `unresolved` and not an error.
fn is_current(value: &str) -> bool {
    let normalized = value.trim().to_lowercase();
    !normalized.is_empty() && normalized != "unresolved" && !normalized.starts_with("error:")
}

fn text(object: &Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .map_or_else(String::new, |value| value.trim().to_string())
}

/// The enabled physical I/O drivers, each named once in the order found: the communication schema's
/// field-device protocols with an enabled instance, then the topology's field endpoints that are not
/// disabled. A driver that touches no real I/O (`simulated`, `sim`, `noop`, `loopback`, `none`;
/// specification 46 §6.1 item 5) is not physical.
pub(super) fn physical_drivers(schema: Option<&Value>, topology: Option<&Value>) -> Vec<String> {
    let mut drivers: Vec<String> = Vec::new();
    let mut add = |name: String| {
        if !drivers.contains(&name) {
            drivers.push(name);
        }
    };
    let protocols = schema
        .and_then(|schema| schema.get("protocols"))
        .and_then(Value::as_array);
    for protocol in protocols.into_iter().flatten().filter_map(Value::as_object) {
        let id = text(protocol, "id").to_lowercase();
        if protocol.get("category").and_then(Value::as_str) != Some("field_device")
            || crate::io::is_nonphysical_driver(&id)
        {
            continue;
        }
        let enabled = protocol
            .get("instances")
            .and_then(Value::as_array)
            .is_some_and(|instances| {
                instances.iter().any(|instance| {
                    instance.as_object().is_some_and(|instance| {
                        instance.get("enabled") != Some(&Value::Bool(false))
                    })
                })
            });
        if !enabled {
            continue;
        }
        let title = text(protocol, "title");
        add(first_named(&[title, id]));
    }
    let hosts = topology
        .and_then(|topology| topology.get("hosts"))
        .and_then(Value::as_array);
    for host in hosts.into_iter().flatten().filter_map(Value::as_object) {
        let containers = host
            .get("containers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|container| container.get("runtimes").and_then(Value::as_array))
            .flatten();
        let runtimes = host
            .get("runtimes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .chain(containers);
        for runtime in runtimes {
            let endpoints = runtime.get("endpoints").and_then(Value::as_array);
            for endpoint in endpoints.into_iter().flatten().filter_map(Value::as_object) {
                let protocol = text(endpoint, "protocol").to_lowercase();
                let physical = endpoint.get("category").and_then(Value::as_str)
                    == Some("field_device")
                    || endpoint.get("kind").and_then(Value::as_str) == Some("field");
                if !physical
                    || crate::io::is_nonphysical_driver(&protocol)
                    || text(endpoint, "health").to_lowercase() == "disabled"
                {
                    continue;
                }
                add(first_named(&[
                    text(endpoint, "display_name"),
                    text(endpoint, "name"),
                    protocol,
                ]));
            }
        }
    }
    drivers
}

/// The first non-empty name, else `physical I/O`.
fn first_named(names: &[String]) -> String {
    names
        .iter()
        .find(|name| !name.is_empty())
        .cloned()
        .unwrap_or_else(|| "physical I/O".to_string())
}
