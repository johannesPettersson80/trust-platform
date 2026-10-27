//! The Structured Text companion of an SFC program (specification 17 §3, specification 25 §11.4,
//! `sfc-profile.md`, "Execution of a visual SFC").
//!
//! The companion is one `FUNCTION_BLOCK FB_<base>_SFC` that executes the chart by IEC 61131-3
//! §6.7 and, when the chart has actions, the type `FB_<base>_SFC_ACTION` of the actions' flag
//! structures after it. The function block holds:
//!
//! 1. the program's variables in a `VAR` section (when it has any), each with its initial value
//!    and its comment;
//! 2. a `VAR` section with one `<step>_active : BOOL` flag per step (`TRUE` for an initial step),
//!    the flag structure and the control parts of each action, and one `SFC_T<n> : BOOL` per IEC
//!    transition;
//! 3. the actions in chart order: each action's control (Figure 23 a), with final scan) and its
//!    statements, which run while its `A` is `TRUE`;
//! 4. the transitions in priority order, each tested against the steps active before the
//!    evolution;
//! 5. the evolution: the steps before each clearing transition are left, then the steps after it
//!    entered.
//!
//! The companion is made only of a chart that Validate accepts. Every direct address the generated
//! code names becomes a global of the runtime entry (`crate::direct_io`).

use super::chart::{Action, Chart, Qualifier, TimerDuration, Transition};
use super::{validate, Workspace};
use crate::direct_io::with_external_globals;
use crate::js::{member, string, trim, truthy};
use crate::json::number_text;
use crate::naming::{comment_text, fb_name_for_source};
use crate::{Generated, Json, Problem, VisualKind};

/// The message when a file is not an SFC program the companion can be made of.
const FORMAT_MESSAGE: &str =
    "Invalid SFC workspace format. Expected JSON object with name, steps, and transitions.";

/// The companion of the SFC program `program` saved as `<base_name>.sfc.json` (or `.sfc`).
///
/// The function block is called `FB_<base_name>_SFC`; without a base name the program's `name`
/// names it, and without that `SFC`.
///
/// # Errors
/// The program has no `name` text, or its steps or transitions are not arrays; or Validate finds
/// something in it, and each finding is a problem.
pub fn generate(program: &Json, base_name: &str) -> Result<Generated, Vec<Problem>> {
    let (Some(name), Some(_), Some(_)) = (
        program.str_at("name"),
        program.get("steps").and_then(Json::as_array),
        program.get("transitions").and_then(Json::as_array),
    ) else {
        return Err(vec![Problem::new(FORMAT_MESSAGE)]);
    };
    let issues = validate(program).map_err(|message| vec![Problem::new(message)])?;
    if !issues.is_empty() {
        return Err(issues
            .into_iter()
            .map(|issue| match issue.element {
                Some(element) => Problem::at(element, issue.message),
                None => Problem::new(issue.message),
            })
            .collect());
    }
    let workspace = Workspace::read(program).map_err(|message| vec![Problem::new(message)])?;
    let chart = Chart::read(&workspace);
    let source = if base_name.is_empty() {
        match trim(name) {
            "" => "SFC",
            name => name,
        }
    } else {
        base_name
    };
    let block = fb_name_for_source(source, VisualKind::Sfc.block_suffix());
    let flag_type = format!("{block}_ACTION");

    let mut lines = vec![format!("FUNCTION_BLOCK {block}")];
    variables(program.get("variables"), &mut lines);
    declarations(&chart, &flag_type, &mut lines);
    if !chart.actions.is_empty() {
        actions(&chart, &mut lines);
    }
    if !chart.transitions.is_empty() {
        transitions(&chart, &mut lines);
        evolution(&chart, &mut lines);
    }
    lines.push("END_FUNCTION_BLOCK".to_owned());
    if !chart.actions.is_empty() {
        lines.extend([
            String::new(),
            format!("TYPE {flag_type} :"),
            "STRUCT".to_owned(),
            "  Q : BOOL; // the action flag".to_owned(),
            "  A : BOOL; // the activation: the action's statements run while A is TRUE".to_owned(),
            "END_STRUCT;".to_owned(),
            "END_TYPE".to_owned(),
        ]);
    }

    // A body or condition written with CRLF line ends gets LF ones, as every other line.
    let text = lines.join("\n").replace("\r\n", "\n");
    let (function_block, globals) = with_external_globals(&text);
    Ok(Generated {
        function_block,
        globals,
        warnings: Vec::new(),
    })
}

