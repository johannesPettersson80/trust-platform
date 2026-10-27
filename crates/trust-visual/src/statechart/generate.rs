//! The Structured Text companion of a statechart (specification 25 §11.4, specification 17 §3).
//!
//! The companion is one `FUNCTION_BLOCK FB_<base>_STATECHART`:
//!
//! - the states are numbered in file order (`0 = <first state>`); `_state` holds the active one,
//!   starting at `initial` (or the first state), and `STATE_INDEX`/`STATE_NAME` answer it;
//! - each event of an untimed transition is a `BOOL` input `EV_<EVENT>`;
//! - each transition with an `after` delay (milliseconds, rounded) has its own `TON`, reset every
//!   scan and run while its state is active; it fires on `Q`;
//! - the transitions of the active state are tried in file order (`IF … ELSIF …`), a `guard`
//!   joining the trigger with `AND` (written `=`, `<>`, `AND`, `OR`, `TRUE`, `FALSE` where the
//!   JavaScript-style guard says `==`, `!=`, `&&`, `||`, `true`, `false`);
//! - a taken transition runs the exit actions of its state, its own actions and the entry actions
//!   of its target, then enters the target. An action writes a direct address or variable
//!   (`WRITE_OUTPUT`, `WRITE_VARIABLE`, `SET_MULTIPLE`) or becomes a comment (`LOG`, an unmapped
//!   or unsupported action, an invalid target).
//!
//! Direct addresses are lowered to their globals (`crate::direct_io`).

use std::collections::{HashMap, HashSet};

use crate::direct_io::with_external_globals;
use crate::js;
use crate::js::trim;
use crate::json::{js_round, number_text};
use crate::naming::{
    comment_text, event_input_name, fb_name_for_source, is_assignable_reference, local_name,
    st_string,
};
use crate::{Generated, Json, Problem, VisualKind};

const INDENT: &str = "  ";

/// A transition of a state.
struct Transition<'a> {
    event: String,
    /// The target as the file names it (a truthy value; normally the target state's name).
    target: &'a Json,
    guard: Option<&'a str>,
    actions: Vec<&'a str>,
    /// The `TON` of a timed transition and its rounded delay in milliseconds.
    timer: Option<(String, f64)>,
}

/// The `TON` of a timed transition.
struct Timer {
    name: String,
    after_ms: f64,
    state: String,
    event: String,
}

/// The transitions, event inputs and timers of a chart.
struct Machine<'a> {
    /// The transitions of each state, in state order.
    transitions: Vec<Vec<Transition<'a>>>,
    /// Each event of an untimed transition with its input, in first-use order.
    event_inputs: Vec<(String, String)>,
    timers: Vec<Timer>,
}

/// The companion function block of the statechart `program` saved as `<base>.statechart.json`
/// (specification 17 §3). Without a base name the chart's `id` names the function block.
///
/// # Errors
/// The program is not a statechart ([`super::check_format`]), has no states, or names an action
/// mapping the companion cannot read (no action type, an address or variable that is not text,
/// a `null` target).
pub fn generate(program: &Json, base: &str) -> Result<Generated, Vec<Problem>> {
    super::check_format(program).map_err(|message| vec![Problem::new(message)])?;
    let Some(states_object @ Json::Object(members)) = program.get("states") else {
        return Err(vec![Problem::new(super::FORMAT_MESSAGE)]);
    };
    let states: Vec<(&str, &Json)> = js::ordered(members)
        .into_iter()
        .map(|(name, value)| (name.as_str(), value))
        .collect();
    if states.is_empty() {
        return Err(vec![Problem::new("Statechart has no states.")]);
    }
    let index_of: HashMap<&str, usize> = states
        .iter()
        .enumerate()
        .map(|(index, (name, _))| (*name, index))
        .collect();
    let initial = match program.get("initial") {
        Some(Json::String(name)) if js::truthy(states_object.get(name)) => {
            index_of.get(name.as_str()).copied().unwrap_or(0)
        }
        _ => 0,
    };
    let machine = machine(&states);
    let context = Context {
        states: states_object,
        index_of: &index_of,
        mappings: program.get("actionMappings"),
        event_inputs: &machine.event_inputs,
    };
    let body = context.body(&states, &machine, initial)?;

    let block_name = fb_name_for_source(
        if base.is_empty() {
            program.str_at("id").unwrap_or_default()
        } else {
            base
        },
        VisualKind::Statechart.block_suffix(),
    );
    let mut output = declarations(&block_name, &states, &machine);
    output.extend(body);
    output.push("END_FUNCTION_BLOCK".to_owned());
    let (function_block, globals) = with_external_globals(&output.join("\n"));
    Ok(Generated {
        function_block,
        globals,
        warnings: Vec::new(),
    })
}

