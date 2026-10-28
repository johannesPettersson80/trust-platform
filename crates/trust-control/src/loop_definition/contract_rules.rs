//! The loop-file rules that the page's loop validator (`loopDefinition.ts`) applies when a loop opens, applied by the engine to the raw
//! file before it decodes (specification 35, B1.2; parity at open). Each rule names the page's condition it mirrors. Two rules the page
//! has are not here, because the specification contradicts them: a missing output limit or experiment duration still loads (B3.4 step
//! 1, B1.3), and `operatingPoint` is read and ignored (B1.4).

use serde_json::{Map, Value};

/// The most scenarios a verification campaign declares (`scenarioContract.ts`, `validEngineerScenarios`).
const MAXIMUM_SCENARIOS: usize = 24;
/// The most timed writes a scenario declares.
const MAXIMUM_EVENTS: usize = 12_000;
/// The most requirements a scenario checks.
const MAXIMUM_REQUIREMENTS: usize = 64;
/// The identity that no scenario may take.
const TARGET_RESPONSE: &str = "target-response";

/// The fields a loop file may hold as null, because the page does not read them: `cascade` is parked and kept as declared,
/// `operatingPoint` is read and ignored (B1.4), and `expectedSettlingSeconds` and `relayDirection` are not decoded by the page. A
/// null in any other field, at any depth, is refused, as the page's `=== undefined` checks refuse it.
const UNREAD_FIELDS: [&str; 4] = [
    "cascade",
    "expectedSettlingSeconds",
    "operatingPoint",
    "relayDirection",
];

/// The keys a proportional law may hold (`controllerLaw.ts`, `decodeProportionalLaw`).
const PROPORTIONAL_KEYS: [&str; 2] = ["kind", "gainSymbol"];

/// The keys a Control.PID initialization may hold (`loopDefinition.ts`, `decodeLibraryPidInitialization`).
const CONTROL_PID_KEYS: [&str; 4] = [
    "kind",
    "maximumSnapshotAgeMs",
    "states",
    "applicationStates",
];

/// The keys a parallel PID's initialization holds, exactly (`loopDefinition.ts`, `pidInitialization`).
const PARALLEL_PID_KEYS: [&str; 6] = [
    "derivativeSetpointWeight",
    "derivativeState",
    "integralState",
    "kind",
    "maximumSnapshotAgeMs",
    "proportionalSetpointWeight",
];

/// Checks the rules the page applies to a loop file's raw fields when the loop opens.
///
/// # Errors
/// A field breaks one of the page's rules; the sentence names the field.
pub(super) fn check(value: &Value) -> Result<(), String> {
    let Some(fields) = value.as_object() else {
        // The decoder refuses a file that is not an object, with its own sentence.
        return Ok(());
    };
    check_nulls(value, "")?;
    check_initial_indices(value)?;
    check_plant_id(fields)?;
    check_analysis_units(fields)?;
    check_scenarios(fields)?;
    check_controller(fields)
}

