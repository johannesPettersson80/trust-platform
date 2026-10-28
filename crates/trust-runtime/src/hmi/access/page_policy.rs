//! Controls against the runtime's write policy in the editor (specification 39, §12.2.3): a
//! control that needs more than the policy allows is an error. That covers writing a point the
//! policy does not cover, asking a lower role, or wider limits than the policy's. A method command
//! needs no write rule (§21.2 item 1).

use crate::hmi::binding_checks::BindingProblem;
use crate::hmi::catalogue::{Catalogue, ComponentCategory};
use crate::hmi::member_paths::MemberPath;
use crate::hmi::page_files::{HmiBindingDirection, HmiObject, HmiPage};
use crate::security::AccessRole;

use super::config::WriteRule;
use super::policy::{control_role, rule_for};

/// Every control's writing bindings checked against the write policy.
pub fn check_page_policy(
    page: &HmiPage,
    catalogue: &Catalogue,
    policy: &[WriteRule],
) -> Vec<BindingProblem> {
    let mut problems = Vec::new();
    for object in &page.objects {
        let is_control = catalogue
            .component(&object.kind)
            .is_some_and(|component| component.category == ComponentCategory::Control);
        if !is_control {
            continue;
        }
        if let Some(command) = &object.command
            && let Ok(path) = MemberPath::parse(&command.instance)
        {
            // A method command needs no write rule; a rule that covers its instance raises
            // the role it needs (§21.2 item 1).
            let message = rule_for(policy, &path)
                .and_then(|rule| needs_more(rule, object, &command.instance));
            if let Some(message) = message {
                problems.push(BindingProblem {
                    object: object.id.clone(),
                    property: Some("command".into()),
                    message,
                    point: None,
                });
            }
        }
        for (property, binding) in &object.bind {
            if binding.direction == HmiBindingDirection::Read {
                continue;
            }
            // A malformed path is the binding checks' to report.
            let Ok(path) = MemberPath::parse(&binding.path) else {
                continue;
            };
            // A point no rule covers is named, so that the editor's quick fix can allow it.
            let (message, point) = match rule_for(policy, &path) {
                None => (
                    Some(format!(
                        "\"{}\" is not writable by the runtime's write policy",
                        binding.path
                    )),
                    Some(binding.path.clone()),
                ),
                Some(rule) => (needs_more(rule, object, &binding.path), None),
            };
            if let Some(message) = message {
                problems.push(BindingProblem {
                    object: object.id.clone(),
                    property: Some(property.clone()),
                    message,
                    point,
                });
            }
        }
    }
    problems
}

/// What the control asks beyond the rule: a lower role or wider limits.
fn needs_more(rule: &WriteRule, object: &HmiObject, path: &str) -> Option<String> {
    if let Some(asked) = object.role.map(control_role)
        && asked < rule.role
    {
        return Some(format!(
                "the runtime's write policy lets only the {} role write \"{path}\", but the control asks for {}",
                role_name(rule.role),
                role_name(asked)
            ));
    }
    let lower = matches!((rule.min, object.min), (Some(policy), Some(control)) if control < policy);
    let higher =
        matches!((rule.max, object.max), (Some(policy), Some(control)) if control > policy);
    (lower || higher).then(|| {
        format!(
            "the control's limits {} are wider than the write policy's {} for \"{path}\"",
            range(object.min, object.max),
            range(rule.min, rule.max)
        )
    })
}

fn role_name(role: AccessRole) -> String {
    let name = role.as_str();
    name[..1].to_uppercase() + &name[1..]
}

fn range(min: Option<f64>, max: Option<f64>) -> String {
    let end =
        |limit: Option<f64>| limit.map_or_else(|| "any".to_string(), |limit| limit.to_string());
    format!("{} to {}", end(min), end(max))
}
