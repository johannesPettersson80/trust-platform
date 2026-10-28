//! The SFC editor's Validate: the checks of an SFC program (specification 25 §11.4,
//! `sfc-profile.md`, "What Validate refuses").
//!
//! An SFC needs an initial step, unique step names, transitions between existing nodes with a
//! condition, and well-formed parallel branches: a split with at least two branches, one incoming
//! transition and one transition to each branch; a join with at least two branches, one transition
//! from each branch and one outgoing transition to its next step. The checks of the chart's step
//! names, actions, networks and declared names are in [`super::chart`]. The companion is made only
//! of a chart these checks accept.

use super::{branch_ids, chart, Workspace};
use crate::js::trim;
use crate::js::{strict_eq, string, truthy};
use crate::Json;

/// What a finding is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueKind {
    /// A step.
    Step,
    /// A transition.
    Transition,
    /// A parallel split or join and its transitions.
    Connection,
    /// An action of a step.
    Action,
    /// A variable of the program.
    Variable,
}

/// One finding of [`validate`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Issue {
    /// A stable id naming the rule and the element, for example `split_branch_count_split_1`.
    pub id: String,
    /// What the finding is about.
    #[serde(rename = "type")]
    pub kind: IssueKind,
    /// What is wrong, in a sentence.
    pub message: String,
    /// The id of the element the finding is about, when there is one.
    #[serde(rename = "elementId", skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
}

impl Issue {
    pub(super) fn new(
        id: String,
        kind: IssueKind,
        message: String,
        element: Option<&Json>,
    ) -> Self {
        Self {
            id,
            kind,
            message,
            element: element.map(|element| string(Some(element))),
        }
    }
}

/// The findings of the SFC program `program`, in the order the checks run; none when it is valid.
///
/// # Errors
/// The file is not an SFC program: steps or transitions are not arrays, or a parallel split or
/// join has no `branchIds` array.
pub fn validate(program: &Json) -> Result<Vec<Issue>, String> {
    let workspace = Workspace::read(program)?;
    let mut issues = Vec::new();
    check_steps(&workspace, &mut issues);
    let before_structure = issues.len();
    check_transitions(&workspace, &mut issues);
    check_splits(&workspace, &mut issues);
    check_joins(&workspace, &mut issues);
    let structure_ok = issues.len() == before_structure;
    let variables = program
        .get("variables")
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    issues.extend(chart::check(&workspace, variables, structure_ok));
    Ok(issues)
}

/// The ids of the steps.
fn step_ids<'a>(workspace: &Workspace<'a>) -> Vec<Option<&'a Json>> {
    workspace.steps.iter().map(|step| step.get("id")).collect()
}

fn contains(ids: &[Option<&Json>], id: Option<&Json>) -> bool {
    ids.iter().any(|candidate| strict_eq(*candidate, id))
}

/// The transitions whose member `end` (`sourceStepId` or `targetStepId`) is `id`.
fn transitions_at<'a>(workspace: &Workspace<'a>, end: &str, id: Option<&Json>) -> Vec<&'a Json> {
    workspace
        .transitions
        .iter()
        .filter(|transition| strict_eq(transition.get(end), id))
        .collect()
}

fn check_steps(workspace: &Workspace<'_>, issues: &mut Vec<Issue>) {
    if !workspace
        .steps
        .iter()
        .any(|step| truthy(step.get("initial")))
    {
        issues.push(Issue::new(
            "no_initial_step".to_owned(),
            IssueKind::Step,
            "SFC must have at least one initial step".to_owned(),
            None,
        ));
    }
    let mut names: Vec<Option<&Json>> = Vec::new();
    for step in workspace.steps {
        let name = step.get("name");
        if contains(&names, name) {
            issues.push(Issue::new(
                format!("duplicate_step_{}", string(step.get("id"))),
                IssueKind::Step,
                format!("Duplicate step name: {}", string(name)),
                step.get("id"),
            ));
        }
        names.push(name);
    }
}

fn check_transitions(workspace: &Workspace<'_>, issues: &mut Vec<Issue>) {
    let mut node_ids = step_ids(workspace);
    node_ids.extend(workspace.splits.iter().map(|split| split.get("id")));
    node_ids.extend(workspace.joins.iter().map(|join| join.get("id")));
    for transition in workspace.transitions {
        let id = transition.get("id");
        let source = transition.get("sourceStepId");
        let target = transition.get("targetStepId");
        if !contains(&node_ids, source) {
            issues.push(Issue::new(
                format!("invalid_source_{}", string(id)),
                IssueKind::Transition,
                format!(
                    "Transition references non-existent source node: {}",
                    string(source)
                ),
                id,
            ));
        }
        if !contains(&node_ids, target) {
            issues.push(Issue::new(
                format!("invalid_target_{}", string(id)),
                IssueKind::Transition,
                format!(
                    "Transition references non-existent target node: {}",
                    string(target)
                ),
                id,
            ));
        }
        if !has_condition(transition.get("condition")) {
            issues.push(Issue::new(
                format!("empty_condition_{}", string(id)),
                IssueKind::Transition,
                "Transition must have a condition".to_owned(),
                id,
            ));
        }
    }
}

/// Whether a transition has a condition: a value that is present and, when it is text, not blank.
fn has_condition(condition: Option<&Json>) -> bool {
    match condition {
        Some(Json::String(condition)) => !trim(condition).is_empty(),
        condition => truthy(condition),
    }
}