/// No field of the loop file may be null, at any depth, except the fields the page does not read (`UNREAD_FIELDS`): the page's
/// decoders test each present field with `=== undefined`, so a null fails them, and an optional field the engine's serde types read as
/// absent must not be read that way. `path` names the value, for the sentence.
fn check_nulls(value: &Value, path: &str) -> Result<(), String> {
    match value {
        Value::Null => Err(format!(
            "The field {path} must not be null: remove it or give it a value."
        )),
        Value::Object(fields) => {
            for (key, member) in fields {
                if UNREAD_FIELDS.contains(&key.as_str()) {
                    continue;
                }
                let name = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                check_nulls(member, &name)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                check_nulls(item, &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The largest integer a JavaScript number holds exactly: the largest index or byte offset the page reads (`Number.isSafeInteger`).
const MAXIMUM_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// A declared state's initial source names its index as a safe integer, as the page reads it (`decodeControllerInitialization`).
fn check_initial_indices(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(fields) => {
            let index = fields
                .get("initial")
                .and_then(|initial| initial.get("index"))
                .and_then(Value::as_f64);
            if index.is_some_and(|index| index > MAXIMUM_SAFE_INTEGER) {
                return Err("A state's initial index is a safe integer.".into());
            }
            fields.values().try_for_each(check_initial_indices)
        }
        Value::Array(items) => items.iter().try_for_each(check_initial_indices),
        _ => Ok(()),
    }
}

/// The plant identifier is text that is not blank once trimmed (`optionalString`, `nonempty`).
fn check_plant_id(fields: &Map<String, Value>) -> Result<(), String> {
    match fields.get("plantId") {
        None => Ok(()),
        Some(plant) if plant.as_str().is_some_and(|text| !text.trim().is_empty()) => Ok(()),
        Some(_) => Err("The plant identifier must be text that is not blank.".into()),
    }
}

/// The analysis's plant units are the loop's signal units, when the analysis declares them as text (`loopDefinition.ts`,
/// `incompatible_model_units`). A missing unit is left to the analysis's own decode.
fn check_analysis_units(fields: &Map<String, Value>) -> Result<(), String> {
    let Some(plant) = fields
        .get("analysis")
        .and_then(|analysis| analysis.get("plant"))
    else {
        return Ok(());
    };
    // A signal whose unit is blank is dimensionless: the page states it as "1" before it compares units (`withDimensionlessUnits`).
    let signal_unit = |name: &str| {
        fields
            .get("signals")
            .and_then(|signals| signals.get(name))
            .and_then(|signal| signal.get("unit"))
            .and_then(Value::as_str)
            .map(|unit| if unit.trim().is_empty() { "1" } else { unit })
    };
    for (analysed_key, signal_name) in [
        ("inputUnit", "manipulatedVariable"),
        ("outputUnit", "processVariable"),
    ] {
        if let (Some(analysed), Some(actual)) = (
            plant.get(analysed_key).and_then(Value::as_str),
            signal_unit(signal_name),
        ) && analysed != actual
        {
            return Err(
                "The analysis's plant units must be the units of the loop's signals.".into(),
            );
        }
    }
    Ok(())
}

/// The campaign's scenarios have the structure the page's verification runs (`validEngineerScenarios`).
fn check_scenarios(fields: &Map<String, Value>) -> Result<(), String> {
    match fields.get("verificationScenarios") {
        None => Ok(()),
        Some(scenarios) if valid_scenarios(scenarios) => Ok(()),
        Some(_) => Err("The verification scenarios do not have the structure Verify runs.".into()),
    }
}

/// The controller's proportional law holds only its kind and its gain symbol, and its parallel PID initialization holds exactly its keys.
fn check_controller(fields: &Map<String, Value>) -> Result<(), String> {
    let Some(controller) = fields.get("controller").and_then(Value::as_object) else {
        return Ok(());
    };
    if let Some(law) = controller.get("law").and_then(Value::as_object) {
        let proportional = law.get("kind").and_then(Value::as_str) == Some("proportional");
        if proportional
            && law
                .keys()
                .any(|key| !PROPORTIONAL_KEYS.contains(&key.as_str()))
        {
            return Err("A proportional law holds only its kind and its gain symbol.".into());
        }
    }
    if let Some(initialization) = controller
        .get("pidInitialization")
        .and_then(Value::as_object)
    {
        let kind = initialization.get("kind").and_then(Value::as_str);
        if kind == Some("parallel-pid-v1") && !exact_keys(initialization, &PARALLEL_PID_KEYS) {
            return Err("A parallel PID initialization holds exactly its six keys.".into());
        }
        if kind == Some("control-pid-v1")
            && initialization
                .keys()
                .any(|key| !CONTROL_PID_KEYS.contains(&key.as_str()))
        {
            return Err(
                "A Control.PID initialization holds only its kind, its snapshot age, its states and its application states.".into(),
            );
        }
    }
    Ok(())
}

/// Whether the object holds exactly the keys named, no more and no fewer.
fn exact_keys(fields: &Map<String, Value>, expected: &[&str]) -> bool {
    fields.len() == expected.len() && expected.iter().all(|key| fields.contains_key(*key))
}

/// Whether a value is a number the page accepts (`finite`): every JSON number is finite.
fn is_number(value: &Value) -> bool {
    value.is_number()
}

/// Whether one timed write is well formed: a time that is not negative, a symbol, and a number.
fn valid_event(event: &Value) -> bool {
    let Some(fields) = event.as_object() else {
        return false;
    };
    fields
        .get("atMs")
        .and_then(Value::as_f64)
        .is_some_and(|at| at >= 0.0)
        && fields
            .get("symbol")
            .and_then(Value::as_str)
            .is_some_and(|symbol| !symbol.is_empty())
        && fields.get("value").is_some_and(is_number)
}

/// The identity of a well-formed scenario, or `None` when the scenario breaks the page's rules.
fn valid_scenario(scenario: &Value) -> Option<&str> {
    let fields = scenario.as_object()?;
    let id = fields
        .get("id")?
        .as_str()
        .filter(|id| !id.is_empty() && *id != TARGET_RESPONSE)?;
    let initial = fields.get("initialValues")?.as_object()?;
    if !initial
        .iter()
        .all(|(symbol, item)| !symbol.is_empty() && is_number(item))
    {
        return None;
    }
    let events = fields.get("events")?.as_array()?;
    if events.len() > MAXIMUM_EVENTS || !events.iter().all(valid_event) {
        return None;
    }
    let requirements = fields.get("requirements")?.as_array()?;
    if requirements.is_empty() || requirements.len() > MAXIMUM_REQUIREMENTS {
        return None;
    }
    Some(id)
}

/// Whether the scenarios are a list of at most 24 well-formed scenarios whose identities are distinct.
fn valid_scenarios(scenarios: &Value) -> bool {
    let Some(list) = scenarios.as_array() else {
        return false;
    };
    if list.len() > MAXIMUM_SCENARIOS {
        return false;
    }
    let mut identities = Vec::with_capacity(list.len());
    for scenario in list {
        match valid_scenario(scenario) {
            Some(id) => identities.push(id),
            None => return false,
        }
    }
    identities.sort_unstable();
    identities.windows(2).all(|pair| pair[0] != pair[1])
}

#[cfg(test)]
#[path = "contract_rules_tests.rs"]
mod tests;
