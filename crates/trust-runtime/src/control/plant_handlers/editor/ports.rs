//! The plant's ports and their PLC signals (specification 47 §8; specification 46 §13): the named
//! command and feedback ports a model has, the checks of each binding against the compiled signal
//! catalog, and a binding change that keeps every other port's index and conversion.

use serde_json::{json, Map, Value};

use super::issues::{Issue, Target};
use super::js;
use super::signals::Catalog;
use super::state_space;

/// The named ports of the draft's model: command ports, then feedback ports.
pub(super) fn labels(draft: &Map<String, Value>) -> (Vec<String>, Vec<String>) {
    let strings = |values: &[&str]| values.iter().map(|value| (*value).to_string()).collect();
    match js::text(draft, "model") {
        "gravity_tank_cascade" => (
            strings(&["Pump flow (m³/s)"]),
            strings(&["Upstream level (m)", "Downstream level (m)"]),
        ),
        "robot_joint" => (
            strings(&["Torque command (N·m)"]),
            strings(&["Joint angle (rad)", "Angular speed (rad/s)"]),
        ),
        "state_space" => {
            let dimensions = state_space::dimensions(js::object(draft, "parameters"));
            (
                (1..=dimensions.inputs)
                    .map(|index| format!("Input {index}"))
                    .collect(),
                (1..=dimensions.outputs)
                    .map(|index| format!("Output {index}"))
                    .collect(),
            )
        }
        _ => (
            strings(&["Command / PLC output"]),
            strings(&["Feedback / PLC input"]),
        ),
    }
}

/// The canonical form of an address: trimmed and in capitals.
pub(super) fn canonical(address: &str) -> String {
    address.trim().to_ascii_uppercase()
}

/// Why `address` cannot serve as a `direction` signal (`"command"` or `"feedback"`), or the compiled
/// address it names (specification 47 §8): the project compiled; the address is a compiled numeric
/// signal of that direction (a `%Q` output or `%M` memory value for a command, a `%I` input for
/// feedback); a feedback address is not already written by another plant (`used`); and no enabled
/// physical I/O driver owns the process image.
pub(super) fn check_address(
    catalog: &Catalog,
    direction: &str,
    address: &str,
    used: &[String],
) -> Result<String, (&'static str, String)> {
    let command = direction == "command";
    if !catalog.ready {
        return Err((
            "catalog_unavailable",
            "Compile the project successfully before entering a PLC signal.".into(),
        ));
    }
    let (candidates, opposite) = if command {
        (&catalog.commands, &catalog.feedbacks)
    } else {
        (&catalog.feedbacks, &catalog.commands)
    };
    let requested = canonical(address);
    let Some(candidate) = candidates
        .iter()
        .find(|candidate| canonical(&candidate.address) == requested)
    else {
        if opposite
            .iter()
            .any(|candidate| canonical(&candidate.address) == requested)
        {
            return Err((
                "wrong_direction",
                if command {
                    "Use a compiled numeric %Q output or %M memory value for a command."
                } else {
                    "Use a compiled numeric %I input for feedback."
                }
                .into(),
            ));
        }
        return Err((
            "not_compiled",
            if command {
                "Enter or choose a compiled numeric %Q output or %M memory value."
            } else {
                "Enter or choose a compiled numeric %I input."
            }
            .into(),
        ));
    };
    if !command
        && used
            .iter()
            .any(|used| canonical(used) == canonical(&candidate.address))
    {
        return Err((
            "duplicate_feedback",
            format!(
                "Choose a different %I input; {} is already used by another plant.",
                candidate.address
            ),
        ));
    }
    if !catalog.physical_drivers.is_empty() {
        let drivers = catalog.physical_drivers.join(", ");
        return Err((
            "physical_io_enabled",
            format!(
                "{} is owned by enabled physical I/O ({drivers}), so simulated {} cannot {} it. Disable {drivers} in Devices & Connections or choose another compiled %{}.",
                candidate.address,
                if command { "command" } else { "feedback" },
                if command { "read" } else { "write" },
                if command { "Q output" } else { "I input" },
            ),
        ));
    }
    Ok(candidate.address.clone())
}

