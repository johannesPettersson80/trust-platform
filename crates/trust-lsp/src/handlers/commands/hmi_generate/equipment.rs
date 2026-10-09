//! One instance as Create PLC code writes it (specification 39, §10.2): its declaration, with the
//! initial values chosen for it, and its call, with its hardware signals and the instances it
//! depends on; and the order of the calls.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use trust_runtime::hmi::page_files::{
    HmiCallPosition, HmiDependency, HmiPair, HmiTerminalDirection,
};

use super::super::hmi_authoring::declarations::Declarations;
use super::super::hmi_authoring::equipment_settings::{
    block_default, chosen_literal, duration_nanos, kind_of, Kind,
};
use super::super::hmi_authoring::io;
use super::super::hmi_plant_project::DesignEntry;

/// The instance with one dependency entry per member: of a member's alternatives, one for each
/// interface it can drive (a controller's output drives a position, a power or a speed), the one
/// the chosen instance implements, else the first.
pub(super) fn with_chosen_alternatives(
    index: &Declarations,
    planned: &[Planned],
    item: &Planned,
    design: Option<&DesignEntry>,
) -> Planned {
    let Some(definition) = &item.definition else {
        return item.clone();
    };
    let mut kept: Vec<HmiDependency> = Vec::new();
    for dependency in &definition.dependencies {
        let Some(at) = kept
            .iter()
            .position(|entry| entry.member.eq_ignore_ascii_case(&dependency.member))
        else {
            kept.push(dependency.clone());
            continue;
        };
        let target = design.and_then(|entry| {
            entry
                .dependencies
                .iter()
                .find(|(member, _)| member.eq_ignore_ascii_case(&dependency.member))
                .map(|(_, target)| target.as_str())
        });
        let block = target.and_then(|target| {
            planned
                .iter()
                .find(|other| other.path.eq_ignore_ascii_case(target))
                .map(|other| other.block.as_str())
        });
        if let Some(block) = block {
            if !index.implements_capability(block, &kept[at].interface)
                && index.implements_capability(block, &dependency.interface)
            {
                kept[at] = dependency.clone();
            }
        }
    }
    let mut resolved = item.clone();
    if let Some(definition) = resolved.definition.as_mut() {
        definition.dependencies = kept;
    }
    resolved
}

/// A connected terminal needed only with a condition (`required_when = "HasFeedback"`) means the
/// condition holds: when the block leaves it FALSE and neither the values nor the design set it,
/// the declaration sets it TRUE.
pub(super) fn imply_conditions(
    index: &Declarations,
    planned: &Planned,
    design: Option<&DesignEntry>,
    values: &mut Vec<(String, String)>,
) {
    let (Some(definition), Some(design)) = (&planned.definition, design) else {
        return;
    };
    for terminal in &definition.io {
        let Some(condition) = terminal.required_when.as_deref() else {
            continue;
        };
        let connected = design
            .signals
            .keys()
            .any(|member| member.eq_ignore_ascii_case(&terminal.member));
        let set = values
            .iter()
            .map(|(member, _)| member)
            .chain(design.settings.keys())
            .any(|member| member.eq_ignore_ascii_case(condition));
        if !connected || set {
            continue;
        }
        let off = match block_default(index, &planned.block, condition) {
            Some(text) => text.trim().eq_ignore_ascii_case("FALSE"),
            None => kind_of(index, &planned.block, condition) == Kind::Bool,
        };
        if off {
            values.push((condition.to_string(), "TRUE".to_string()));
        }
    }
}

/// The longest generated line before it is written one entry per line.
const LINE: usize = 100;

/// An instance the regions of one program hold.
#[derive(Debug, Clone)]
pub(super) struct Planned {
    /// Its name in the program (`P101`).
    pub name: String,
    /// Its instance path (`Main.P101`).
    pub path: String,
    /// Its function block type as the declaration writes it.
    pub block: String,
    /// Its component definition, when there is one.
    pub definition: Option<HmiPair>,
}

/// A problem Create PLC code leaves to the user (§10.3 item 3).
pub(super) fn note(instance: &str, message: impl Into<String>) -> Value {
    json!({ "instance": instance, "message": message.into() })
}

/// Whether two ST literals of a member of this kind hold the same value.
fn same_value(kind: Kind, left: &str, right: &str) -> bool {
    let number = |text: &str| {
        let text = text.trim();
        text.rsplit_once('#')
            .map_or(text, |(_, value)| value)
            .replace('_', "")
            .parse::<f64>()
            .ok()
    };
    match kind {
        Kind::Time => {
            duration_nanos(left).is_some() && duration_nanos(left) == duration_nanos(right)
        }
        Kind::Integer | Kind::Real => number(left).is_some() && number(left) == number(right),
        _ => left.trim().eq_ignore_ascii_case(right.trim()),
    }
}

