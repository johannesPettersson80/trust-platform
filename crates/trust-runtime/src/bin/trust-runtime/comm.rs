use anyhow::Context;
use serde_json::json;

use crate::cli::CommAction;

pub fn run_comm(action: CommAction) -> anyhow::Result<()> {
    match action {
        CommAction::Schema { protocol, json: _ } => {
            let params = protocol.map(|protocol| json!({ "protocol": protocol }));
            print_json(
                trust_runtime::control::offline_comm_schema_json(params.as_ref())
                    .map_err(anyhow::Error::msg)?,
            )
        }
        CommAction::Topology { project, json: _ } => print_json(
            trust_runtime::control::offline_fleet_topology_json(project.as_path())
                .map_err(anyhow::Error::msg)?,
        ),
        CommAction::Apply {
            project,
            protocol,
            params,
            action,
            instance_id,
            json: _,
        } => {
            let parsed_params = parse_params(params.as_str())?;
            let mut payload = json!({
                "protocol": protocol,
                "action": action.as_str(),
                "params": parsed_params,
            });
            if let Some(instance_id) = instance_id {
                payload["instance_id"] = json!(instance_id);
            }
            print_json(
                trust_runtime::control::offline_comm_apply_json(project.as_path(), payload)
                    .map_err(anyhow::Error::msg)?,
            )
        }
        CommAction::Discover {
            protocol,
            origin,
            cidr,
            host,
            adapter,
            timeout_ms,
            passive,
            json: _,
        } => {
            let mut scope = serde_json::Map::new();
            if let Some(cidr) = cidr {
                scope.insert("cidr".to_string(), json!(cidr));
            }
            if let Some(host) = host {
                scope.insert("host".to_string(), json!(host));
            }
            if let Some(adapter) = adapter {
                scope.insert("adapter".to_string(), json!(adapter));
            }
            if let Some(timeout_ms) = timeout_ms {
                scope.insert("timeout_ms".to_string(), json!(timeout_ms));
            }
            print_json(
                trust_runtime::control::offline_comm_discover_json(json!({
                    "protocol": protocol,
                    "origin": origin.as_str(),
                    "scope": serde_json::Value::Object(scope),
                    "passive": passive,
                }))
                .map_err(anyhow::Error::msg)?,
            )
        }
        CommAction::BrowseSymbols {
            protocol,
            project,
            target,
            snapshot_file,
            instance_id,
            kind,
            connection_name,
            json: _,
        } => {
            let mut payload = json!({
                "protocol": protocol,
                "kind": kind,
            });
            if let Some(target) = target {
                payload["target"] = parse_params(target.as_str())?;
            }
            if let Some(snapshot_file) = snapshot_file {
                let text = std::fs::read_to_string(&snapshot_file).with_context(|| {
                    format!("failed to read snapshot file {}", snapshot_file.display())
                })?;
                payload["snapshot"] = serde_json::from_str(&text).with_context(|| {
                    format!("failed to parse snapshot file {}", snapshot_file.display())
                })?;
            }
            if let Some(instance_id) = instance_id {
                payload["instance_id"] = json!(instance_id);
            }
            if let Some(connection_name) = connection_name {
                payload["connection_name"] = json!(connection_name);
            }
            let value = if let Some(project) = project {
                trust_runtime::control::offline_comm_browse_symbols_project_json(
                    project.as_path(),
                    payload,
                )
            } else {
                trust_runtime::control::offline_comm_browse_symbols_json(payload)
            }
            .map_err(anyhow::Error::msg)?;
            print_json(value)
        }
    }
}

fn parse_params(text: &str) -> anyhow::Result<serde_json::Value> {
    let value: serde_json::Value =
        serde_json::from_str(text).with_context(|| "failed to parse --params as a JSON object")?;
    if !value.is_object() {
        anyhow::bail!("--params must be a JSON object");
    }
    Ok(value)
}

fn print_json(value: serde_json::Value) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
