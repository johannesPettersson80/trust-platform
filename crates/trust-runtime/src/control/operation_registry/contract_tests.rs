use super::*;

use std::collections::{BTreeMap, BTreeSet};

use crate::control::policy::{is_debug_request, required_role_for_control_request};
use serde_json::json;

const VIEWER: &[&str] = &[
    "ads.discover",
    "ads.doctor.status",
    "ads.identity",
    "ads.route_plan",
    "ads.server.doctor.status",
    "ads.server.route_plan",
    "ads.server.status",
    "ads.server.symbols",
    "ads.status",
    "breakpoints.list",
    "comm.capabilities",
    "comm.discover",
    "comm.schema",
    "config.get",
    "connectors.status",
    "debug.breakpoint_locations",
    "debug.scopes",
    "debug.stack",
    "debug.state",
    "debug.stops",
    "debug.variables",
    "events",
    "events.tail",
    "faults",
    "fleet.topology",
    "health",
    "historian.alerts",
    "historian.query",
    "hmi.alarms.get",
    "hmi.descriptor.get",
    "hmi.schema.get",
    "hmi.trends.get",
    "hmi.values.get",
    "io.list",
    "io.read",
    "status",
    "tasks.stats",
    "var.forced",
];

const OPERATOR: &[&str] = &["hmi.alarm.ack", "pair.claim", "pause", "restart", "resume"];

const ENGINEER: &[&str] = &[
    "ads.server.doctor",
    "ads.server.doctor.start",
    "breakpoints.clear",
    "breakpoints.clear_all",
    "breakpoints.clear_id",
    "breakpoints.set",
    "comm.apply",
    "comm.test",
    "debug.evaluate",
    "eval",
    "hmi.descriptor.update",
    "hmi.scaffold.reset",
    "hmi.write",
    "io.force",
    "io.unforce",
    "io.write",
    "set",
    "step_in",
    "step_out",
    "step_over",
    "var.force",
    "var.unforce",
];

const ADMIN: &[&str] = &[
    "ads.route_add",
    "ads.route_remove",
    "bytecode.reload",
    "pair.list",
    "pair.revoke",
    "pair.start",
    "shutdown",
];

const PARAMETER_SENSITIVE: &[(&str, RolePolicy)] = &[
    ("ads.doctor", RolePolicy::AdsDoctor),
    ("ads.doctor.start", RolePolicy::AdsDoctor),
    ("ads.import_symbols", RolePolicy::AdsImportSymbols),
    ("ads.import_symbols.apply", RolePolicy::AdsImportSymbols),
    ("comm.browse_symbols", RolePolicy::CommBrowseSymbols),
    ("config.set", RolePolicy::ConfigSet),
];

const DEBUG_SURFACE: &[&str] = &[
    "breakpoints.clear",
    "breakpoints.clear_all",
    "breakpoints.clear_id",
    "breakpoints.list",
    "breakpoints.set",
    "debug.breakpoint_locations",
    "debug.evaluate",
    "debug.scopes",
    "debug.stack",
    "debug.state",
    "debug.stops",
    "debug.variables",
    "eval",
    "pause",
    "resume",
    "set",
    "step_in",
    "step_out",
    "step_over",
    "var.force",
    "var.forced",
    "var.unforce",
];

fn names_for_role(role: AccessRole) -> BTreeSet<&'static str> {
    OPERATION_POLICIES
        .iter()
        .filter_map(|policy| match policy.role {
            RolePolicy::Fixed(candidate) if candidate == role => Some(policy.kind),
            _ => None,
        })
        .collect()
}

fn expected_set(names: &'static [&'static str]) -> BTreeSet<&'static str> {
    names.iter().copied().collect()
}

#[test]
fn viewer_operation_set_matches_the_complete_normative_matrix() {
    assert_eq!(names_for_role(AccessRole::Viewer), expected_set(VIEWER));
}

#[test]
fn operator_operation_set_matches_the_complete_normative_matrix() {
    assert_eq!(names_for_role(AccessRole::Operator), expected_set(OPERATOR));
}

#[test]
fn engineer_operation_set_matches_the_complete_normative_matrix() {
    assert_eq!(names_for_role(AccessRole::Engineer), expected_set(ENGINEER));
}

#[test]
fn admin_operation_set_matches_the_complete_normative_matrix() {
    assert_eq!(names_for_role(AccessRole::Admin), expected_set(ADMIN));
}

#[test]
fn parameter_sensitive_operation_set_is_exact() {
    let actual = OPERATION_POLICIES
        .iter()
        .filter_map(|policy| match policy.role {
            RolePolicy::Fixed(_) => None,
            dynamic => Some((policy.kind, dynamic)),
        })
        .collect::<BTreeMap<_, _>>();
    let expected = PARAMETER_SENSITIVE
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, expected);
}

#[test]
fn complete_registry_is_the_disjoint_union_of_reviewed_policy_groups() {
    let expected = VIEWER
        .iter()
        .chain(OPERATOR)
        .chain(ENGINEER)
        .chain(ADMIN)
        .copied()
        .chain(PARAMETER_SENSITIVE.iter().map(|(kind, _)| *kind))
        .collect::<BTreeSet<_>>();
    let actual = OPERATION_POLICIES
        .iter()
        .map(|policy| policy.kind)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), OPERATION_POLICIES.len());
}