/// The initial values the declaration writes: the definition's initial values and the design's
/// settings, without those equal to the block's own default; in the definition's settings order.
pub(super) fn initial_values(
    index: &Declarations,
    planned: &Planned,
    design: Option<&DesignEntry>,
) -> Vec<(String, String)> {
    let none = BTreeMap::new();
    let initial = planned
        .definition
        .as_ref()
        .and_then(|definition| definition.program.as_ref())
        .map_or(&none, |program| &program.initial);
    let mut members: Vec<String> = Vec::new();
    let mut add = |member: &str| {
        if !members
            .iter()
            .any(|known| known.eq_ignore_ascii_case(member))
        {
            members.push(member.to_string());
        }
    };
    for setting in planned
        .definition
        .iter()
        .flat_map(|definition| &definition.settings)
    {
        add(setting.member.as_str());
    }
    initial.keys().for_each(|member| add(member.as_str()));
    design
        .into_iter()
        .flat_map(|entry| entry.settings.keys())
        .for_each(|member| add(member.as_str()));
    let declared = BTreeMap::new();
    members
        .into_iter()
        .filter_map(|member| {
            let (literal, _) =
                chosen_literal(index, &planned.block, &member, design, &declared, initial)?;
            let kind = kind_of(index, &planned.block, &member);
            let default = block_default(index, &planned.block, &member);
            if default.is_some_and(|default| same_value(kind, &default, &literal)) {
                return None;
            }
            Some((member, literal))
        })
        .collect()
}

/// `entries` joined into one line when it fits, else one entry per line.
pub(super) fn joined(head: &str, entries: &[String], close: &str) -> String {
    let line = format!("    {head}{}{close}\n", entries.join(", "));
    if line.len() <= LINE || entries.len() < 2 {
        return line;
    }
    let mut text = format!("    {head}\n");
    for (position, entry) in entries.iter().enumerate() {
        let separator = if position + 1 == entries.len() {
            close
        } else {
            ","
        };
        text.push_str(&format!("        {entry}{separator}\n"));
    }
    text
}

/// The declaration line of an instance.
pub(super) fn declaration(planned: &Planned, values: &[(String, String)]) -> String {
    if values.is_empty() {
        return format!("    {} : {};\n", planned.name, planned.block);
    }
    let entries: Vec<String> = values
        .iter()
        .map(|(member, value)| format!("{member} := {value}"))
        .collect();
    joined(
        &format!("{} : {} := (", planned.name, planned.block),
        &entries,
        ");",
    )
}

/// A signal or instance path as the program writes it: without the program's own prefix.
fn local(path: &str, program: &str) -> String {
    match path.split_once('.') {
        Some((root, rest)) if root.eq_ignore_ascii_case(program) => rest.to_string(),
        _ => path.to_string(),
    }
}

