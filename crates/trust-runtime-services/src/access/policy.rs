//! The minimum role of a control request, read from the operation registry and, for the
//! parameter-sensitive operations, from the request parameters.

use super::registry::{operation_policy, RolePolicy};
use super::role::AccessRole;

/// Whether `kind` is a debug-surface operation, which is refused while debugging is disabled.
#[must_use]
pub fn is_debug_request(kind: &str) -> bool {
    operation_policy(kind).is_some_and(|policy| policy.debug_surface)
}

/// The minimum role a request of operation `kind` with `params` requires. An operation missing
/// from the registry requires `Admin`.
#[must_use]
pub fn required_role_for_control_request(
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
        Some(RolePolicy::CommSearchStart) => required_role_for_comm_search_start(params),
        Some(RolePolicy::ConfigSet) => required_role_for_config_set(params),
        None => AccessRole::Admin,
    }
}

/// A search that confirms a fieldbus scan needs `Engineer`; every other search only reads and
/// needs `Viewer` (specification 11, "One search").
fn required_role_for_comm_search_start(params: Option<&serde_json::Value>) -> AccessRole {
    let confirms_fieldbus_scan = params
        .and_then(serde_json::Value::as_object)
        .and_then(|object| object.get("confirm_fieldbus_scan"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if confirms_fieldbus_scan {
        AccessRole::Engineer
    } else {
        AccessRole::Viewer
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
    fn search_requests_have_their_roles() {
        for params in [
            None,
            Some(json!({})),
            Some(json!({ "only": ["modbus_tcp"], "deadline_ms": 5000 })),
            Some(json!({ "confirm_fieldbus_scan": false })),
            Some(json!({ "confirm_fieldbus_scan": "true" })),
            Some(json!({ "fieldbus": [{ "bus": "ethercat", "adapter": "eth1" }] })),
        ] {
            assert_eq!(
                required_role_for_control_request("comm.search.start", params.as_ref()),
                AccessRole::Viewer,
                "params={params:?}"
            );
        }
        assert_eq!(
            required_role_for_control_request(
                "comm.search.start",
                Some(&json!({
                    "confirm_fieldbus_scan": true,
                    "fieldbus": [{ "bus": "ethercat", "adapter": "eth1" }]
                }))
            ),
            AccessRole::Engineer
        );
        for kind in ["comm.search.status", "comm.search.cancel"] {
            assert_eq!(
                required_role_for_control_request(kind, Some(&json!({ "search_id": "s-1" }))),
                AccessRole::Viewer,
                "kind={kind}"
            );
            assert!(!is_debug_request(kind), "kind={kind}");
        }
        assert!(!is_debug_request("comm.search.start"));
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