/// The port checks (specification 47 §8): every port of the model is bound, each binding is a
/// usable compiled signal, a feedback target is used once across the plants, and a port the model no
/// longer has is reviewed before saving. `other_feedback` holds the other plants' feedback targets.
pub(super) fn checks(
    draft: &Map<String, Value>,
    catalog: &Catalog,
    other_feedback: &[String],
) -> Vec<Issue> {
    let (commands, feedbacks) = labels(draft);
    let sources = js::strings(draft, "commandSources");
    let targets = js::strings(draft, "feedbackTargets");
    let mut issues = Vec::new();
    for (direction, addresses, names, key) in [
        ("command", &sources, &commands, "command_sources"),
        ("feedback", &targets, &feedbacks, "feedback_targets"),
    ] {
        for index in 0..addresses.len().max(names.len()) {
            let mut add = |message: String| {
                issues.push(Issue::new(
                    "invalid_port_binding",
                    format!("{key}[{index}]"),
                    message,
                    Target::Port(direction, index),
                ));
            };
            let Some(name) = names.get(index) else {
                add(format!(
                    "This model has {} {direction} port(s). Review the extra binding before saving.",
                    names.len()
                ));
                continue;
            };
            let address = addresses.get(index).map_or("", String::as_str);
            if address.is_empty() {
                add(format!(
                    "Choose a PLC {} for {name}.",
                    if direction == "command" {
                        "output"
                    } else {
                        "input"
                    }
                ));
                continue;
            }
            let mut used = other_feedback.to_vec();
            if direction == "feedback" {
                used.extend(
                    targets
                        .iter()
                        .enumerate()
                        .filter(|(other, _)| *other != index)
                        .map(|(_, target)| target.clone()),
                );
            }
            if let Err((_, message)) = check_address(catalog, direction, address, &used) {
                add(format!("{name}: {message}"));
            }
        }
    }
    issues
}

/// The draft with port `index` of `direction` bound to `address` (`""` unbinds it): ports are
/// identities, so the other ports keep their index and binding, a port the model no longer has is
/// kept for review, and the conversions grow with the ports, a new command port starting at input 0,
/// scale 1, offset 0 and a new feedback port at scale 1, offset 0. An index the model does not have
/// changes nothing.
pub(super) fn bind(
    draft: &Map<String, Value>,
    direction: &str,
    index: usize,
    address: &str,
) -> Map<String, Value> {
    let (commands, feedbacks) = labels(draft);
    let command = direction == "command";
    let count = if command {
        commands.len()
    } else {
        feedbacks.len()
    };
    if index >= count {
        return draft.clone();
    }
    let key = if command {
        "commandSources"
    } else {
        "feedbackTargets"
    };
    let current = js::strings(draft, key);
    let length = count.max(current.len());
    let addresses: Vec<Value> = (0..length)
        .map(|port| {
            if port == index {
                json!(address)
            } else {
                json!(current.get(port).map_or("", String::as_str))
            }
        })
        .collect();
    let mut next = draft.clone();
    next.insert(key.into(), Value::Array(addresses));
    next.insert("dirty".into(), json!(true));
    let extend = |next: &mut Map<String, Value>, key: &str, fallback: i64| {
        let Some(values) = draft.get(key).and_then(Value::as_array) else {
            return;
        };
        let grown = (0..count.max(values.len()))
            .map(|port| values.get(port).cloned().unwrap_or(json!(fallback)))
            .collect();
        next.insert(key.into(), Value::Array(grown));
    };
    if command {
        extend(&mut next, "initialInput", 0);
        extend(&mut next, "inputScale", 1);
        extend(&mut next, "inputOffset", 0);
    } else {
        extend(&mut next, "outputScale", 1);
        extend(&mut next, "outputOffset", 0);
    }
    next
}
