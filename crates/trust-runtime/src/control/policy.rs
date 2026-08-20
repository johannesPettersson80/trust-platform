use crate::security::AccessRole;

use super::operation_registry::{operation_policy, RolePolicy};

pub(super) fn is_debug_request(kind: &str) -> bool {
    operation_policy(kind).is_some_and(|policy| policy.debug_surface)
}

pub(crate) fn required_role_for_control_request(
    kind: &str,
    params: Option<&serde_json::Value>,
) -> AccessRole {
    if kind == "ads.import_symbols.apply" {
        return AccessRole::Engineer;
    }
    match operation_policy(kind).map(|policy| policy.role) {
        Some(RolePolicy::Fixed(role)) => role,
        Some(RolePolicy::AdsDoctor) => required_role_for_ads_doctor(params),
        Some(RolePolicy::AdsImportSymbols) => required_role_for_ads_import_symbols(params),
        Some(RolePolicy::CommBrowseSymbols) => required_role_for_comm_browse_symbols(params),
        Some(RolePolicy::ConfigSet) => required_role_for_config_set(params),
        None => AccessRole::Admin,
    }
}

fn required_role_for_comm_browse_symbols(params: Option<&serde_json::Value>) -> AccessRole {
    let Some(params) = params.and_then(serde_json::Value::as_object) else {
        return AccessRole::Viewer;
    };
    let has_live_target = params.get("target").is_some_and(|value| !value.is_null());
    if has_live_target {
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
    if has_live_target {
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
            AccessRole::Engineer
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
