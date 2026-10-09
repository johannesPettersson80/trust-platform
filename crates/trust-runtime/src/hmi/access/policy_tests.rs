//! Specification 39, §12.2: the effective permission is the intersection of the runtime's write
//! policy and the control's page permissions.

use super::config::WriteRule;
use super::policy::{
    authorize, command_role, effective_permission, rule_for, within_limits, Permission,
};
use crate::hmi::member_paths::MemberPath;
use crate::hmi::page_files::{parse_page, HmiObject};
use crate::security::AccessRole;

fn path(text: &str) -> MemberPath {
    MemberPath::parse(text).expect("a path")
}

fn rule(text: &str, role: AccessRole, min: Option<f64>, max: Option<f64>) -> WriteRule {
    let (prefix, below) = match text.strip_suffix(".*") {
        Some(prefix) => (prefix, true),
        None => (text, false),
    };
    WriteRule {
        path: path(prefix),
        below,
        role,
        min,
        max,
    }
}

fn control(keys: &str) -> HmiObject {
    let text = format!("format = 1\ntitle = \"P\"\n\n[[object]]\nid = \"sp\"\ntype = \"setpoint\"\nat = [0, 0]\nsize = [200, 48]\n{keys}\n");
    parse_page("hmi/pages/p.toml", &text)
        .expect("the page loads")
        .objects
        .remove(0)
}

fn policy() -> Vec<WriteRule> {
    vec![
        rule("Loop.*", AccessRole::Engineer, None, None),
        rule(
            "Loop.Setpoint",
            AccessRole::Operator,
            Some(0.0),
            Some(120.0),
        ),
    ]
}

#[test]
fn the_most_specific_rule_decides() {
    let policy = policy();
    assert_eq!(
        rule_for(&policy, &path("Loop.Setpoint")).map(|rule| rule.role),
        Some(AccessRole::Operator)
    );
    assert_eq!(
        rule_for(&policy, &path("Loop.Gain")).map(|rule| rule.role),
        Some(AccessRole::Engineer)
    );
    assert!(
        rule_for(&policy, &path("Plant.Valve")).is_none(),
        "deny by default"
    );
}

#[test]
fn a_point_without_a_rule_cannot_be_written() {
    let refused =
        effective_permission(&policy(), &path("Plant.Valve"), &control("")).expect_err("refused");
    assert!(
        refused.contains("Plant.Valve") && refused.contains("write policy"),
        "{refused}"
    );
}

#[test]
fn page_permissions_only_narrow_the_policy() {
    let policy = policy();
    let setpoint = path("Loop.Setpoint");
    assert_eq!(
        effective_permission(&policy, &setpoint, &control("")).expect("allowed"),
        Permission {
            role: AccessRole::Operator,
            min: Some(0.0),
            max: Some(120.0)
        }
    );
    let narrowed = effective_permission(
        &policy,
        &setpoint,
        &control("min = 20.0\nmax = 80.0\nrole = \"engineer\""),
    )
    .expect("allowed");
    assert_eq!(
        narrowed,
        Permission {
            role: AccessRole::Engineer,
            min: Some(20.0),
            max: Some(80.0)
        }
    );
    let wider = effective_permission(&policy, &setpoint, &control("min = -10.0\nmax = 200.0"))
        .expect("allowed");
    assert_eq!(
        (wider.min, wider.max),
        (Some(0.0), Some(120.0)),
        "the policy's limits stay"
    );
    let lower_role =
        effective_permission(&policy, &path("Loop.Gain"), &control("role = \"operator\""))
            .expect("allowed");
    assert_eq!(
        lower_role.role,
        AccessRole::Engineer,
        "the policy's role stays"
    );
    let disjoint = effective_permission(&policy, &setpoint, &control("min = 130.0\nmax = 140.0"))
        .expect_err("no common range");
    assert!(disjoint.contains("limits"), "{disjoint}");
}

#[test]
fn a_request_is_checked_against_the_role_and_the_limits() {
    let permission = Permission {
        role: AccessRole::Engineer,
        min: Some(0.0),
        max: Some(120.0),
    };
    assert!(authorize(&permission, AccessRole::Engineer).is_ok());
    let refused = authorize(&permission, AccessRole::Operator).expect_err("operator");
    assert!(refused.to_lowercase().contains("engineer"), "{refused}");
    assert!(authorize(&permission, AccessRole::Viewer).is_err());
    assert!(within_limits(&permission, 60.0).is_ok());
    assert!(within_limits(&permission, 0.0).is_ok() && within_limits(&permission, 120.0).is_ok());
    let above = within_limits(&permission, 121.0).expect_err("above");
    assert!(above.contains("121") && above.contains("120"), "{above}");
    assert!(within_limits(&permission, -0.5).is_err());
}

/// Specification 39, §21.2 item 1: a method command needs the control's own role, Operator unless
/// it says Engineer, and no write rule; a rule that covers the instance raises the role.
#[test]
fn a_method_command_needs_the_controls_role_raised_by_a_covering_rule() {
    let policy = policy();
    assert_eq!(
        command_role(&[], &path("Main.P101"), &control("")),
        AccessRole::Operator,
        "zero-config: no rule is needed"
    );
    assert_eq!(
        command_role(&[], &path("Main.P101"), &control("role = \"engineer\"")),
        AccessRole::Engineer
    );
    assert_eq!(
        command_role(&policy, &path("Loop.Pump"), &control("")),
        AccessRole::Engineer,
        "Loop.* asks for an Engineer"
    );
    assert_eq!(
        command_role(
            &policy,
            &path("Loop.Setpoint"),
            &control("role = \"engineer\"")
        ),
        AccessRole::Engineer,
        "the higher of the two"
    );
}
