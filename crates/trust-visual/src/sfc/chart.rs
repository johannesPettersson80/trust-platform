//! The chart an SFC program draws, read for execution (`sfc-profile.md`, "Execution of a visual
//! SFC"): its steps, its IEC transitions and its actions with their associations, and the checks
//! of the chart that [`super::validate`] adds to the structure checks.
//!
//! - A transition between two steps is one IEC transition. A parallel split, with the transition
//!   into it and its transitions to its branches, is one IEC transition (IEC 61131-3 Table 61
//!   feature 4a); a parallel join, with its transitions from its branches and its outgoing
//!   transition, is one too (feature 4c).
//! - An action is identified by its name, ignoring case; each entry with that name in a step is an
//!   association of the action with that step (Table 57 feature 1).

use std::cmp::Ordering;

use trust_syntax::TokenKind;

use super::validate::{Issue, IssueKind};
use super::{branch_ids, Workspace};
use crate::js::{strict_eq, string, trim, truthy};
use crate::json::number_text;
use crate::Json;

/// An action qualifier (IEC 61131-3 Table 59).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Qualifier {
    /// Non-stored.
    N,
    /// Overriding reset.
    R,
    /// Set (stored).
    S,
    /// Time limited.
    L,
    /// Time delayed.
    D,
    /// Pulse.
    P,
    /// Stored and time delayed.
    Sd,
    /// Delayed and stored.
    Ds,
    /// Stored and time limited.
    Sl,
    /// Pulse on the rising edge.
    P1,
    /// Pulse on the falling edge.
    P0,
}

impl Qualifier {
    /// The qualifier `text` names, ignoring case; an empty text is `N` (Table 59 feature 1).
    fn parse(text: &str) -> Option<Self> {
        Some(match trim(text).to_ascii_uppercase().as_str() {
            "" | "N" => Self::N,
            "R" => Self::R,
            "S" => Self::S,
            "L" => Self::L,
            "D" => Self::D,
            "P" => Self::P,
            "SD" => Self::Sd,
            "DS" => Self::Ds,
            "SL" => Self::Sl,
            "P1" => Self::P1,
            "P0" => Self::P0,
            _ => return None,
        })
    }

    /// The qualifier as Table 59 writes it.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::N => "N",
            Self::R => "R",
            Self::S => "S",
            Self::L => "L",
            Self::D => "D",
            Self::P => "P",
            Self::Sd => "SD",
            Self::Ds => "DS",
            Self::Sl => "SL",
            Self::P1 => "P1",
            Self::P0 => "P0",
        }
    }

    /// Whether the qualifier has a duration (§6.7.4.5: L, D, SD, DS and SL).
    pub(super) fn is_timed(self) -> bool {
        matches!(self, Self::L | Self::D | Self::Sd | Self::Ds | Self::Sl)
    }
}

/// A step of the chart.
pub(super) struct Step<'a> {
    /// The step's `id`.
    id: Option<&'a Json>,
    /// The step's `name` as the file holds it.
    raw_name: Option<&'a Json>,
    /// The step's name, trimmed.
    pub(super) name: String,
    /// Whether the step is an initial step.
    pub(super) initial: bool,
}

impl Step<'_> {
    /// The step flag: `<name>_active`.
    pub(super) fn flag(&self) -> String {
        format!("{}_active", self.name)
    }

    /// Whether the step is also a variable of its name, read as `<name>.X` and `<name>.T` (IEC
    /// 61131-3 Table 54): its name is an identifier. A keyword (`ON`, `STEP`) is no variable name,
    /// so such a step has only its flag.
    pub(super) fn readable(&self) -> bool {
        is_identifier(&self.name)
    }
}

/// One IEC transition: the steps before it, the steps after it and its condition.
pub(super) struct Transition {
    /// The preceding steps, as indices of [`Chart::steps`].
    pub(super) before: Vec<usize>,
    /// The following steps.
    pub(super) after: Vec<usize>,
    /// The conditions of its parts that are not always true, in order (`sfc-profile.md`).
    pub(super) conditions: Vec<String>,
    /// The name of its first part in the file.
    pub(super) name: String,
    /// The lowest `priority` number of its parts.
    pub(super) priority: Option<f64>,
    /// The file position of its first part.
    first: usize,
}

