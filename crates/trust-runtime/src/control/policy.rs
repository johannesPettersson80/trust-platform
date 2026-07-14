use crate::security::AccessRole;

pub(super) fn is_debug_request(kind: &str) -> bool {
    matches!(
        kind,
        "pause"
            | "resume"
            | "step_in"
            | "step_over"
            | "step_out"
            | "breakpoints.set"
            | "breakpoints.clear"
            | "breakpoints.clear_all"
            | "breakpoints.clear_id"
            | "breakpoints.list"
            | "eval"
            | "set"
            | "var.force"
            | "var.unforce"
            | "var.forced"
            | "debug.state"
            | "debug.stops"
            | "debug.stack"
            | "debug.scopes"
            | "debug.variables"
            | "debug.evaluate"
            | "debug.breakpoint_locations"
    )
}

pub(crate) fn required_role_for_control_request(
    kind: &str,
    params: Option<&serde_json::Value>,
) -> AccessRole {
    match kind {
        "status"
        | "health"
        | "tasks.stats"
        | "events.tail"
        | "events"
        | "faults"
        | "config.get"
        | "connectors.status"
        | "ads.discover"
        | "ads.identity"
        | "ads.doctor.status"
        | "ads.route_plan"
        | "ads.status"
        | "ads.server.status"
        | "ads.server.symbols"
        | "ads.server.doctor.status"
        | "ads.server.route_plan"
        | "comm.capabilities"
        | "comm.schema"
        | "comm.discover"
        | "fleet.topology"
        | "io.list"
        | "io.read"
        | "hmi.schema.get"
        | "hmi.values.get"
        | "hmi.trends.get"
        | "hmi.alarms.get"
        | "hmi.descriptor.get"
        | "historian.query"
        | "historian.alerts"
        | "debug.state"
        | "debug.stops"
        | "debug.stack"
        | "debug.scopes"
        | "debug.variables"
        | "debug.breakpoint_locations"
        | "breakpoints.list"
        | "var.forced" => AccessRole::Viewer,
        "ads.doctor" | "ads.doctor.start" => required_role_for_ads_doctor(params),
        "comm.browse_symbols" => required_role_for_comm_browse_symbols(params),
        "ads.import_symbols" | "ads.import_symbols.apply" => {
            required_role_for_ads_import_symbols(params)
        }
        "ads.server.doctor" | "ads.server.doctor.start" => AccessRole::Engineer,
        "pause" | "resume" | "restart" | "hmi.alarm.ack" | "pair.claim" => AccessRole::Operator,
        "step_in"
        | "step_over"
        | "step_out"
        | "breakpoints.set"
        | "breakpoints.clear"
        | "breakpoints.clear_all"
        | "breakpoints.clear_id"
        | "eval"
        | "set"
        | "var.force"
        | "var.unforce"
        | "io.write"
        | "io.force"
        | "io.unforce"
        | "debug.evaluate"
        | "hmi.write"
        | "hmi.descriptor.update"
        | "hmi.scaffold.reset"
        | "comm.apply"
        | "comm.test" => AccessRole::Engineer,
        "config.set" => required_role_for_config_set(params),
        "shutdown" | "bytecode.reload" | "ads.route_add" | "ads.route_remove" | "pair.start"
        | "pair.list" | "pair.revoke" => AccessRole::Admin,
        _ => AccessRole::Admin,
    }
}

fn required_role_for_comm_browse_symbols(params: Option<&serde_json::Value>) -> AccessRole {
    let Some(params) = params.and_then(serde_json::Value::as_object) else {
        return AccessRole::Viewer;
    };
    let has_live_target = params.get("target").is_some_and(|value| !value.is_null());
    let has_snapshot = params.get("snapshot").is_some_and(|value| !value.is_null());
    if has_live_target && !has_snapshot {
        AccessRole::Engineer
    } else {
        AccessRole::Viewer
    }
}

fn required_role_for_ads_import_symbols(params: Option<&serde_json::Value>) -> AccessRole {
    let Some(params) = params.and_then(serde_json::Value::as_object) else {
        return AccessRole::Viewer;
    };
    let has_live_target = params.get("target").is_some_and(|value| !value.is_null());
    let has_snapshot = params.get("snapshot").is_some_and(|value| !value.is_null());
    if has_live_target && !has_snapshot {
        AccessRole::Engineer
    } else {
        AccessRole::Viewer
    }
}