/// The transitions of every state, the event inputs and the timers, in file order. A transition
/// without a target is left out; one with a positive `after` gets a timer, the others name an
/// event input (each event once, `EV_<EVENT>` made unique with `_1`, `_2`, …).
fn machine<'a>(states: &[(&'a str, &'a Json)]) -> Machine<'a> {
    let mut machine = Machine {
        transitions: Vec::new(),
        event_inputs: Vec::new(),
        timers: Vec::new(),
    };
    let mut event_names: HashSet<String> = HashSet::new();
    let mut timer_names: HashSet<String> = HashSet::new();
    for (state, value) in states {
        let mut transitions = Vec::new();
        let on = value.get("on").map(js::entries).unwrap_or_default();
        for (index, (event, raw)) in on.into_iter().enumerate() {
            let (target, object) = match raw {
                Json::String(_) => (Some(raw), None),
                other => (other.get("target"), Some(other)),
            };
            let Some(target) = target.filter(|target| js::truthy(Some(target))) else {
                continue;
            };
            let member = |name: &str| object.and_then(|object| object.get(name));
            let after = member("after")
                .and_then(Json::as_finite)
                .filter(|after| *after > 0.0);
            let timer = if let Some(after) = after {
                let name = unique_name(
                    local_name("sc_timer", &format!("{state}_{event}_{index}")),
                    &mut timer_names,
                );
                let after_ms = js_round(after);
                machine.timers.push(Timer {
                    name: name.clone(),
                    after_ms,
                    state: (*state).to_owned(),
                    event: event.clone(),
                });
                Some((name, after_ms))
            } else {
                if !machine.event_inputs.iter().any(|(name, _)| *name == event) {
                    let input = unique_name(event_input_name(&event), &mut event_names);
                    machine.event_inputs.push((event.clone(), input));
                }
                None
            };
            transitions.push(Transition {
                event,
                target,
                guard: member("guard").and_then(Json::as_str),
                actions: action_list(member("actions")),
                timer,
            });
        }
        machine.transitions.push(transitions);
    }
    machine
}

/// The lines of the function block before its body: the header comment, the `FUNCTION_BLOCK`
/// line, the event inputs, the state outputs, the locals with one `TON` per timer, and the state
/// numbering.
fn declarations(block_name: &str, states: &[(&str, &Json)], machine: &Machine) -> Vec<String> {
    let mut output: Vec<String> = [
        "(*",
        "  Auto-generated Statechart companion.",
        "  Source of truth remains the .statechart.json file.",
        "  Generated to support .st-first runtime workflows.",
        "*)",
        "",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    output.push(format!("FUNCTION_BLOCK {block_name}"));
    if !machine.event_inputs.is_empty() {
        output.push("VAR_INPUT".to_owned());
        for (_, input) in &machine.event_inputs {
            output.push(format!("{INDENT}{input} : BOOL;"));
        }
        output.push("END_VAR".to_owned());
    }
    output.extend([
        "VAR_OUTPUT".to_owned(),
        format!("{INDENT}STATE_INDEX : INT;"),
        format!("{INDENT}STATE_NAME : STRING[80];"),
        "END_VAR".to_owned(),
        "VAR".to_owned(),
        format!("{INDENT}_initialized : BOOL := FALSE;"),
        format!("{INDENT}_state : INT := 0;"),
    ]);
    for timer in &machine.timers {
        output.push(format!(
            "{INDENT}{} : TON; (* {} *)",
            timer.name,
            comment_text(&format!("{}.{}", timer.state, timer.event))
        ));
    }
    output.extend([
        "END_VAR".to_owned(),
        String::new(),
        "(* State encoding *)".to_owned(),
    ]);
    for (index, (name, _)) in states.iter().enumerate() {
        output.push(format!("(*{INDENT}{index} = {} *)", comment_text(name)));
    }
    output.push(String::new());
    output
}

/// What a transition's code needs from the whole chart.
struct Context<'a> {
    states: &'a Json,
    index_of: &'a HashMap<&'a str, usize>,
    mappings: Option<&'a Json>,
    event_inputs: &'a [(String, String)],
}

/// Refuses a target written like a direct address that the runtime's address parser refuses
/// (`%QX0.9`): an output the companion silently skipped would never be written.
fn refuse_bad_address(target: &str, action: &str) -> Result<(), Vec<Problem>> {
    match crate::direct_io::refused_address(target) {
        Some(problem) => Err(vec![Problem::new(format!(
            "Action mapping \"{action}\": {problem}."
        ))]),
        None => Ok(()),
    }
}