/// How many of `transitions` have the member `end` equal to `id`.
fn count_at(transitions: &[&Json], end: &str, id: Option<&Json>) -> usize {
    transitions
        .iter()
        .filter(|transition| strict_eq(transition.get(end), id))
        .count()
}

fn connection_issue(id: String, message: String, element: Option<&Json>) -> Issue {
    Issue::new(id, IssueKind::Connection, message, element)
}

fn check_splits(workspace: &Workspace<'_>, issues: &mut Vec<Issue>) {
    let steps = step_ids(workspace);
    for split in workspace.splits {
        let id = split.get("id");
        let name = string(split.get("name"));
        let branches: Vec<Option<&Json>> = branch_ids(split)
            .unwrap_or_default()
            .iter()
            .map(Some)
            .collect();
        if branches.len() < 2 {
            issues.push(connection_issue(
                format!("split_branch_count_{}", string(id)),
                format!("Parallel split {name} must define at least 2 branches"),
                id,
            ));
        }
        for branch in &branches {
            if !contains(&steps, *branch) {
                issues.push(connection_issue(
                    format!("split_invalid_branch_{}_{}", string(id), string(*branch)),
                    format!(
                        "Parallel split {name} references unknown branch step: {}",
                        string(*branch)
                    ),
                    id,
                ));
            }
        }
        if transitions_at(workspace, "targetStepId", id).len() != 1 {
            issues.push(connection_issue(
                format!("split_incoming_{}", string(id)),
                format!("Parallel split {name} must have exactly one incoming transition"),
                id,
            ));
        }
        let outgoing = transitions_at(workspace, "sourceStepId", id);
        for branch in &branches {
            if count_at(&outgoing, "targetStepId", *branch) != 1 {
                issues.push(connection_issue(
                    format!("split_outgoing_{}_{}", string(id), string(*branch)),
                    format!(
                        "Parallel split {name} must connect to branch {} exactly once",
                        string(*branch)
                    ),
                    id,
                ));
            }
        }
        for transition in &outgoing {
            let target = transition.get("targetStepId");
            if !contains(&branches, target) {
                issues.push(connection_issue(
                    format!(
                        "split_extra_target_{}_{}",
                        string(id),
                        string(transition.get("id"))
                    ),
                    format!(
                        "Parallel split {name} has transition {} to non-branch target {}",
                        string(transition.get("id")),
                        string(target)
                    ),
                    id,
                ));
            }
        }
    }
}

fn check_joins(workspace: &Workspace<'_>, issues: &mut Vec<Issue>) {
    let steps = step_ids(workspace);
    for join in workspace.joins {
        let id = join.get("id");
        let name = string(join.get("name"));
        let branches: Vec<Option<&Json>> = branch_ids(join)
            .unwrap_or_default()
            .iter()
            .map(Some)
            .collect();
        if branches.len() < 2 {
            issues.push(connection_issue(
                format!("join_branch_count_{}", string(id)),
                format!("Parallel join {name} must define at least 2 branches"),
                id,
            ));
        }
        for branch in &branches {
            if !contains(&steps, *branch) {
                issues.push(connection_issue(
                    format!("join_invalid_branch_{}_{}", string(id), string(*branch)),
                    format!(
                        "Parallel join {name} references unknown branch step: {}",
                        string(*branch)
                    ),
                    id,
                ));
            }
        }
        let incoming = transitions_at(workspace, "targetStepId", id);
        for branch in &branches {
            if count_at(&incoming, "sourceStepId", *branch) != 1 {
                issues.push(connection_issue(
                    format!("join_incoming_{}_{}", string(id), string(*branch)),
                    format!(
                        "Parallel join {name} must receive branch {} exactly once",
                        string(*branch)
                    ),
                    id,
                ));
            }
        }
        for transition in &incoming {
            let source = transition.get("sourceStepId");
            if !contains(&branches, source) {
                issues.push(connection_issue(
                    format!(
                        "join_extra_source_{}_{}",
                        string(id),
                        string(transition.get("id"))
                    ),
                    format!(
                        "Parallel join {name} has transition {} from non-branch source {}",
                        string(transition.get("id")),
                        string(source)
                    ),
                    id,
                ));
            }
        }
        let outgoing = transitions_at(workspace, "sourceStepId", id);
        if outgoing.len() != 1 {
            issues.push(connection_issue(
                format!("join_outgoing_{}", string(id)),
                format!("Parallel join {name} must have exactly one outgoing transition"),
                id,
            ));
        }
        check_join_next(join, &steps, &outgoing, issues);
    }
}

/// The checks of a join's `nextStepId`: a step that exists, and the target of its one outgoing
/// transition.
fn check_join_next(
    join: &Json,
    steps: &[Option<&Json>],
    outgoing: &[&Json],
    issues: &mut Vec<Issue>,
) {
    let next = join.get("nextStepId");
    if !truthy(next) {
        return;
    }
    let id = join.get("id");
    let name = string(join.get("name"));
    if !contains(steps, next) {
        issues.push(connection_issue(
            format!("join_next_missing_{}", string(id)),
            format!(
                "Parallel join {name} references unknown next step: {}",
                string(next)
            ),
            id,
        ));
    }
    if let Some(transition) = outgoing.first() {
        let target = transition.get("targetStepId");
        if !strict_eq(target, next) {
            issues.push(connection_issue(
                format!("join_next_mismatch_{}", string(id)),
                format!(
                    "Parallel join {name} nextStepId ({}) does not match outgoing transition target ({})",
                    string(next),
                    string(target)
                ),
                id,
            ));
        }
    }
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
