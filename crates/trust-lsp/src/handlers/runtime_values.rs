use rustc_hash::FxHashMap;
use serde::Deserialize;
use serde_json::Value;
use smol_str::SmolStr;
use std::time::Duration;
use tracing::{debug, warn};
use trust_runtime_client::{ControlClient, ControlEndpoint};

const CONTROL_IO_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeInlineValues {
    pub(crate) locals: FxHashMap<SmolStr, String>,
    pub(crate) globals: FxHashMap<SmolStr, String>,
    pub(crate) retain: FxHashMap<SmolStr, String>,
}

/// One inline-values request; a failure is logged and ends the snapshot.
fn request(client: &mut ControlClient, kind: &str, params: Option<Value>) -> Option<Value> {
    debug!("inlineValue control request kind={}", kind);
    client
        .request(kind, params)
        .map_err(|error| warn!("inlineValue control request kind={kind} failed: {error}"))
        .ok()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugScope {
    name: String,
    variables_reference: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugVariable {
    name: String,
    value: String,
}

pub(crate) fn fetch_runtime_inline_values(
    endpoint: &str,
    auth: Option<&str>,
    frame_id: u32,
    owner_hints: &[SmolStr],
) -> Option<RuntimeInlineValues> {
    let endpoint = match ControlEndpoint::parse(endpoint) {
        Some(endpoint) => endpoint,
        None => {
            warn!("inlineValue control endpoint parse failed: {}", endpoint);
            return None;
        }
    };
    let mut client = match ControlClient::connect(&endpoint, auth, CONTROL_IO_TIMEOUT) {
        Ok(client) => client,
        Err(error) => {
            warn!("inlineValue control connect failed: {error}");
            return None;
        }
    };
    debug!(
        "inlineValue control connected frame_id={} owner_hints={}",
        frame_id,
        owner_hints.len()
    );
    let scopes_value = request(
        &mut client,
        "debug.scopes",
        Some(serde_json::json!({ "frame_id": frame_id })),
    )?;
    let scopes = scopes_value
        .get("scopes")
        .and_then(|value| serde_json::from_value::<Vec<DebugScope>>(value.clone()).ok())?;
    debug!(
        "inlineValue control scopes={:?}",
        scopes
            .iter()
            .map(|scope| scope.name.as_str())
            .collect::<Vec<_>>()
    );

    let mut locals_ref = None;
    let mut globals_ref = None;
    let mut retain_ref = None;
    let mut instances_ref = None;
    for scope in scopes {
        match scope.name.trim().to_ascii_lowercase().as_str() {
            "locals" if locals_ref.replace(scope.variables_reference).is_some() => {
                return None;
            }
            "globals" if globals_ref.replace(scope.variables_reference).is_some() => {
                return None;
            }
            "retain" if retain_ref.replace(scope.variables_reference).is_some() => {
                return None;
            }
            "instances" if instances_ref.replace(scope.variables_reference).is_some() => {
                return None;
            }
            _ => {}
        }
    }

    let mut locals = match locals_ref {
        Some(reference) => fetch_variables(&mut client, reference)?,
        None => FxHashMap::default(),
    };
    let globals = match globals_ref {
        Some(reference) => fetch_variables(&mut client, reference)?,
        None => FxHashMap::default(),
    };
    let retain = match retain_ref {
        Some(reference) => fetch_variables(&mut client, reference)?,
        None => FxHashMap::default(),
    };
    debug!(
        "inlineValue control values locals={} globals={} retain={}",
        locals.len(),
        globals.len(),
        retain.len()
    );

    if let Some(reference) = instances_ref {
        let instance_vars = fetch_instance_variables(&mut client, reference, owner_hints)?;
        for (name, value) in instance_vars {
            locals.entry(name).or_insert(value);
        }
    }
    debug!(
        "inlineValue merged locals count={} (after instances)",
        locals.len()
    );

    Some(RuntimeInlineValues {
        locals,
        globals,
        retain,
    })
}

fn fetch_instance_variables(
    client: &mut ControlClient,
    reference: u32,
    owner_hints: &[SmolStr],
) -> Option<FxHashMap<SmolStr, String>> {
    let instances_value = request(
        client,
        "debug.variables",
        Some(serde_json::json!({ "variables_reference": reference })),
    )?;
    let instances = instances_value
        .get("variables")
        .and_then(|value| serde_json::from_value::<Vec<DebugVariableRef>>(value.clone()).ok())
        .unwrap_or_default();

    if instances.is_empty() {
        debug!("inlineValue instances scope empty");
        return Some(FxHashMap::default());
    }
    debug!(
        "inlineValue instances candidates={:?}",
        instances
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>()
    );

    let mut chosen = None;
    if !owner_hints.is_empty() {
        for hint in owner_hints {
            let needle = format!("{}#", hint);
            if let Some(instance) = instances.iter().find(|entry| {
                entry
                    .name
                    .to_ascii_lowercase()
                    .starts_with(&needle.to_ascii_lowercase())
            }) {
                chosen = Some(instance.variables_reference);
                debug!("inlineValue instance match exact prefix hint={}", hint);
                break;
            }
        }
    }

    if chosen.is_none() && !owner_hints.is_empty() {
        for hint in owner_hints {
            if let Some(instance) = instances.iter().find(|entry| {
                entry
                    .name
                    .split('#')
                    .next()
                    .map(|type_name| type_name.eq_ignore_ascii_case(hint.as_str()))
                    .unwrap_or(false)
            }) {
                chosen = Some(instance.variables_reference);
                debug!("inlineValue instance match type name hint={}", hint);
                break;
            }
        }
    }

    if chosen.is_none() && !owner_hints.is_empty() {
        let mut candidates = Vec::new();
        for hint in owner_hints {
            let hint_base = hint.rsplit('.').next().unwrap_or(hint.as_str());
            for instance in &instances {
                let type_name = instance.name.split('#').next().unwrap_or("");
                let instance_base = type_name.rsplit('.').next().unwrap_or(type_name);
                if instance_base.eq_ignore_ascii_case(hint_base)
                    && !candidates.contains(&instance.variables_reference)
                {
                    candidates.push(instance.variables_reference);
                }
            }
        }
        if candidates.len() == 1 {
            chosen = Some(candidates[0]);
            debug!(
                "inlineValue instance match base-name candidate_ref={}",
                candidates[0]
            );
        }
    }

    if chosen.is_none() && instances.len() == 1 {
        chosen = Some(instances[0].variables_reference);
        debug!("inlineValue instance match single candidate");
    }

    let Some(reference) = chosen else {
        return Some(FxHashMap::default());
    };
    fetch_variables(client, reference)
}

fn fetch_variables(
    client: &mut ControlClient,
    reference: u32,
) -> Option<FxHashMap<SmolStr, String>> {
    let vars_value = request(
        client,
        "debug.variables",
        Some(serde_json::json!({ "variables_reference": reference })),
    )?;
    let variables = vars_value
        .get("variables")
        .and_then(|value| serde_json::from_value::<Vec<DebugVariable>>(value.clone()).ok())?;
    let mut out = FxHashMap::default();
    for variable in variables {
        let name = variable.name.trim();
        if name.is_empty() {
            continue;
        }
        out.entry(SmolStr::new(name)).or_insert(variable.value);
    }
    Some(out)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugVariableRef {
    name: String,
    variables_reference: u32,
}

#[cfg(test)]
#[path = "runtime_values/contract_tests.rs"]
mod contract_tests;