impl Context<'_> {
    /// The body: start in the `initial` state once, reset every timer, run the transitions of the
    /// active state, then answer the active state.
    fn body(
        &self,
        states: &[(&str, &Json)],
        machine: &Machine,
        initial: usize,
    ) -> Result<Vec<String>, Vec<Problem>> {
        let mut body = vec![
            "IF NOT _initialized THEN".to_owned(),
            format!("{INDENT}_state := {initial};"),
            format!("{INDENT}_initialized := TRUE;"),
            "END_IF;".to_owned(),
            String::new(),
        ];
        for timer in &machine.timers {
            body.push(format!(
                "{}(IN := FALSE, PT := T#{}ms);",
                timer.name,
                number_text(timer.after_ms)
            ));
        }
        if !machine.timers.is_empty() {
            body.push(String::new());
        }
        body.push("CASE _state OF".to_owned());
        for (index, ((_, value), transitions)) in
            states.iter().zip(&machine.transitions).enumerate()
        {
            body.push(format!("{INDENT}{index}:"));
            for transition in transitions {
                if let Some((name, after_ms)) = &transition.timer
                    && *after_ms != 0.0
                {
                    body.push(format!(
                        "{INDENT}{INDENT}{name}(IN := TRUE, PT := T#{}ms);",
                        number_text(*after_ms)
                    ));
                }
            }
            let exit = action_list(value.get("exit"));
            for (position, transition) in transitions.iter().enumerate() {
                self.transition(&mut body, position, transition, &exit)?;
            }
            if !transitions.is_empty() {
                body.push(format!("{INDENT}{INDENT}END_IF;"));
            }
        }
        body.extend([
            "END_CASE;".to_owned(),
            String::new(),
            "STATE_INDEX := _state;".to_owned(),
            "CASE _state OF".to_owned(),
        ]);
        for (index, (name, _)) in states.iter().enumerate() {
            body.push(format!(
                "{INDENT}{index}: STATE_NAME := {};",
                st_string(name)
            ));
        }
        body.extend([
            format!("{INDENT}ELSE"),
            format!("{INDENT}{INDENT}STATE_NAME := 'UNKNOWN';"),
            "END_CASE;".to_owned(),
        ]);
        Ok(body)
    }

    /// The `IF`/`ELSIF` branch of the transition at `position` of its state, whose exit actions
    /// are `exit`.
    fn transition(
        &self,
        body: &mut Vec<String>,
        position: usize,
        transition: &Transition,
        exit: &[&str],
    ) -> Result<(), Vec<Problem>> {
        let trigger = match &transition.timer {
            Some((name, after_ms)) if *after_ms != 0.0 => format!("{name}.Q"),
            _ => self
                .event_inputs
                .iter()
                .find(|(event, _)| *event == transition.event)
                .map_or_else(|| "FALSE".to_owned(), |(_, input)| input.clone()),
        };
        let condition = match transition.guard {
            Some(guard) if !trim(guard).is_empty() => {
                format!("({trigger}) AND ({})", normalize_guard(guard))
            }
            _ => trigger,
        };
        let keyword = if position == 0 { "IF" } else { "ELSIF" };
        body.push(format!("{INDENT}{INDENT}{keyword} {condition} THEN"));
        self.actions(body, exit)?;
        self.actions(body, &transition.actions)?;
        let target_name = js::string(Some(transition.target));
        let target_state = self.states.get(&target_name);
        if js::truthy(target_state) {
            let entry = action_list(target_state.and_then(|state| state.get("entry")));
            self.actions(body, &entry)?;
            let index = match transition.target {
                Json::String(name) => self.index_of.get(name.as_str()).copied().unwrap_or(0),
                _ => 0,
            };
            body.push(format!("{INDENT}{INDENT}{INDENT}_state := {index};"));
        } else {
            body.push(comment(&format!(
                "Invalid transition target: {target_name}"
            )));
        }
        Ok(())
    }

    /// The statements of the actions `names`, in order, three levels deep.
    fn actions(&self, body: &mut Vec<String>, names: &[&str]) -> Result<(), Vec<Problem>> {
        let indent = INDENT.repeat(3);
        for name in names {
            let mapping = self.mappings.and_then(|mappings| mappings.get(name));
            let Some(mapping) = mapping.filter(|mapping| js::truthy(Some(mapping))) else {
                body.push(comment(&format!("Unmapped action: {name}")));
                continue;
            };
            let Some(action) = mapping.get("action").and_then(Json::as_str) else {
                return Err(vec![Problem::new(format!(
                    "The action mapping \"{name}\" has no action type."
                ))]);
            };
            match action.to_uppercase().as_str() {
                kind @ ("WRITE_OUTPUT" | "WRITE_VARIABLE") => {
                    let member = if kind == "WRITE_OUTPUT" {
                        "address"
                    } else {
                        "variable"
                    };
                    let target = text_member(mapping, member, name)?;
                    refuse_bad_address(target, name)?;
                    if is_assignable_reference(target) {
                        body.push(format!(
                            "{indent}{target} := {};",
                            literal(mapping.get("value"))
                        ));
                    } else {
                        body.push(comment(&format!(
                            "Invalid {kind} target for action {name}: {target}"
                        )));
                    }
                }
                "SET_MULTIPLE" => {
                    let targets = mapping
                        .get("targets")
                        .and_then(Json::as_array)
                        .map_or(&[][..], Vec::as_slice);
                    for target in targets {
                        if matches!(target, Json::Null) {
                            return Err(vec![Problem::new(format!(
                                "A target of the action mapping \"{name}\" is null."
                            ))]);
                        }
                        let address = text_member(target, "address", name)?;
                        refuse_bad_address(address, name)?;
                        if is_assignable_reference(address) {
                            body.push(format!(
                                "{indent}{address} := {};",
                                literal(target.get("value"))
                            ));
                        } else {
                            body.push(comment(&format!(
                                "Invalid SET_MULTIPLE target for action {name}: {address}"
                            )));
                        }
                    }
                }
                "LOG" => {
                    let message = match mapping.get("message") {
                        None | Some(Json::Null) => (*name).to_owned(),
                        message => js::string(message),
                    };
                    body.push(comment(&format!("LOG: {message}")));
                }
                _ => body.push(comment(&format!(
                    "Unsupported action type '{action}' for {name}"
                ))),
            }
        }
        Ok(())
    }
}