/// The call of an instance, the assignments of its outputs that drive other instances, and the
/// notes for what is still missing. A loop (§10.5) has its code instead of a call; `interval` is
/// the period of its program's task.
#[allow(clippy::too_many_arguments)]
pub(super) fn call(
    planned: &Planned,
    program: &str,
    design: Option<&DesignEntry>,
    values: &[(String, String)],
    own_addresses: &BTreeSet<String>,
    known_paths: &BTreeSet<String>,
    interval: Option<&str>,
) -> (String, Vec<Value>) {
    let mut parameters = Vec::new();
    let mut drives = Vec::new();
    let mut notes = Vec::new();
    let known: BTreeMap<String, String> = values.iter().cloned().collect();
    let Some(definition) = &planned.definition else {
        return (format!("    {}();\n", planned.name), notes);
    };
    let is_loop = super::loops::is_loop(&planned.block);
    // The loop whose setpoint this loop's output drives: a cascade.
    let mut follows: Option<String> = None;
    for terminal in &definition.io {
        let chosen = design.and_then(|entry| {
            entry
                .signals
                .iter()
                .find(|(member, _)| member.eq_ignore_ascii_case(&terminal.member))
                .map(|(_, signal)| signal.clone())
        });
        let own = format!("{}.{}", planned.path, terminal.member).to_ascii_uppercase();
        match chosen {
            Some(signal) if signal.eq_ignore_ascii_case(&own) => {}
            Some(signal) => {
                let operator = match terminal.direction {
                    HmiTerminalDirection::Input => ":=",
                    HmiTerminalDirection::Output => "=>",
                };
                parameters.push(format!(
                    "{} {operator} {}",
                    terminal.member,
                    local(&signal, program)
                ));
            }
            None if own_addresses.contains(&own) => {}
            None if io::required(terminal, &known) => {
                notes.push(note(
                    &planned.path,
                    format!("{} is not connected to hardware", terminal.title),
                ));
            }
            None => {}
        }
    }
    for dependency in &definition.dependencies {
        // A loop's measurement goes into its code, not into a call.
        if is_loop && dependency.drives.is_none() {
            continue;
        }
        let chosen = design.and_then(|entry| {
            entry
                .dependencies
                .iter()
                .find(|(member, _)| member.eq_ignore_ascii_case(&dependency.member))
                .map(|(_, target)| target.clone())
        });
        let Some(target) = chosen else {
            if dependency.required {
                notes.push(note(
                    &planned.path,
                    format!("choose its {}", dependency.title.to_lowercase()),
                ));
            }
            continue;
        };
        let same_program = target
            .split_once('.')
            .is_some_and(|(root, rest)| root.eq_ignore_ascii_case(program) && !rest.contains('.'));
        if !same_program || !known_paths.contains(&target.to_ascii_uppercase()) {
            notes.push(note(
                &planned.path,
                format!(
                    "{}: {target} is not equipment of program {program}",
                    dependency.title
                ),
            ));
            continue;
        }
        let target = local(&target, program);
        if is_loop
            && dependency
                .drives
                .as_deref()
                .is_some_and(trust_runtime::hmi::page_files::beside_instance)
        {
            follows = Some(target.clone());
        }
        match &dependency.drives {
            // A member beside an instance (`{instance}_Setpoint`, §8.5) is the variable named
            // after it: a loop's output drives `TIC201_Setpoint` from `TIC200_Output`.
            Some(input) => drives.push(format!(
                "    {} := {};\n",
                super::loops::member_of(&target, input),
                super::loops::member_of(&planned.name, &dependency.member)
            )),
            None => parameters.push(format!("{} := {target}", dependency.member)),
        }
    }
    let mut text = if is_loop {
        let (code, missing) = super::loops::code(
            planned,
            program,
            design,
            known_paths,
            interval,
            follows.as_deref(),
        );
        notes.extend(missing);
        code
    } else if parameters.is_empty() {
        format!("    {}();\n", planned.name)
    } else {
        joined(&format!("{}(", planned.name), &parameters, ");")
    };
    drives.into_iter().for_each(|line| text.push_str(&line));
    (text, notes)
}

fn rank(planned: &Planned) -> u8 {
    match planned
        .definition
        .as_ref()
        .and_then(|definition| definition.program.as_ref())
        .and_then(|program| program.position)
    {
        Some(HmiCallPosition::Measurements) => 0,
        Some(HmiCallPosition::Controllers) | None => 1,
        Some(HmiCallPosition::Actuators) => 2,
    }
}

