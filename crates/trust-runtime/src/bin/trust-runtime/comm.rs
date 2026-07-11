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
            target_net_id,
            ams_port,
            adapter,
            timeout_ms,
            unit_id,
            probe_read_address,
            probe_read_quantity,
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
            if let Some(target_net_id) = target_net_id {
                scope.insert("target_ams_net_id".to_string(), json!(target_net_id));
            }
            if let Some(ams_port) = ams_port {
                scope.insert("ams_port".to_string(), json!(ams_port));
            }
            if let Some(adapter) = adapter {
                scope.insert("adapter".to_string(), json!(adapter));
            }
            if let Some(timeout_ms) = timeout_ms {
                scope.insert("timeout_ms".to_string(), json!(timeout_ms));
            }
            if let Some(unit_id) = unit_id {
                scope.insert("unit_id".to_string(), json!(unit_id));
            }
            if let Some(probe_read_address) = probe_read_address {
                scope.insert("probe_read_address".to_string(), json!(probe_read_address));
            }
            if let Some(probe_read_quantity) = probe_read_quantity {
                scope.insert(
                    "probe_read_quantity".to_string(),
                    json!(probe_read_quantity),
                );
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
        CommAction::OpcUaTrust { action, json: _ } => match action {
            crate::cli::CommOpcUaTrustAction::List => {
                let certs = trust_runtime::opcua::list_trusted_opcua_client_server_certificates()
                    .map_err(anyhow::Error::from)?
                    .into_iter()
                    .map(|cert| {
                        json!({
                            "file_name": cert.file_name,
                            "path": cert.path.display().to_string(),
                        })
                    })
                    .collect::<Vec<_>>();
                print_json(json!({
                    "schema_version": 1,
                    "protocol": "opcua_client",
                    "pki_dir": trust_runtime::opcua::opcua_client_pki_dir().display().to_string(),
                    "trusted": certs,
                }))
            }
            crate::cli::CommOpcUaTrustAction::Clear => {
                let cleared =
                    trust_runtime::opcua::clear_trusted_opcua_client_server_certificates()
                        .map_err(anyhow::Error::from)?;
                print_json(json!({
                    "schema_version": 1,
                    "protocol": "opcua_client",
                    "pki_dir": trust_runtime::opcua::opcua_client_pki_dir().display().to_string(),
                    "cleared": cleared,
                }))
            }
        },
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