#[test]
fn lookup_accepts_only_exact_lowercase_wire_tokens() {
    for policy in OPERATION_POLICIES {
        assert_eq!(operation_policy(policy.kind), Some(*policy));
        assert_eq!(policy.kind, policy.kind.trim());
        assert_eq!(policy.kind, policy.kind.to_ascii_lowercase());
        assert!(operation_policy(&policy.kind.to_ascii_uppercase()).is_none());
        assert!(operation_policy(&format!(" {}", policy.kind)).is_none());
        assert!(operation_policy(&format!("{} ", policy.kind)).is_none());
    }
}

#[test]
fn debug_surface_set_is_exact_and_non_debug_mutations_stay_out() {
    let actual = OPERATION_POLICIES
        .iter()
        .filter(|policy| policy.debug_surface)
        .map(|policy| policy.kind)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected_set(DEBUG_SURFACE));
    for policy in OPERATION_POLICIES {
        assert_eq!(is_debug_request(policy.kind), actual.contains(policy.kind));
    }
    for kind in [
        "io.force",
        "io.write",
        "hmi.write",
        "config.set",
        "bytecode.reload",
    ] {
        assert!(!is_debug_request(kind), "kind={kind}");
    }
}

#[test]
fn every_fixed_policy_resolves_to_its_declared_minimum_role() {
    for (role, kinds) in [
        (AccessRole::Viewer, VIEWER),
        (AccessRole::Operator, OPERATOR),
        (AccessRole::Engineer, ENGINEER),
        (AccessRole::Admin, ADMIN),
    ] {
        for kind in kinds {
            assert_eq!(
                required_role_for_control_request(kind, None),
                role,
                "kind={kind}"
            );
        }
    }
}

#[test]
fn unclassified_and_spelling_variant_requests_fail_safe_to_admin() {
    for kind in [
        "",
        "STATUS",
        " status",
        "status ",
        "future.operation",
        "debug",
        "io",
    ] {
        assert_eq!(
            required_role_for_control_request(kind, None),
            AccessRole::Admin,
            "kind={kind:?}"
        );
    }
}

#[test]
fn ads_doctor_write_intent_is_the_only_role_escalation() {
    for kind in ["ads.doctor", "ads.doctor.start"] {
        for params in [
            None,
            Some(json!({})),
            Some(json!({"writes_enabled": false})),
            Some(json!({"writes_enabled": "false"})),
        ] {
            assert_eq!(
                required_role_for_control_request(kind, params.as_ref()),
                AccessRole::Viewer,
                "kind={kind}, params={params:?}"
            );
        }
        assert_eq!(
            required_role_for_control_request(kind, Some(&json!({"writes_enabled": true}))),
            AccessRole::Engineer,
            "kind={kind}"
        );
    }
}

#[test]
fn any_non_null_live_symbol_target_requires_engineer_even_with_snapshot() {
    for kind in ["ads.import_symbols", "comm.browse_symbols"] {
        for params in [
            json!({"target": {}}),
            json!({"target": {"host": "plc"}, "snapshot": {}}),
            json!({"target": "malformed", "snapshot": {"symbols": []}}),
        ] {
            assert_eq!(
                required_role_for_control_request(kind, Some(&params)),
                AccessRole::Engineer,
                "kind={kind}, params={params}"
            );
        }
        for params in [
            json!({}),
            json!({"target": null}),
            json!({"snapshot": {}}),
            json!({"target": null, "snapshot": {}}),
        ] {
            assert_eq!(
                required_role_for_control_request(kind, Some(&params)),
                AccessRole::Viewer,
                "kind={kind}, params={params}"
            );
        }
    }
}

#[test]
fn symbol_import_apply_is_always_engineer_because_it_writes() {
    for params in [
        None,
        Some(json!({})),
        Some(json!({"snapshot": {"symbols": []}})),
        Some(json!({"target": {"host": "plc"}})),
    ] {
        assert_eq!(
            required_role_for_control_request("ads.import_symbols.apply", params.as_ref()),
            AccessRole::Engineer,
            "params={params:?}"
        );
    }
}

#[test]
fn every_security_or_exposure_config_key_requires_admin() {
    let admin_keys = [
        "control.auth_token",
        "control.debug_enabled",
        "control.mode",
        "mesh.auth_token",
        "mesh.plugin_versions",
        "mesh.role",
        "mesh.zenohd_version",
        "runtime_cloud.links.transports",
        "runtime_cloud.profile",
        "runtime_cloud.wan.allow_write",
        "web.auth",
    ];
    for key in admin_keys {
        let mut object = serde_json::Map::new();
        object.insert(key.to_string(), json!("candidate"));
        let params = serde_json::Value::Object(object);
        assert_eq!(
            required_role_for_control_request("config.set", Some(&params)),
            AccessRole::Admin,
            "key={key}"
        );
    }
    let mixed = json!({
        "log.level": "debug",
        "control.mode": "debug"
    });
    assert_eq!(
        required_role_for_control_request("config.set", Some(&mixed)),
        AccessRole::Admin
    );
}

#[test]
fn ordinary_and_malformed_config_sets_require_at_least_engineer() {
    for params in [
        None,
        Some(json!(null)),
        Some(json!("not-an-object")),
        Some(json!({})),
        Some(json!({"log.level": "debug"})),
        Some(json!({"watchdog.enabled": false})),
    ] {
        assert_eq!(
            required_role_for_control_request("config.set", params.as_ref()),
            AccessRole::Engineer,
            "params={params:?}"
        );
    }
}