/// A comment line three levels deep holding `text`.
fn comment(text: &str) -> String {
    format!("{}(* {} *)", INDENT.repeat(3), comment_text(text))
}

/// The text member `name` of an action mapping or target, trimmed; empty when absent.
fn text_member<'a>(object: &'a Json, name: &str, action: &str) -> Result<&'a str, Vec<Problem>> {
    match object.get(name) {
        None | Some(Json::Null) => Ok(""),
        Some(Json::String(text)) => Ok(trim(text)),
        Some(_) => Err(vec![Problem::new(format!(
            "The {name} of the action mapping \"{action}\" must be text."
        ))]),
    }
}

/// The action names of a list (other values name none).
fn action_list(value: Option<&Json>) -> Vec<&str> {
    value
        .and_then(Json::as_array)
        .map(|items| items.iter().filter_map(Json::as_str).collect())
        .unwrap_or_default()
}

/// The Structured Text literal of an action value: Booleans and numbers as such, text as an IEC
/// string (specification 01 §9), other values as the IEC string of their JSON, and `0` for none.
fn literal(value: Option<&Json>) -> String {
    match value {
        Some(Json::Bool(value)) => if *value { "TRUE" } else { "FALSE" }.to_owned(),
        Some(Json::Number(number)) if number.is_finite() => number_text(*number),
        Some(Json::String(text)) => st_string(text),
        None | Some(Json::Null | Json::Number(_)) => "0".to_owned(),
        Some(other) => st_string(&js::normalized(other).to_compact()),
    }
}

/// `base`, or `base_<n>` with the smallest `n` from 1 not in `used`; the name is added to `used`.
fn unique_name(base: String, used: &mut HashSet<String>) -> String {
    if used.insert(base.clone()) {
        return base;
    }
    let mut counter = 1;
    loop {
        let candidate = format!("{base}_{counter}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        counter += 1;
    }
}

/// A JavaScript-style guard in Structured Text, replacing in this order `==` by `=`, `!=` by
/// `<>`, `&&` by ` AND `, `||` by ` OR `, and the words `true` and `false` (any case) by `TRUE`
/// and `FALSE`.
pub(super) fn normalize_guard(guard: &str) -> String {
    let text = guard
        .replace("==", "=")
        .replace("!=", "<>")
        .replace("&&", " AND ")
        .replace("||", " OR ");
    let text = replace_word(&text, "true", "TRUE");
    replace_word(&text, "false", "FALSE")
}

/// `text` with every whole-word occurrence of the ASCII `word` (any case) replaced by `with`.
fn replace_word(text: &str, word: &str, with: &str) -> String {
    let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut at = 0;
    while at + word.len() <= bytes.len() {
        let end = at + word.len();
        if bytes[at..end].eq_ignore_ascii_case(word.as_bytes())
            && (at == 0 || !is_word(bytes[at - 1]))
            && (end == bytes.len() || !is_word(bytes[end]))
        {
            out.push_str(&text[copied..at]);
            out.push_str(with);
            at = end;
            copied = end;
            continue;
        }
        at += 1;
    }
    out.push_str(&text[copied..]);
    out
}