/// `text` made safe inside a `//` comment: no line break ends it early (specification 01 §3).
fn line_comment(text: &str) -> String {
    comment_text(text).replace('\u{c}', " ")
}

/// The items of an array value; nothing for anything else.
fn items(value: Option<&Json>) -> &[Json] {
    value
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// The `VAR` section of the program's variables: `name : type [:= initial];[ // comment]`.
fn variables(value: Option<&Json>, lines: &mut Vec<String>) {
    let variables = items(value);
    if variables.is_empty() {
        return;
    }
    lines.push("VAR".to_owned());
    for variable in variables {
        let variable = Some(variable);
        let initial = member(variable, "initialValue");
        let initial = if truthy(initial) {
            format!(" := {}", string(initial))
        } else {
            String::new()
        };
        let comment = member(variable, "comment");
        let comment = if truthy(comment) {
            format!(" // {}", line_comment(&string(comment)))
        } else {
            String::new()
        };
        lines.push(format!(
            "  {} : {}{initial};{comment}",
            string(member(variable, "name")),
            string(member(variable, "type"))
        ));
    }
    lines.push("END_VAR".to_owned());
    lines.push(String::new());
}

/// The `VAR` section of the step flags, the actions and the transition flags.
fn declarations(chart: &Chart<'_>, flag_type: &str, lines: &mut Vec<String>) {
    lines.push("VAR".to_owned());
    lines.push("  // Steps: TRUE while the step is active (IEC 61131-3 6.7.2)".to_owned());
    for step in &chart.steps {
        let initial = if step.initial { "TRUE" } else { "FALSE" };
        lines.push(format!("  {} : BOOL := {initial};", step.flag()));
    }
    if !chart.actions.is_empty() {
        lines.push("  // Actions: the flags Q and A and the action control (6.7.4.6)".to_owned());
        for action in &chart.actions {
            for (name, declared) in action.declarations(flag_type) {
                lines.push(format!("  {name} : {declared};"));
            }
        }
    }
    if !chart.transitions.is_empty() {
        lines.push(
            "  // Transitions: TRUE after a scan in which the transition cleared (6.7.5)"
                .to_owned(),
        );
        for index in 0..chart.transitions.len() {
            lines.push(format!("  {} : BOOL;", Chart::transition_flag(index)));
        }
    }
    lines.push("END_VAR".to_owned());
    lines.push(String::new());
}

/// The actions, in chart order.
fn actions(chart: &Chart<'_>, lines: &mut Vec<String>) {
    lines.push(
        "// Actions with final scan (IEC 61131-3 6.7.4.6, Table 60 feature 1): A is TRUE while Q is"
            .to_owned(),
    );
    lines.push(
        "// and in the scan after Q falls, and an action's statements run while its A is TRUE."
            .to_owned(),
    );
    for action in &chart.actions {
        lines.push(String::new());
        action_control(chart, action, lines);
    }
    lines.push(String::new());
}

/// The input of `qualifier` of the control of `action`: the flags of the steps of its
/// associations with `qualifier`, joined with OR; `None` when it has none.
fn input(chart: &Chart<'_>, action: &Action, qualifier: Qualifier) -> Option<String> {
    let flags: Vec<String> = action
        .steps_with(qualifier)
        .into_iter()
        .map(|step| chart.steps[step].flag())
        .collect();
    match flags.as_slice() {
        [] => None,
        [flag] => Some(flag.clone()),
        flags => Some(format!("({})", flags.join(" OR "))),
    }
}

/// The duration the timer of `qualifier` of `action` times with: its literal, or the variable that
/// the lines added to `lines` set to the duration of the first active association.
fn timer_duration(
    chart: &Chart<'_>,
    action: &Action,
    qualifier: Qualifier,
    lines: &mut Vec<String>,
) -> String {
    match action.duration(qualifier) {
        TimerDuration::Fixed(duration) => duration.to_owned(),
        TimerDuration::Chosen(durations) => {
            let variable = action.part(&format!("{}_PT", qualifier.name()));
            for (index, (step, duration)) in durations.iter().enumerate() {
                let keyword = if index == 0 { "IF" } else { "ELSIF" };
                lines.push(format!("{keyword} {} THEN", chart.steps[*step].flag()));
                lines.push(format!("  {variable} := {duration};"));
            }
            if !durations.is_empty() {
                lines.push("END_IF;".to_owned());
            }
            variable
        }
    }
}

/// The control of `action` (IEC 61131-3 Figure 23 a), only the parts it uses) and its statements.
fn action_control(chart: &Chart<'_>, action: &Action, lines: &mut Vec<String>) {
    let associations: Vec<String> = action
        .associations
        .iter()
        .map(|association| {
            let step = &chart.steps[association.step].name;
            match &association.duration {
                Some(duration) => format!("{} {duration} {step}", association.qualifier.name()),
                None => format!("{} {step}", association.qualifier.name()),
            }
        })
        .collect();
    lines.push(format!(
        "// Action {}: {}",
        action.name,
        line_comment(&associations.join(", "))
    ));
    let flag = &action.name;
    let reset = input(chart, action, Qualifier::R);
    let reset_input = reset.as_deref().unwrap_or("FALSE");
    let mut terms = Vec::new();
    if let Some(active) = input(chart, action, Qualifier::N) {
        terms.push(active);
    }
    if let Some(active) = input(chart, action, Qualifier::S) {
        let flip_flop = action.part("S_FF");
        lines.push(format!("{flip_flop}(S := {active}, R1 := {reset_input});"));
        terms.push(format!("{flip_flop}.Q1"));
    }
    if let Some(active) = input(chart, action, Qualifier::L) {
        let timer = action.part("L_TMR");
        let duration = timer_duration(chart, action, Qualifier::L, lines);
        lines.push(format!("{timer}(IN := {active}, PT := {duration});"));
        terms.push(format!("({active} AND NOT {timer}.Q)"));
    }
    if let Some(active) = input(chart, action, Qualifier::D) {
        let timer = action.part("D_TMR");
        let duration = timer_duration(chart, action, Qualifier::D, lines);
        lines.push(format!("{timer}(IN := {active}, PT := {duration});"));
        terms.push(format!("{timer}.Q"));
    }
    if let Some(active) = input(chart, action, Qualifier::P) {
        let trigger = action.part("P_TRIG");
        lines.push(format!("{trigger}(CLK := {active});"));
        terms.push(format!("{trigger}.Q"));
    }
    stored_timers(chart, action, reset_input, &mut terms, lines);
    let q = match (reset.as_deref(), terms.as_slice()) {
        (_, []) => "FALSE".to_owned(),
        (None, terms) => terms.join(" OR "),
        (Some(reset), [term]) => format!("NOT {reset} AND {term}"),
        (Some(reset), terms) => format!("NOT {reset} AND ({})", terms.join(" OR ")),
    };
    lines.push(format!("{flag}.A := {flag}.Q; // Q of the previous scan"));
    lines.push(format!("{flag}.Q := {q};"));
    let mut activation = vec![format!("{flag}.A"), format!("{flag}.Q")];
    if let Some(active) = input(chart, action, Qualifier::P1) {
        let trigger = action.part("P1_TRIG");
        lines.push(format!("{trigger}(CLK := {active});"));
        activation.push(format!("{trigger}.Q"));
    }
    let falling = input(chart, action, Qualifier::P0);
    let memory = action.part("P0_M");
    if let Some(active) = &falling {
        activation.push(format!("({memory} AND NOT {active})"));
    }
    lines.push(format!("{flag}.A := {};", activation.join(" OR ")));
    if let Some(active) = falling {
        lines.push(format!("{memory} := {active};"));
    }
    if action.has_statements() {
        lines.push(format!("IF {flag}.A THEN"));
        for line in action.body.split('\n') {
            lines.push(format!("  {line}"));
        }
        lines.push("END_IF;".to_owned());
    }
}

/// The parts of the control of `action` for SD, DS and SL, whose terms of Q are added to `terms`.
fn stored_timers(
    chart: &Chart<'_>,
    action: &Action,
    reset_input: &str,
    terms: &mut Vec<String>,
    lines: &mut Vec<String>,
) {
    if let Some(active) = input(chart, action, Qualifier::Sd) {
        let (flip_flop, timer) = (action.part("SD_FF"), action.part("SD_TMR"));
        lines.push(format!("{flip_flop}(S := {active}, R1 := {reset_input});"));
        let duration = timer_duration(chart, action, Qualifier::Sd, lines);
        lines.push(format!("{timer}(IN := {flip_flop}.Q1, PT := {duration});"));
        terms.push(format!("{timer}.Q"));
    }
    if let Some(active) = input(chart, action, Qualifier::Ds) {
        let (timer, flip_flop) = (action.part("DS_TMR"), action.part("DS_FF"));
        let duration = timer_duration(chart, action, Qualifier::Ds, lines);
        lines.push(format!("{timer}(IN := {active}, PT := {duration});"));
        lines.push(format!("{flip_flop}(S := {timer}.Q, R1 := {reset_input});"));
        terms.push(format!("{flip_flop}.Q1"));
    }
    if let Some(active) = input(chart, action, Qualifier::Sl) {
        let (flip_flop, timer) = (action.part("SL_FF"), action.part("SL_TMR"));
        lines.push(format!("{flip_flop}(S := {active}, R1 := {reset_input});"));
        let duration = timer_duration(chart, action, Qualifier::Sl, lines);
        lines.push(format!("{timer}(IN := {flip_flop}.Q1, PT := {duration});"));
        terms.push(format!("({flip_flop}.Q1 AND NOT {timer}.Q)"));
    }
}

/// The transitions, in priority order.
fn transitions(chart: &Chart<'_>, lines: &mut Vec<String>) {
    lines.push(
        "// Transitions (IEC 61131-3 6.7.5) in priority order, tested against the steps active before"
            .to_owned(),
    );
    lines.push(
        "// the evolution; one that shares a step with a transition before it clears only if that one does not."
            .to_owned(),
    );
    let names = |steps: &[usize]| {
        steps
            .iter()
            .map(|&step| chart.steps[step].name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    for (index, transition) in chart.transitions.iter().enumerate() {
        let flag = Chart::transition_flag(index);
        let mut notes = Vec::new();
        if !transition.name.is_empty() {
            notes.push(transition.name.clone());
        }
        if let Some(priority) = transition.priority {
            notes.push(format!("priority {}", number_text(priority)));
        }
        let notes = if notes.is_empty() {
            String::new()
        } else {
            format!(" ({})", notes.join(", "))
        };
        lines.push(String::new());
        lines.push(format!(
            "// {flag}: {}",
            line_comment(&format!(
                "{} -> {}{notes}",
                names(&transition.before),
                names(&transition.after)
            ))
        ));
        let mut terms: Vec<String> = transition
            .before
            .iter()
            .map(|&step| chart.steps[step].flag())
            .collect();
        terms.extend(
            transition
                .conditions
                .iter()
                .map(|condition| format!("({condition})")),
        );
        let blockers: Vec<String> = chart.transitions[..index]
            .iter()
            .enumerate()
            .filter(|(_, earlier)| {
                earlier
                    .before
                    .iter()
                    .any(|step| transition.before.contains(step))
            })
            .map(|(earlier, _)| Chart::transition_flag(earlier))
            .collect();
        match blockers.as_slice() {
            [] => {}
            [blocker] => terms.push(format!("NOT {blocker}")),
            blockers => terms.push(format!("NOT ({})", blockers.join(" OR "))),
        }
        lines.push(format!("{flag} := {};", terms.join(" AND ")));
    }
    lines.push(String::new());
}

/// The flags of the transitions for which `pick` holds, in priority order.
fn flags_where(chart: &Chart<'_>, pick: impl Fn(&Transition) -> bool) -> Vec<String> {
    chart
        .transitions
        .iter()
        .enumerate()
        .filter(|(_, transition)| pick(transition))
        .map(|(index, _)| Chart::transition_flag(index))
        .collect()
}

/// The evolution: each step a clearing transition leaves or enters.
fn evolution(chart: &Chart<'_>, lines: &mut Vec<String>) {
    lines.push(
        "// Evolution: the steps before each clearing transition are left, then the steps after it entered."
            .to_owned(),
    );
    for (index, step) in chart.steps.iter().enumerate() {
        let left = flags_where(chart, |transition| transition.before.contains(&index));
        let entered = flags_where(chart, |transition| transition.after.contains(&index));
        let flag = step.flag();
        let left = match left.as_slice() {
            [] => None,
            [single] => Some(single.clone()),
            several => Some(format!("({})", several.join(" OR "))),
        };
        match (left, entered.is_empty()) {
            (None, true) => {}
            (Some(left), true) => lines.push(format!("{flag} := {flag} AND NOT {left};")),
            (None, false) => lines.push(format!("{flag} := {flag} OR {};", entered.join(" OR "))),
            (Some(left), false) => lines.push(format!(
                "{flag} := ({flag} AND NOT {left}) OR {};",
                entered.join(" OR ")
            )),
        }
    }
    lines.push(String::new());
}

#[cfg(test)]
#[path = "generate_tests.rs"]
mod tests;