/// One association of an action with a step.
pub(super) struct Association {
    /// The step, as an index of [`Chart::steps`].
    pub(super) step: usize,
    /// The qualifier.
    pub(super) qualifier: Qualifier,
    /// The duration literal of a time-related qualifier.
    pub(super) duration: Option<String>,
}

/// An action: its name, its statements and its associations in chart order.
pub(super) struct Action {
    /// The name, as its first association writes it.
    pub(super) name: String,
    /// The statements: the first body of its associations that has more than blanks and
    /// comments; empty when none has.
    pub(super) body: String,
    /// The associations, in chart order.
    pub(super) associations: Vec<Association>,
}

/// The duration a timer of an action times with.
pub(super) enum TimerDuration<'a> {
    /// Every association of the qualifier has this duration.
    Fixed(&'a str),
    /// The associations' durations differ: the variable `<action>_<qualifier>_PT` holds the
    /// duration of the first active association, in chart order, and keeps the last one while
    /// none is active.
    Chosen(Vec<(usize, &'a str)>),
}

impl Action {
    /// Whether an association of the action has `qualifier`.
    pub(super) fn has(&self, qualifier: Qualifier) -> bool {
        self.associations
            .iter()
            .any(|association| association.qualifier == qualifier)
    }

    /// The steps of the associations with `qualifier`, in chart order, each once.
    pub(super) fn steps_with(&self, qualifier: Qualifier) -> Vec<usize> {
        let mut steps = Vec::new();
        for association in &self.associations {
            if association.qualifier == qualifier && !steps.contains(&association.step) {
                steps.push(association.step);
            }
        }
        steps
    }

    /// The duration of the timer of the time-related `qualifier`.
    pub(super) fn duration(&self, qualifier: Qualifier) -> TimerDuration<'_> {
        let durations: Vec<(usize, &str)> = self
            .associations
            .iter()
            .filter(|association| association.qualifier == qualifier)
            .filter_map(|association| {
                association
                    .duration
                    .as_deref()
                    .map(|duration| (association.step, duration))
            })
            .collect();
        match durations.first() {
            Some((_, first))
                if durations
                    .iter()
                    .all(|(_, duration)| duration.eq_ignore_ascii_case(first)) =>
            {
                TimerDuration::Fixed(first)
            }
            _ => TimerDuration::Chosen(durations),
        }
    }

    /// Whether the action has statements: a body with more than blanks and comments.
    pub(super) fn has_statements(&self) -> bool {
        has_statements(&self.body)
    }

    /// The name of a part of the action's control: `<action>_<part>`.
    pub(super) fn part(&self, part: &str) -> String {
        format!("{}_{part}", self.name)
    }

    /// The variables the action declares, with their declared types: its flag structure (of type
    /// `flag_type`), then the parts of IEC 61131-3 Figure 23 a) its qualifiers use, in the
    /// figure's order (`sfc-profile.md`, "What the companion declares").
    pub(super) fn declarations(&self, flag_type: &str) -> Vec<(String, String)> {
        let mut declared = vec![(self.name.clone(), flag_type.to_owned())];
        let timer = |declared: &mut Vec<(String, String)>, qualifier: Qualifier| {
            declared.push((
                self.part(&format!("{}_TMR", qualifier.name())),
                "TON".to_owned(),
            ));
            if let TimerDuration::Chosen(durations) = self.duration(qualifier) {
                let first = durations.first().map_or("T#0s", |(_, duration)| duration);
                declared.push((
                    self.part(&format!("{}_PT", qualifier.name())),
                    format!("TIME := {first}"),
                ));
            }
        };
        if self.has(Qualifier::S) {
            declared.push((self.part("S_FF"), "RS".to_owned()));
        }
        if self.has(Qualifier::L) {
            timer(&mut declared, Qualifier::L);
        }
        if self.has(Qualifier::D) {
            timer(&mut declared, Qualifier::D);
        }
        if self.has(Qualifier::P) {
            declared.push((self.part("P_TRIG"), "R_TRIG".to_owned()));
        }
        if self.has(Qualifier::Sd) {
            declared.push((self.part("SD_FF"), "RS".to_owned()));
            timer(&mut declared, Qualifier::Sd);
        }
        if self.has(Qualifier::Ds) {
            timer(&mut declared, Qualifier::Ds);
            declared.push((self.part("DS_FF"), "RS".to_owned()));
        }
        if self.has(Qualifier::Sl) {
            declared.push((self.part("SL_FF"), "RS".to_owned()));
            timer(&mut declared, Qualifier::Sl);
        }
        if self.has(Qualifier::P1) {
            declared.push((self.part("P1_TRIG"), "R_TRIG".to_owned()));
        }
        if self.has(Qualifier::P0) {
            declared.push((self.part("P0_M"), "BOOL".to_owned()));
        }
        declared
    }
}

/// The chart of an SFC program whose structure Validate accepts.
pub(super) struct Chart<'a> {
    /// The steps, in file order.
    pub(super) steps: Vec<Step<'a>>,
    /// The IEC transitions, in priority order (`sfc-profile.md`, "Rules of evolution").
    pub(super) transitions: Vec<Transition>,
    /// The actions, in chart order.
    pub(super) actions: Vec<Action>,
}