/// The call order: measurements, controllers, actuators, each by name; an instance another one
/// takes as an input is called before it, and one that drives another before that one.
pub(super) fn order(planned: &[Planned], designs: &BTreeMap<String, &DesignEntry>) -> Vec<usize> {
    let position = |path: &str| {
        planned
            .iter()
            .position(|candidate| candidate.path.eq_ignore_ascii_case(path))
    };
    // `before[i]` holds the instances that must be called before instance `i`.
    let mut before: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); planned.len()];
    for (at, item) in planned.iter().enumerate() {
        let (Some(definition), Some(design)) = (
            &item.definition,
            designs.get(&item.path.to_ascii_uppercase()),
        ) else {
            continue;
        };
        for dependency in &definition.dependencies {
            let Some(target) = design
                .dependencies
                .iter()
                .find(|(member, _)| member.eq_ignore_ascii_case(&dependency.member))
                .and_then(|(_, target)| position(target.as_str()))
            else {
                continue;
            };
            if target == at {
                continue;
            }
            if dependency.drives.is_some() {
                before[target].insert(at);
            } else {
                before[at].insert(target);
            }
        }
    }
    let key = |at: usize| {
        (
            rank(&planned[at]),
            planned[at].name.to_ascii_uppercase(),
            at,
        )
    };
    let mut done = vec![false; planned.len()];
    let mut result = Vec::with_capacity(planned.len());
    while result.len() < planned.len() {
        let ready = (0..planned.len())
            .filter(|at| !done[*at] && before[*at].iter().all(|other| done[*other]))
            .min_by_key(|at| key(*at))
            // A cycle of dependencies: the remaining instances keep the plain order.
            .or_else(|| (0..planned.len()).filter(|at| !done[*at]).min_by_key(|at| key(*at)));
        let Some(next) = ready else { break };
        done[next] = true;
        result.push(next);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_runtime::hmi::page_files::parse_pair;

    fn definition(text: &str) -> HmiPair {
        parse_pair("hmi/pairs/test.toml", text).expect("the definition loads")
    }

    fn planned(name: &str, block: &str, definition: Option<HmiPair>) -> Planned {
        Planned {
            name: name.into(),
            path: format!("Main.{name}"),
            block: block.into(),
            definition,
        }
    }

    const PUMP: &str = "format = 1\nblock = \"Plant.FB_Pump\"\nfaceplate = \"plant-run\"\nsymbol = \"pump\"\n\n[[io]]\nmember = \"RunCommand\"\ntitle = \"Run command\"\ndirection = \"output\"\n\n[[io]]\nmember = \"RunningFeedback\"\ntitle = \"Running feedback\"\ndirection = \"input\"\nrequired_when = \"HasFeedback\"\n\n[program]\nposition = \"actuators\"\n";

    #[test]
    fn a_call_connects_the_chosen_signals_and_notes_what_is_missing() {
        let pump = planned("P101", "Plant.FB_Pump", Some(definition(PUMP)));
        let mut design = DesignEntry {
            block: "Plant.FB_Pump".into(),
            ..DesignEntry::default()
        };
        design
            .signals
            .insert("RunCommand".into(), "Main.PumpRun".into());
        let paths = BTreeSet::new();
        let (text, notes) = call(
            &pump,
            "Main",
            Some(&design),
            &[],
            &BTreeSet::new(),
            &paths,
            None,
        );
        assert_eq!(text, "    P101(RunCommand => PumpRun);\n");
        assert_eq!(
            notes.len(),
            1,
            "the feedback is needed while HasFeedback is unknown"
        );
        let values = vec![("HasFeedback".to_string(), "FALSE".to_string())];
        let (_, notes) = call(
            &pump,
            "Main",
            Some(&design),
            &values,
            &BTreeSet::new(),
            &paths,
            None,
        );
        assert!(notes.is_empty(), "no feedback is needed without it");
    }

    #[test]
    fn a_long_declaration_takes_one_line_per_value() {
        let level = planned("LT101", "Plant.FB_LevelInput", None);
        let values: Vec<(String, String)> = [
            ("Unit", "'m'"),
            ("RangeHigh", "10.0"),
            ("HighHighEnabled", "TRUE"),
            ("HighHighLimit", "9.5"),
            ("HighEnabled", "TRUE"),
        ]
        .into_iter()
        .map(|(member, value)| (member.to_string(), value.to_string()))
        .collect();
        assert_eq!(
            declaration(&level, &values),
            "    LT101 : Plant.FB_LevelInput := (\n        Unit := 'm',\n        RangeHigh := 10.0,\n        HighHighEnabled := TRUE,\n        HighHighLimit := 9.5,\n        HighEnabled := TRUE);\n"
        );
        assert_eq!(
            declaration(&level, &values[..1]),
            "    LT101 : Plant.FB_LevelInput := (Unit := 'm');\n"
        );
    }

    #[test]
    fn measurements_come_first_and_a_driving_controller_before_its_actuator() {
        let measurement = definition("format = 1\nblock = \"Plant.FB_LevelInput\"\nfaceplate = \"plant-analog-input\"\nsymbol = \"level-transmitter\"\n\n[program]\nposition = \"measurements\"\n");
        let controller = definition("format = 1\nblock = \"Control.PID\"\nfaceplate = \"plant-pid\"\nsymbol = \"controller\"\n\n[[dependencies]]\nmember = \"{instance}_Output\"\ntitle = \"Valve\"\ninterface = \"Plant.IPositioner\"\ndrives = \"AutoPosition\"\n\n[program]\nposition = \"actuators\"\n");
        let items = vec![
            planned("FV101", "Plant.FB_ControlValve", Some(definition(PUMP))),
            planned("LIC101", "Control.PID", Some(controller)),
            planned("LT101", "Plant.FB_LevelInput", Some(measurement)),
        ];
        let mut loop_design = DesignEntry::default();
        loop_design
            .dependencies
            .insert("{instance}_Output".into(), "Main.FV101".into());
        let designs = BTreeMap::from([("MAIN.LIC101".to_string(), &loop_design)]);
        let names: Vec<&str> = order(&items, &designs)
            .into_iter()
            .map(|at| items[at].name.as_str())
            .collect();
        // LIC101 is an actuator here only to show the dependency rule: it still precedes FV101.
        assert_eq!(names, vec!["LT101", "LIC101", "FV101"]);
    }
}
