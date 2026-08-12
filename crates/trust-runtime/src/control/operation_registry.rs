use crate::security::AccessRole;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RolePolicy {
    Fixed(AccessRole),
    AdsDoctor,
    AdsImportSymbols,
    CommBrowseSymbols,
    ConfigSet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OperationPolicy {
    pub(super) kind: &'static str,
    pub(super) role: RolePolicy,
    pub(super) debug_surface: bool,
}

const fn operation(kind: &'static str, role: RolePolicy, debug_surface: bool) -> OperationPolicy {
    OperationPolicy {
        kind,
        role,
        debug_surface,
    }
}

pub(super) const OPERATION_POLICIES: &[OperationPolicy] = &[
    operation("status", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation("health", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation("tasks.stats", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation("events.tail", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation("events", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation("faults", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation("config.get", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation(
        "connectors.status",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation("ads.discover", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation("ads.identity", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation(
        "ads.doctor.status",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "ads.route_plan",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation("ads.status", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation(
        "ads.server.status",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "ads.server.symbols",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "ads.server.doctor.status",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "ads.server.route_plan",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "comm.capabilities",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation("comm.schema", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation(
        "comm.discover",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "fleet.topology",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation("io.list", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation("io.read", RolePolicy::Fixed(AccessRole::Viewer), false),
    operation(
        "hmi.schema.get",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "hmi.values.get",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "hmi.trends.get",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "hmi.alarms.get",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "hmi.descriptor.get",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "historian.query",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation(
        "historian.alerts",
        RolePolicy::Fixed(AccessRole::Viewer),
        false,
    ),
    operation("debug.state", RolePolicy::Fixed(AccessRole::Viewer), true),
    operation("debug.stops", RolePolicy::Fixed(AccessRole::Viewer), true),
    operation("debug.stack", RolePolicy::Fixed(AccessRole::Viewer), true),
    operation("debug.scopes", RolePolicy::Fixed(AccessRole::Viewer), true),
    operation(
        "debug.variables",
        RolePolicy::Fixed(AccessRole::Viewer),
        true,
    ),
    operation(
        "debug.breakpoint_locations",
        RolePolicy::Fixed(AccessRole::Viewer),
        true,
    ),
    operation(
        "breakpoints.list",
        RolePolicy::Fixed(AccessRole::Viewer),
        true,
    ),
    operation("var.forced", RolePolicy::Fixed(AccessRole::Viewer), true),
    operation("pause", RolePolicy::Fixed(AccessRole::Operator), true),
    operation("resume", RolePolicy::Fixed(AccessRole::Operator), true),
    operation("restart", RolePolicy::Fixed(AccessRole::Operator), false),
    operation(
        "hmi.alarm.ack",
        RolePolicy::Fixed(AccessRole::Operator),
        false,
    ),
    operation("pair.claim", RolePolicy::Fixed(AccessRole::Operator), false),
    operation("step_in", RolePolicy::Fixed(AccessRole::Engineer), true),
    operation("step_over", RolePolicy::Fixed(AccessRole::Engineer), true),
    operation("step_out", RolePolicy::Fixed(AccessRole::Engineer), true),
    operation(
        "breakpoints.set",
        RolePolicy::Fixed(AccessRole::Engineer),
        true,
    ),
    operation(
        "breakpoints.clear",
        RolePolicy::Fixed(AccessRole::Engineer),
        true,
    ),
    operation(
        "breakpoints.clear_all",
        RolePolicy::Fixed(AccessRole::Engineer),
        true,
    ),
    operation(
        "breakpoints.clear_id",
        RolePolicy::Fixed(AccessRole::Engineer),
        true,
    ),
    operation("eval", RolePolicy::Fixed(AccessRole::Engineer), true),
    operation("set", RolePolicy::Fixed(AccessRole::Engineer), true),
    operation("var.force", RolePolicy::Fixed(AccessRole::Engineer), true),
    operation("var.unforce", RolePolicy::Fixed(AccessRole::Engineer), true),
    operation("io.write", RolePolicy::Fixed(AccessRole::Engineer), false),
    operation("io.force", RolePolicy::Fixed(AccessRole::Engineer), false),
    operation("io.unforce", RolePolicy::Fixed(AccessRole::Engineer), false),
    operation(
        "debug.evaluate",
        RolePolicy::Fixed(AccessRole::Engineer),
        true,
    ),
    operation("hmi.write", RolePolicy::Fixed(AccessRole::Engineer), false),
    operation(
        "hmi.descriptor.update",
        RolePolicy::Fixed(AccessRole::Engineer),
        false,
    ),
    operation(
        "hmi.scaffold.reset",
        RolePolicy::Fixed(AccessRole::Engineer),
        false,
    ),
    operation("comm.apply", RolePolicy::Fixed(AccessRole::Engineer), false),
    operation("comm.test", RolePolicy::Fixed(AccessRole::Engineer), false),
    operation(
        "ads.server.doctor",
        RolePolicy::Fixed(AccessRole::Engineer),
        false,
    ),
    operation(
        "ads.server.doctor.start",
        RolePolicy::Fixed(AccessRole::Engineer),
        false,
    ),
    operation("shutdown", RolePolicy::Fixed(AccessRole::Admin), false),
    operation(
        "bytecode.reload",
        RolePolicy::Fixed(AccessRole::Admin),
        false,
    ),
    operation("ads.route_add", RolePolicy::Fixed(AccessRole::Admin), false),
    operation(
        "ads.route_remove",
        RolePolicy::Fixed(AccessRole::Admin),
        false,
    ),
    operation("pair.start", RolePolicy::Fixed(AccessRole::Admin), false),
    operation("pair.list", RolePolicy::Fixed(AccessRole::Admin), false),
    operation("pair.revoke", RolePolicy::Fixed(AccessRole::Admin), false),
    operation("ads.doctor", RolePolicy::AdsDoctor, false),
    operation("ads.doctor.start", RolePolicy::AdsDoctor, false),
    operation("ads.import_symbols", RolePolicy::AdsImportSymbols, false),
    operation(
        "ads.import_symbols.apply",
        RolePolicy::AdsImportSymbols,
        false,
    ),
    operation("comm.browse_symbols", RolePolicy::CommBrowseSymbols, false),
    operation("config.set", RolePolicy::ConfigSet, false),
];

pub(super) fn operation_policy(kind: &str) -> Option<OperationPolicy> {
    OPERATION_POLICIES
        .iter()
        .copied()
        .find(|policy| policy.kind == kind)
}

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn reviewed_operation_registry_has_unique_names_and_default_deny_boundary() {
        let names = OPERATION_POLICIES
            .iter()
            .map(|policy| policy.kind)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), OPERATION_POLICIES.len());
        assert!(operation_policy("status").is_some());
        assert!(operation_policy("future.unclassified.operation").is_none());
    }

    #[test]
    fn debug_surface_classification_is_owned_by_the_operation_registry() {
        assert!(operation_policy("pause").is_some_and(|policy| policy.debug_surface));
        assert!(operation_policy("debug.evaluate").is_some_and(|policy| policy.debug_surface));
        assert!(!operation_policy("io.force").is_some_and(|policy| policy.debug_surface));
    }
}