impl<'a> Chart<'a> {
    /// The chart of `workspace`. Entries that Validate refuses are left out.
    pub(super) fn read(workspace: &Workspace<'a>) -> Self {
        let steps = read_steps(workspace.steps);
        let (actions, _) = read_actions(workspace.steps, &steps);
        let transitions = read_transitions(workspace, &steps);
        Self {
            steps,
            transitions,
            actions,
        }
    }

    /// The flag of the transition at `index` in priority order: `SFC_T<index + 1>`.
    pub(super) fn transition_flag(index: usize) -> String {
        format!("SFC_T{}", index + 1)
    }

    /// The timer of the step at `index` in chart order, whose elapsed time is the step's `T`:
    /// `SFC_S<index + 1>`.
    pub(super) fn step_timer(index: usize) -> String {
        format!("SFC_S{}", index + 1)
    }
}

fn read_steps(steps: &[Json]) -> Vec<Step<'_>> {
    steps
        .iter()
        .map(|step| Step {
            id: step.get("id"),
            raw_name: step.get("name"),
            name: step
                .get("name")
                .and_then(Json::as_str)
                .map(|name| trim(name).to_owned())
                .unwrap_or_default(),
            initial: truthy(step.get("initial")),
        })
        .collect()
}

/// The items of an array value; nothing for anything else.
fn items(value: Option<&Json>) -> &[Json] {
    value
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn action_issue(id: String, step: &Step<'_>, message: String) -> Issue {
    Issue::new(id, IssueKind::Action, message, step.id)
}

/// One action entry of a step, read.
struct Entry<'j> {
    name: &'j str,
    qualifier: Qualifier,
    duration: Option<String>,
    body: String,
}