fn required_role_for_ads_doctor(params: Option<&serde_json::Value>) -> AccessRole {
    let writes_enabled = params
        .and_then(serde_json::Value::as_object)
        .and_then(|object| object.get("writes_enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if writes_enabled {
        AccessRole::Engineer
    } else {
        AccessRole::Viewer
    }
}

fn required_role_for_config_set(params: Option<&serde_json::Value>) -> AccessRole {
    let Some(params) = params.and_then(serde_json::Value::as_object) else {
        return AccessRole::Engineer;
    };
    let requires_admin = params.keys().any(|key| {
        matches!(
            key.as_str(),
            "control.auth_token"
                | "control.debug_enabled"
                | "mesh.auth_token"
                | "mesh.role"
                | "mesh.zenohd_version"
                | "mesh.plugin_versions"
                | "control.mode"
                | "web.auth"
                | "runtime_cloud.profile"
                | "runtime_cloud.wan.allow_write"
                | "runtime_cloud.links.transports"
        )
    });
    if requires_admin {
        AccessRole::Admin
    } else {
        AccessRole::Engineer
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn ads_import_symbols_requires_engineer_for_live_target_only() {
        let cached = json!({
            "connection_name": "line1",
            "snapshot": {
                "route_name": "line1",
                "symbols": []
            }
        });
        assert_eq!(
            required_role_for_control_request("ads.import_symbols", Some(&cached)),
            AccessRole::Viewer
        );

        let live = json!({
            "connection_name": "line1",
            "target": {
                "ip": "192.168.10.5",
                "ams_net_id": "5.23.91.12.1.1",
                "ams_port": 851
            }
        });
        assert_eq!(
            required_role_for_control_request("ads.import_symbols", Some(&live)),
            AccessRole::Engineer
        );
    }

    #[test]
    fn comm_browse_symbols_requires_engineer_for_live_target_only() {
        let cached = json!({
            "protocol": "ads",
            "snapshot": {
                "schema_version": 1,
                "route_name": "line1",
                "symbols": []
            }
        });
        assert_eq!(
            required_role_for_control_request("comm.browse_symbols", Some(&cached)),
            AccessRole::Viewer
        );

        let live = json!({
            "protocol": "ads",
            "target": {
                "host": "192.168.10.5",
                "ams_net_id": "5.23.91.12.1.1",
                "ams_port": 851
            }
        });
        assert_eq!(
            required_role_for_control_request("comm.browse_symbols", Some(&live)),
            AccessRole::Engineer
        );
    }

    #[test]
    fn connectors_status_requires_viewer_role() {
        assert_eq!(
            required_role_for_control_request("connectors.status", None),
            AccessRole::Viewer
        );
    }

    #[test]
    fn ads_connector_status_and_ads_mutations_keep_role_boundaries() {
        assert_eq!(
            required_role_for_control_request("connectors.status", None),
            AccessRole::Viewer
        );
        assert_eq!(
            required_role_for_control_request("ads.route_add", None),
            AccessRole::Admin
        );
        assert_eq!(
            required_role_for_control_request("ads.route_remove", None),
            AccessRole::Admin
        );
        assert_eq!(
            required_role_for_control_request(
                "ads.doctor",
                Some(&json!({ "writes_enabled": true }))
            ),
            AccessRole::Engineer
        );
        assert_eq!(
            required_role_for_control_request(
                "ads.import_symbols.apply",
                Some(&json!({
                    "connection_name": "line1",
                    "target": {
                        "ip": "192.168.10.5",
                        "ams_net_id": "5.23.91.12.1.1",
                        "ams_port": 851
                    }
                }))
            ),
            AccessRole::Engineer
        );
        assert_eq!(
            required_role_for_control_request(
                "ads.import_symbols.apply",
                Some(&json!({
                    "connection_name": "line1",
                    "snapshot": {
                        "route_name": "line1",
                        "symbols": []
                    }
                }))
            ),
            AccessRole::Viewer
        );
    }

    #[test]
    fn security_sensitive_debug_activation_and_unclassified_requests_fail_safe() {
        assert_eq!(
            required_role_for_control_request(
                "config.set",
                Some(&json!({ "control.debug_enabled": true }))
            ),
            AccessRole::Admin
        );
        assert_eq!(
            required_role_for_control_request("future.unclassified.operation", None),
            AccessRole::Admin
        );
    }
}
