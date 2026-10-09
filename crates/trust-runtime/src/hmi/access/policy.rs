//! The effective permission of a control (specification 39, §12.2): the runtime's write policy
//! narrowed by the control's own role and limits, and the checks of one request against it.

use std::collections::BTreeMap;

use crate::hmi::catalogue::{Catalogue, ComponentCategory};
use crate::hmi::member_paths::MemberPath;
use crate::hmi::page_files::{HmiBindingDirection, HmiObject, HmiPage, HmiRole};
use crate::security::AccessRole;

use super::config::WriteRule;

/// Who may write a point from one control, and within which limits.
#[derive(Debug, Clone, PartialEq)]
pub struct Permission {
    /// The lowest role that may send the command.
    pub role: AccessRole,
    /// The lowest number that may be written.
    pub min: Option<f64>,
    /// The highest number that may be written.
    pub max: Option<f64>,
}

/// The policy rule for a point: an exact rule, else the rule for the longest path above it.
pub fn rule_for<'a>(policy: &'a [WriteRule], path: &MemberPath) -> Option<&'a WriteRule> {
    policy
        .iter()
        .filter(|rule| rule.covers(path))
        .max_by_key(|rule| (!rule.below, rule.path.segments.len()))
}

/// The permission a control has to write `path`, or why it has none.
pub fn effective_permission(
    policy: &[WriteRule],
    path: &MemberPath,
    control: &HmiObject,
) -> Result<Permission, String> {
    let shown = display(path);
    let rule = rule_for(policy, path)
        .ok_or_else(|| format!("\"{shown}\" is not writable by the runtime's write policy"))?;
    let role = match control.role.map(control_role) {
        Some(asked) if asked > rule.role => asked,
        _ => rule.role,
    };
    let (own_min, own_max) = own_limits(control);
    let min = narrower(rule.min, own_min, f64::max);
    let max = narrower(rule.max, own_max, f64::min);
    if let (Some(min), Some(max)) = (min, max) {
        if min > max {
            return Err(format!("the limits of the control and the write policy for \"{shown}\" have no value in common"));
        }
    }
    Ok(Permission { role, min, max })
}

/// The role a control's method command needs (specification 39, §21.2 item 1): the control's own
/// `role`, Operator unless it says Engineer, raised to the write policy's role when a rule covers
/// the instance. A method command needs no write rule.
pub fn command_role(
    policy: &[WriteRule],
    instance: &MemberPath,
    control: &HmiObject,
) -> AccessRole {
    let own = control.role.map_or(AccessRole::Operator, control_role);
    rule_for(policy, instance).map_or(own, |rule| own.max(rule.role))
}

/// A control's own entry limits: its `min` and `max`, a slider's from 0 to 100 unless it says
/// otherwise (§5.8).
pub fn own_limits(control: &HmiObject) -> (Option<f64>, Option<f64>) {
    match control.kind.as_str() {
        "slider" => (control.min.or(Some(0.0)), control.max.or(Some(100.0))),
        _ => (control.min, control.max),
    }
}

/// The lowest role each control of a page needs, by object id (specification 39, §5.9 item 5): the
/// policy's, raised by the control's own, for everything it writes; none where the policy does not
/// let it write. A method command needs its [`command_role`]. Objects that write nothing are not
/// listed.
pub fn control_roles(
    page: &HmiPage,
    catalogue: &Catalogue,
    policy: &[WriteRule],
) -> BTreeMap<String, Option<AccessRole>> {
    let mut roles = BTreeMap::new();
    for object in &page.objects {
        let is_control = catalogue
            .component(&object.kind)
            .is_some_and(|component| component.category == ComponentCategory::Control);
        if !is_control {
            continue;
        }
        if let Some(command) = &object.command {
            let role = MemberPath::parse(&command.instance)
                .ok()
                .map(|path| command_role(policy, &path, object));
            roles.insert(object.id.clone(), role);
            continue;
        }
        let mut needed: Option<Option<AccessRole>> = None;
        for binding in object
            .bind
            .values()
            .filter(|binding| binding.direction != HmiBindingDirection::Read)
        {
            let role = MemberPath::parse(&binding.path)
                .ok()
                .and_then(|path| effective_permission(policy, &path, object).ok())
                .map(|permission| permission.role);
            needed = Some(match (needed, role) {
                (None, role) => role,
                (Some(Some(before)), Some(role)) => Some(before.max(role)),
                _ => None,
            });
        }
        if let Some(role) = needed {
            roles.insert(object.id.clone(), role);
        }
    }
    roles
}

/// The tighter of two optional limits.
fn narrower(
    policy: Option<f64>,
    control: Option<f64>,
    tighter: fn(f64, f64) -> f64,
) -> Option<f64> {
    match (policy, control) {
        (Some(policy), Some(control)) => Some(tighter(policy, control)),
        (limit, None) | (None, limit) => limit,
    }
}

/// A path as written.
fn display(path: &MemberPath) -> String {
    use crate::hmi::member_paths::PathSegment;
    let mut text = path.root.to_string();
    for segment in &path.segments {
        match segment {
            PathSegment::Field(name) => {
                text.push('.');
                text.push_str(name);
            }
            PathSegment::Index(indices) => {
                let list = indices
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                text.push_str(&format!("[{list}]"));
            }
        }
    }
    text
}

/// Whether a role may use the permission.
pub fn authorize(permission: &Permission, role: AccessRole) -> Result<(), String> {
    require_role(permission.role, role)
}

/// Whether a role reaches the role a command needs.
pub fn require_role(needed: AccessRole, role: AccessRole) -> Result<(), String> {
    if role.allows(needed) {
        Ok(())
    } else {
        Err(format!(
            "this command needs the {} role",
            capitalized(needed)
        ))
    }
}

fn capitalized(role: AccessRole) -> String {
    let name = role.as_str();
    name[..1].to_uppercase() + &name[1..]
}

/// Whether a number is within the permission's limits.
pub fn within_limits(permission: &Permission, value: f64) -> Result<(), String> {
    if let Some(min) = permission.min.filter(|min| value < *min) {
        return Err(format!("{value} is below the lowest allowed value {min}"));
    }
    if let Some(max) = permission.max.filter(|max| value > *max) {
        return Err(format!("{value} is above the highest allowed value {max}"));
    }
    Ok(())
}

/// The role a control asks for.
pub fn control_role(role: HmiRole) -> AccessRole {
    match role {
        HmiRole::Operator => AccessRole::Operator,
        HmiRole::Engineer => AccessRole::Engineer,
    }
}