/// The action entry `entry` of `step`, or the finding that refuses it.
fn read_entry<'j>(entry: &'j Json, step: &Step<'_>) -> Result<Entry<'j>, Issue> {
    let id = string(entry.get("id"));
    let name = entry
        .get("name")
        .and_then(Json::as_str)
        .map(trim)
        .unwrap_or_default();
    if !is_identifier(name) {
        let message = if name.is_empty() {
            format!("Step {} has an action without a name.", step.name)
        } else {
            format!(
                "Action {name} in step {}: an action name is an identifier (letters, digits and underscores, not a keyword).",
                step.name
            )
        };
        return Err(action_issue(format!("action_name_{id}"), step, message));
    }
    let written = entry
        .get("qualifier")
        .and_then(Json::as_str)
        .unwrap_or_default();
    let Some(qualifier) = Qualifier::parse(written) else {
        return Err(action_issue(
            format!("action_qualifier_{id}"),
            step,
            format!(
                "Action {name} in step {}: {} is not an action qualifier (N, R, S, L, D, P, SD, DS, SL, P1 or P0).",
                step.name,
                trim(written)
            ),
        ));
    };
    let duration = if qualifier.is_timed() {
        let duration = duration(entry.get("time")).map_err(|written| {
            let message = match written {
                None => format!(
                    "Action {name} in step {}: qualifier {} needs a duration such as T#2s.",
                    step.name,
                    qualifier.name()
                ),
                Some(written) => format!(
                    "Action {name} in step {}: {written} is not a duration; write a TIME literal such as T#2s, or a number of milliseconds.",
                    step.name
                ),
            };
            action_issue(format!("action_time_{id}"), step, message)
        })?;
        Some(duration)
    } else {
        None
    };
    Ok(Entry {
        name,
        qualifier,
        duration,
        body: entry
            .get("body")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .replace("\r\n", "\n"),
    })
}

/// The actions of the steps `steps_json` (read as `steps`), and the findings about them.
fn read_actions(steps_json: &[Json], steps: &[Step<'_>]) -> (Vec<Action>, Vec<Issue>) {
    let mut actions: Vec<Action> = Vec::new();
    let mut issues = Vec::new();
    for (index, (json, step)) in steps_json.iter().zip(steps).enumerate() {
        for entry_json in items(json.get("actions")) {
            let entry = match read_entry(entry_json, step) {
                Ok(entry) => entry,
                Err(issue) => {
                    issues.push(issue);
                    continue;
                }
            };
            let id = string(entry_json.get("id"));
            let position = actions
                .iter()
                .position(|action| action.name.eq_ignore_ascii_case(entry.name))
                .unwrap_or_else(|| {
                    actions.push(Action {
                        name: entry.name.to_owned(),
                        body: String::new(),
                        associations: Vec::new(),
                    });
                    actions.len() - 1
                });
            let action = &mut actions[position];
            if has_statements(&entry.body) {
                if action.has_statements() {
                    if trim(&action.body) != trim(&entry.body) {
                        issues.push(action_issue(
                            format!("action_body_{id}"),
                            step,
                            format!(
                                "Action {} has different statements in step {}; an action's statements are written once.",
                                action.name, step.name
                            ),
                        ));
                    }
                } else {
                    action.body = entry.body;
                }
            }
            if entry.qualifier.is_timed()
                && let Some(other) = action.associations.iter().find(|association| {
                    association.step == index && association.qualifier.is_timed()
                })
            {
                issues.push(action_issue(
                    format!("action_timed_twice_{id}"),
                    step,
                    format!(
                        "Action {} has two time-related qualifiers in step {} ({} and {}); they would always be active together.",
                        action.name,
                        step.name,
                        other.qualifier.name(),
                        entry.qualifier.name()
                    ),
                ));
                continue;
            }
            action.associations.push(Association {
                step: index,
                qualifier: entry.qualifier,
                duration: entry.duration,
            });
        }
    }
    (actions, issues)
}

/// The duration literal of an action's `time`: a `TIME` duration literal as written, or
/// `T#<n>ms` for a number of milliseconds. `Err(None)` when there is none, `Err(Some(text))` when
/// `text` is not a duration.
fn duration(value: Option<&Json>) -> Result<String, Option<String>> {
    match value {
        None | Some(Json::Null) => Err(None),
        Some(Json::Number(number)) => {
            let literal = format!("T#{}ms", number_text(*number));
            if *number >= 0.0 && is_duration(&literal) {
                Ok(literal)
            } else {
                Err(Some(number_text(*number)))
            }
        }
        Some(Json::String(text)) => match trim(text) {
            "" => Err(None),
            text if is_duration(text) => Ok(text.to_owned()),
            text => Err(Some(text.to_owned())),
        },
        Some(other) => Err(Some(other.to_compact())),
    }
}

/// Whether `body` has statements: more than blanks and comments.
fn has_statements(body: &str) -> bool {
    trust_syntax::lex(body)
        .iter()
        .any(|token| !token.kind.is_trivia())
}

/// The kind of the one token `text` is, when it is one token and nothing else.
fn single_token(text: &str) -> Option<TokenKind> {
    match trust_syntax::lex(text).as_slice() {
        [token] if usize::from(token.range.len()) == text.len() => Some(token.kind),
        _ => None,
    }
}

/// Whether `text` is one Structured Text identifier: not a keyword, nothing around it.
pub(super) fn is_identifier(text: &str) -> bool {
    single_token(text) == Some(TokenKind::Ident)
}

/// Whether `text` is one `TIME` duration literal that is not negative (IEC 61131-3 Table 8).
fn is_duration(text: &str) -> bool {
    if single_token(text) != Some(TokenKind::TimeLiteral) {
        return false;
    }
    text.split_once('#').is_some_and(|(prefix, value)| {
        (prefix.eq_ignore_ascii_case("T") || prefix.eq_ignore_ascii_case("TIME"))
            && !value.starts_with('-')
    })
}

/// A condition that is always true (§6.7.3): `TRUE` in any case, or `1`.
fn always_true(condition: &str) -> bool {
    let condition = trim(condition);
    condition.eq_ignore_ascii_case("TRUE") || condition == "1"
}

/// The parts of one IEC transition being collected: their file positions.
struct Parts<'w> {
    edges: &'w [Json],
    positions: Vec<usize>,
}

impl<'w> Parts<'w> {
    fn new(edges: &'w [Json]) -> Self {
        Self {
            edges,
            positions: Vec::new(),
        }
    }

    /// Adds the transition from `source` to `target`, when there is one.
    fn add_between(&mut self, source: Option<&Json>, target: Option<&Json>) {
        if let Some(position) = self.edges.iter().position(|edge| {
            strict_eq(edge.get("sourceStepId"), source)
                && strict_eq(edge.get("targetStepId"), target)
        }) {
            self.positions.push(position);
        }
    }

    /// The IEC transition from `before` to `after` made of the parts collected.
    fn transition(self, before: Vec<usize>, after: Vec<usize>) -> Transition {
        let parts: Vec<&Json> = self
            .positions
            .iter()
            .map(|&position| &self.edges[position])
            .collect();
        let conditions = parts
            .iter()
            .map(|part| string(part.get("condition")))
            .filter(|condition| !always_true(condition))
            .map(|condition| trim(&condition).to_owned())
            .collect();
        let first = self.positions.iter().copied().min().unwrap_or_default();
        let name = self
            .edges
            .get(first)
            .and_then(|edge| edge.get("name"))
            .and_then(Json::as_str)
            .map(trim)
            .unwrap_or_default()
            .to_owned();
        let priority = parts
            .iter()
            .filter_map(|part| part.get("priority").and_then(priority_number))
            .min_by(f64::total_cmp);
        Transition {
            before,
            after,
            conditions,
            name,
            priority,
            first,
        }
    }
}

/// The number of a transition's `priority` (`sfc-profile.md`, "Rules of evolution"): a number, or the text that the editor's
/// Priority field holds as typed, trimmed, read as a number. `None` for a value that holds no finite number, such as a blank
/// text, which names no priority; the validation refuses any value that is not a whole number from 0.
pub(super) fn priority_number(value: &Json) -> Option<f64> {
    match value {
        Json::String(text) => trim(text)
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite()),
        other => other.as_finite(),
    }
}

/// The position of the node whose id is `id` among `nodes`.
fn index_of(nodes: &[Json], id: Option<&Json>) -> Option<usize> {
    nodes.iter().position(|node| strict_eq(node.get("id"), id))
}

/// The step whose id is `id`.
fn step_of(steps: &[Step<'_>], id: Option<&Json>) -> Option<usize> {
    steps.iter().position(|step| strict_eq(step.id, id))
}

/// The branch steps of a parallel split or join, in its branch order.
fn branch_steps(steps: &[Step<'_>], node: &Json) -> Vec<usize> {
    branch_ids(node)
        .unwrap_or_default()
        .iter()
        .filter_map(|branch| step_of(steps, Some(branch)))
        .collect()
}

/// The IEC transitions of `workspace`, in priority order: the `priority` numbers lowest first,
/// then the transitions without one; in file order otherwise.
fn read_transitions(workspace: &Workspace<'_>, steps: &[Step<'_>]) -> Vec<Transition> {
    let mut transitions = Vec::new();
    for (position, edge) in workspace.transitions.iter().enumerate() {
        let source = edge.get("sourceStepId");
        let target = edge.get("targetStepId");
        let mut parts = Parts::new(workspace.transitions);
        let before = if let Some(step) = step_of(steps, source) {
            if index_of(workspace.joins, target).is_some() {
                // A transition into a join is a part of the join's transition.
                continue;
            }
            vec![step]
        } else if let Some(join) = index_of(workspace.joins, source) {
            let join = &workspace.joins[join];
            for branch in branch_ids(join).unwrap_or_default() {
                parts.add_between(Some(branch), join.get("id"));
            }
            branch_steps(steps, join)
        } else {
            // A transition from a split is a part of the transition into the split.
            continue;
        };
        parts.positions.push(position);
        let after = if let Some(step) = step_of(steps, target) {
            vec![step]
        } else if let Some(split) = index_of(workspace.splits, target) {
            let split = &workspace.splits[split];
            for branch in branch_ids(split).unwrap_or_default() {
                parts.add_between(split.get("id"), Some(branch));
            }
            branch_steps(steps, split)
        } else {
            continue;
        };
        transitions.push(parts.transition(before, after));
    }
    transitions.sort_by(|left, right| {
        let numbered = match (left.priority, right.priority) {
            (Some(left), Some(right)) => left.total_cmp(&right),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        numbered.then(left.first.cmp(&right.first))
    });
    transitions
}

/// The findings about the chart's step names and actions and, when the structure checks found
/// nothing (`structure_ok`), about its networks and the names its companion declares
/// (`sfc-profile.md`, "What Validate refuses"). `variables` are the chart's variables.
pub(super) fn check(
    workspace: &Workspace<'_>,
    variables: &[Json],
    structure_ok: bool,
) -> Vec<Issue> {
    let steps = read_steps(workspace.steps);
    let mut issues = Vec::new();
    for step in &steps {
        let message = if step.name.is_empty() {
            format!("Step {} has no name.", string(step.id))
        } else if is_identifier(&step.flag()) {
            continue;
        } else {
            format!(
                "Step {}: its flag {} is not an identifier; a step name has letters, digits and underscores.",
                step.name,
                step.flag()
            )
        };
        issues.push(Issue::new(
            format!("step_name_{}", string(step.id)),
            IssueKind::Step,
            message,
            step.id,
        ));
    }
    let (actions, action_issues) = read_actions(workspace.steps, &steps);
    issues.extend(action_issues);
    if structure_ok {
        let transitions = read_transitions(workspace, &steps);
        check_networks(&steps, &transitions, &mut issues);
        let chart = Chart {
            steps,
            transitions,
            actions,
        };
        check_names(&chart, variables, &mut issues);
    }
    issues
}

/// The representative of the network of `step` (a union-find root), shortening the path.
fn network_root(network: &mut [usize], mut step: usize) -> usize {
    while network[step] != step {
        network[step] = network[network[step]];
        step = network[step];
    }
    step
}

/// Each network of steps (the steps joined by transitions) has at most one initial step
/// (IEC 61131-3 §6.7.2).
fn check_networks(steps: &[Step<'_>], transitions: &[Transition], issues: &mut Vec<Issue>) {
    let mut network: Vec<usize> = (0..steps.len()).collect();
    for transition in transitions {
        let mut joined = transition.before.iter().chain(&transition.after).copied();
        let Some(first) = joined.next() else {
            continue;
        };
        for step in joined {
            let root = network_root(&mut network, first);
            let other = network_root(&mut network, step);
            network[other] = root;
        }
    }
    let mut initial_of: Vec<Option<usize>> = vec![None; steps.len()];
    for (index, step) in steps.iter().enumerate() {
        if !step.initial {
            continue;
        }
        let root = network_root(&mut network, index);
        if let Some(first) = initial_of[root] {
            issues.push(Issue::new(
                format!("network_initial_{}", string(step.id)),
                IssueKind::Step,
                format!(
                    "Steps {} and {} are initial steps of one network; a network has exactly one initial step.",
                    steps[first].name, step.name
                ),
                step.id,
            ));
        } else {
            initial_of[root] = Some(index);
        }
    }
}

/// A name the companion declares and what declares it.
struct Declared<'a> {
    name: String,
    what: String,
    kind: IssueKind,
    element: Option<&'a Json>,
    /// For a name a step declares, the step's index and its `name` as the file holds it.
    step: Option<(usize, Option<&'a Json>)>,
}

/// Every name the companion declares is declared once, ignoring case.
fn check_names(chart: &Chart<'_>, variables: &[Json], issues: &mut Vec<Issue>) {
    let mut declared: Vec<Declared<'_>> = Vec::new();
    for variable in variables {
        let name = trim(&string(variable.get("name"))).to_owned();
        declared.push(Declared {
            what: format!("variable {name}"),
            name,
            kind: IssueKind::Variable,
            element: None,
            step: None,
        });
    }
    for (index, step) in chart.steps.iter().enumerate() {
        declared.push(Declared {
            name: step.flag(),
            what: format!("the flag of step {}", step.name),
            kind: IssueKind::Step,
            element: step.id,
            step: Some((index, step.raw_name)),
        });
        // The step itself, read as `<name>.X` and `<name>.T` (IEC 61131-3 Table 54), and its timer.
        if !step.readable() {
            continue;
        }
        declared.push(Declared {
            name: step.name.clone(),
            what: format!("step {}", step.name),
            kind: IssueKind::Step,
            element: step.id,
            step: Some((index, step.raw_name)),
        });
        declared.push(Declared {
            name: Chart::step_timer(index),
            what: format!("the timer of step {}", step.name),
            kind: IssueKind::Step,
            element: step.id,
            step: Some((index, step.raw_name)),
        });
    }
    for action in &chart.actions {
        let element = action
            .associations
            .first()
            .and_then(|association| chart.steps[association.step].id);
        for (index, (name, _)) in action.declarations("").into_iter().enumerate() {
            let what = if index == 0 {
                format!("action {}", action.name)
            } else {
                format!("the control of action {}", action.name)
            };
            declared.push(Declared {
                name,
                what,
                kind: IssueKind::Action,
                element,
                step: None,
            });
        }
    }
    for index in 0..chart.transitions.len() {
        let name = Chart::transition_flag(index);
        declared.push(Declared {
            what: format!("the flag of transition {name}"),
            name,
            kind: IssueKind::Transition,
            element: None,
            step: None,
        });
    }
    for (index, later) in declared.iter().enumerate() {
        let Some(earlier) = declared[..index]
            .iter()
            .find(|earlier| earlier.name.eq_ignore_ascii_case(&later.name))
        else {
            continue;
        };
        // Two steps of one name are the duplicate step name Validate reports already.
        if let (Some((first, first_name)), Some((second, second_name))) = (earlier.step, later.step)
            && first != second
            && strict_eq(first_name, second_name)
        {
            continue;
        }
        issues.push(Issue::new(
            format!("name_clash_{}", later.name.to_ascii_lowercase()),
            later.kind,
            format!(
                "The companion would declare {} twice: {} and {}; rename one of them.",
                later.name, earlier.what, later.what
            ),
            later.element,
        ));
    }
}

#[cfg(test)]
#[path = "chart_tests.rs"]
mod tests;
