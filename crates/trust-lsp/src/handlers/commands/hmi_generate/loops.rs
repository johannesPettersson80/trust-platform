//! A PID loop as Create PLC code writes it (specification 39, §10.5): ordinary ST around the
//! Control library's `PID`, run through its Configure, Initialize and Update methods as the
//! control course does, with no block around it. The loop's values are variables beside the PID,
//! named after it (`TIC101_Setpoint`): the HMI's PID component binds them (`{instance}_Setpoint`,
//! §8.5) and Control Studio tunes the one parameter record the code configures the PID from.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use trust_runtime::hmi::page_files::{pair_member_path, HmiScalar, INSTANCE_PLACEHOLDER};

use super::super::hmi_authoring::equipment_settings::{duration_nanos, scalar_literal, Kind};
use super::super::hmi_plant_project::DesignEntry;
use super::equipment::{joined, note, Planned};

/// The block a loop runs.
const LOOP_BLOCK: &str = "Control.PID";

/// The dependency that names the loop's measurement: the variable the measurement goes into.
const MEASUREMENT: &str = "{instance}_Input.ProcessValue";

/// Whether an instance of `block` is a loop Create PLC code writes as ordinary code.
pub(in crate::handlers::commands) fn is_loop(block: &str) -> bool {
    block.eq_ignore_ascii_case(LOOP_BLOCK)
}

/// A variable of a loop beside its PID, named after it with `suffix` (`TIC101_Setpoint`).
struct Variable {
    suffix: &'static str,
    ty: &'static str,
    kind: Kind,
    /// Kept through a restart: what the engineer and the operator set, and the output the loop
    /// starts again from; the rest is rebuilt.
    retained: bool,
}

const VARIABLES: [Variable; 8] = [
    Variable {
        suffix: "_Input",
        ty: "Control.PIDInput",
        kind: Kind::Other,
        retained: false,
    },
    Variable {
        suffix: "_Start",
        ty: "Control.PIDInitialState",
        kind: Kind::Other,
        retained: false,
    },
    Variable {
        suffix: "_Initialized",
        ty: "BOOL",
        kind: Kind::Bool,
        retained: false,
    },
    Variable {
        suffix: "_Parameters",
        ty: "Control.PIDParameters",
        kind: Kind::Other,
        retained: true,
    },
    Variable {
        suffix: "_Manual",
        ty: "BOOL",
        kind: Kind::Bool,
        retained: true,
    },
    Variable {
        suffix: "_Setpoint",
        ty: "LREAL",
        kind: Kind::Real,
        retained: true,
    },
    Variable {
        suffix: "_ManualOutput",
        ty: "LREAL",
        kind: Kind::Real,
        retained: true,
    },
    Variable {
        suffix: "_Output",
        ty: "LREAL",
        kind: Kind::Real,
        retained: true,
    },
];

/// The fields of the PID's parameter record, in its order (specification 32), with their kind.
const PARAMETERS: [(&str, Kind); 12] = [
    ("ProportionalGain", Kind::Real),
    ("IntegralGain", Kind::Real),
    ("DerivativeGain", Kind::Real),
    ("ProportionalSetpointWeight", Kind::Real),
    ("DerivativeSetpointWeight", Kind::Real),
    ("Bias", Kind::Real),
    ("OutputMinimum", Kind::Real),
    ("OutputMaximum", Kind::Real),
    ("RateMinimum", Kind::Real),
    ("RateMaximum", Kind::Real),
    ("TrackingTime", Kind::Real),
    ("AntiWindup", Kind::Other),
];

/// The kind of a member beside a loop's PID (`{instance}_Parameters.ProportionalGain`), for the
/// inspector; `None` when the loop has no such variable.
pub(in crate::handlers::commands) fn beside_kind(member: &str) -> Option<Kind> {
    let rest = member.strip_prefix(INSTANCE_PLACEHOLDER)?;
    let (variable, field) = match rest.split_once('.') {
        Some((variable, field)) => (variable, Some(field)),
        None => (rest, None),
    };
    let variable = VARIABLES
        .iter()
        .find(|candidate| candidate.suffix.eq_ignore_ascii_case(variable))?;
    match field {
        None => Some(variable.kind),
        Some(field) if variable.suffix == "_Parameters" => PARAMETERS
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(field))
            .map(|(_, kind)| *kind),
        Some(_) => None,
    }
}

/// The names of a loop's variables beside its PID (`TIC101_Input` … `TIC101_Output`), which a
/// rename of the loop renames with it.
pub(in crate::handlers::commands) fn loop_variable_names(name: &str) -> Vec<String> {
    VARIABLES
        .iter()
        .map(|variable| format!("{name}{}", variable.suffix))
        .collect()
}

/// Whether a variable of a program's regions belongs to a loop the regions declare: its name is
/// a loop's name followed by one of the loop's suffixes.
pub(super) fn belongs_to_loop(name: &str, loops: &[&str]) -> bool {
    loops.iter().any(|owner| {
        name.get(..owner.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(owner))
            && name.get(owner.len()..).is_some_and(|tail| {
                VARIABLES
                    .iter()
                    .any(|variable| tail.eq_ignore_ascii_case(variable.suffix))
            })
    })
}

/// The values the loop's variables start with: the definition's initial values, then the design's
/// settings, each keyed by its member beside the PID (`{instance}_Setpoint`), as ST literals.
fn starting_values(planned: &Planned, design: Option<&DesignEntry>) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let initial = planned
        .definition
        .as_ref()
        .and_then(|definition| definition.program.as_ref())
        .map(|program| &program.initial);
    for (member, value) in initial.into_iter().flatten() {
        if let Some(kind) = beside_kind(member) {
            values.insert(member.to_ascii_lowercase(), literal(kind, value));
        }
    }
    for (member, value) in design.into_iter().flat_map(|entry| &entry.settings) {
        if beside_kind(member).is_some() {
            values.insert(member.to_ascii_lowercase(), value.clone());
        }
    }
    values
}

fn literal(kind: Kind, value: &HmiScalar) -> String {
    scalar_literal(kind, value)
}

/// The loop's declarations: the PID and the variables rebuilt after a restart, then those kept
/// through one, for the program's `VAR` and `VAR RETAIN` regions.
pub(super) fn declarations(planned: &Planned, design: Option<&DesignEntry>) -> (String, String) {
    let name = &planned.name;
    let values = starting_values(planned, design);
    let value = |member: String| values.get(&member.to_ascii_lowercase()).cloned();
    let mut plain = format!("    {name} : {};\n", planned.block);
    let mut retained = String::new();
    for variable in &VARIABLES {
        let own = format!("{name}{}", variable.suffix);
        let member = format!("{INSTANCE_PLACEHOLDER}{}", variable.suffix);
        let text = if variable.suffix == "_Parameters" {
            let entries: Vec<String> = PARAMETERS
                .iter()
                .filter_map(|(field, _)| {
                    value(format!("{member}.{field}"))
                        .map(|literal| format!("{field} := {literal}"))
                })
                .collect();
            if entries.is_empty() {
                format!("    {own} : {};\n", variable.ty)
            } else {
                joined(&format!("{own} : {} := (", variable.ty), &entries, ");")
            }
        } else {
            match value(member) {
                Some(literal) => format!("    {own} : {} := {literal};\n", variable.ty),
                None => format!("    {own} : {};\n", variable.ty),
            }
        };
        if variable.retained {
            retained.push_str(&text);
        } else {
            plain.push_str(&text);
        }
    }
    (plain, retained)
}

/// The task period in seconds as an ST literal (`T#100ms` is `0.1`), 0.1 when it is not known.
pub(super) fn cycle_seconds(interval: Option<&str>) -> String {
    let seconds = interval
        .and_then(duration_nanos)
        .filter(|nanos| *nanos > 0)
        .map_or(0.1, |nanos| nanos as f64 / 1e9);
    let text = format!("{seconds}");
    if text.contains('.') || text.contains('e') {
        text
    } else {
        format!("{text}.0")
    }
}

/// The local name of the instance a dependency of the loop names, when it is equipment of the
/// same program the regions hold.
fn chosen(
    planned: &Planned,
    design: Option<&DesignEntry>,
    member: &str,
    program: &str,
    known_paths: &BTreeSet<String>,
) -> Option<Result<String, String>> {
    let target = design?
        .dependencies
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(member))?
        .1;
    let local = target
        .split_once('.')
        .filter(|(root, rest)| root.eq_ignore_ascii_case(program) && !rest.contains('.'))
        .map(|(_, rest)| rest.to_string());
    Some(match local {
        Some(local) if known_paths.contains(&target.to_ascii_uppercase()) => Ok(local),
        _ => Err(format!(
            "{}: {target} is not equipment of program {program}",
            planned
                .definition
                .iter()
                .flat_map(|definition| &definition.dependencies)
                .find(|dependency| dependency.member.eq_ignore_ascii_case(member))
                .map_or("Measurement", |dependency| dependency.title.as_str())
        )),
    })
}

/// The loop's code for one scan, without the assignment of its output to the equipment it drives
/// (written after it as for any block, §10.2): the measurement goes in while it is good; a bad one
/// holds the output in Manual and the loop starts again, without a bump, from the next good one;
/// in Auto the manual output follows the output, so Manual takes over where the loop is.
/// `follows` names a loop whose setpoint this loop's output drives (a cascade): while that loop is
/// in Manual this one tracks its measurement, so closing the cascade is bumpless too.
pub(super) fn code(
    planned: &Planned,
    program: &str,
    design: Option<&DesignEntry>,
    known_paths: &BTreeSet<String>,
    interval: Option<&str>,
    follows: Option<&str>,
) -> (String, Vec<Value>) {
    let n = &planned.name;
    let mut notes = Vec::new();
    let measurement = match chosen(planned, design, MEASUREMENT, program, known_paths) {
        Some(Ok(local)) => Some(local),
        Some(Err(message)) => {
            notes.push(note(&planned.path, message));
            None
        }
        None => {
            notes.push(note(&planned.path, "choose its measurement"));
            None
        }
    };
    let cycle = cycle_seconds(interval);
    let mut text = String::new();
    let mut line = |indent: usize, content: String| {
        text.push_str(&" ".repeat(4 * indent));
        text.push_str(&content);
        text.push('\n');
    };
    // The comments name nothing a rename could change, so a renamed loop's region stays what the
    // generator writes (§10.3 item 1).
    match &measurement {
        Some(_) => line(
            1,
            "(* A PID loop: Control.PID holds its measurement at its setpoint. *)".into(),
        ),
        None => line(
            1,
            "(* A PID loop without a measurement yet: it stays in Manual. *)".into(),
        ),
    }
    line(1, format!("{n}.Configure(Parameters := {n}_Parameters);"));
    line(1, format!("{n}_Input.Setpoint := {n}_Setpoint;"));
    line(1, format!("{n}_Input.ManualOutput := {n}_ManualOutput;"));
    line(1, format!("{n}_Input.CycleTime := {cycle};"));
    match &measurement {
        Some(measured) => {
            line(
                1,
                format!("IF {measured}.GetQuality() = E_Quality#Good THEN"),
            );
            line(
                2,
                format!("{n}_Input.ProcessValue := {measured}.GetValue();"),
            );
            line(2, format!("IF NOT {n}_Initialized THEN"));
            line(
                3,
                "(* It starts from the measurement and the output as they are: no bump. *)".into(),
            );
            line(3, format!("{n}_Start.Setpoint := {n}_Setpoint;"));
            line(
                3,
                format!("{n}_Start.ProcessValue := {n}_Input.ProcessValue;"),
            );
            line(3, format!("{n}_Start.NominalOutput := {n}_Output;"));
            line(3, format!("{n}_Start.SaturatedOutput := {n}_Output;"));
            line(3, format!("{n}.Initialize(State := {n}_Start);"));
            line(3, format!("{n}_Initialized := TRUE;"));
            line(2, "END_IF;".into());
            line(2, format!("IF {n}_Manual THEN"));
            line(3, format!("{n}_Input.Mode := PIDMode#Manual;"));
            line(2, "ELSE".into());
            line(3, format!("{n}_Input.Mode := PIDMode#Auto;"));
            line(2, "END_IF;".into());
            line(1, "ELSE".into());
            line(2, "(* A bad measurement holds the output; the loop starts again from the next good one. *)".into());
            line(2, format!("{n}_Input.Mode := PIDMode#Manual;"));
            line(2, format!("{n}_Initialized := FALSE;"));
            line(1, "END_IF;".into());
        }
        None => line(1, format!("{n}_Input.Mode := PIDMode#Manual;")),
    }
    if let Some(inner) = follows {
        line(1, format!("IF {inner}_Manual THEN"));
        line(2, "(* While the inner loop is in Manual this one follows its measurement: closing the cascade is bumpless. *)".into());
        line(2, format!("{n}_Input.Mode := PIDMode#Track;"));
        line(
            2,
            format!("{n}_Input.TrackingOutput := {inner}_Input.ProcessValue;"),
        );
        line(1, "END_IF;".into());
    }
    line(1, format!("{n}_Output := {n}.Update(Input := {n}_Input);"));
    line(1, format!("IF {n}_Input.Mode = PIDMode#Auto THEN"));
    line(
        2,
        "(* Manual takes over from the output as it is. *)".into(),
    );
    line(2, format!("{n}_ManualOutput := {n}_Output;"));
    line(1, "END_IF;".into());
    (text, notes)
}

/// A number written as an ST literal (`100.0`, `LREAL#-5`, `1_000.0`).
pub(super) fn number(literal: &str) -> Option<f64> {
    let text = literal.trim();
    let text = text.rsplit_once('#').map_or(text, |(_, value)| value);
    text.replace('_', "")
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

/// The write rules the runtime display needs to operate the loop (§12.2): the operator switches
/// between Manual and Auto, sets the setpoint within the measurement's range and the output within
/// the loop's output limits. `program` is the program instance (`Main`).
pub(super) fn write_rules(
    program: &str,
    planned: &Planned,
    design: Option<&DesignEntry>,
    measurement_range: Option<(f64, f64)>,
) -> Vec<Value> {
    let values = starting_values(planned, design);
    let limit = |field: &str| {
        values
            .get(&format!("{INSTANCE_PLACEHOLDER}_Parameters.{field}").to_ascii_lowercase())
            .and_then(|literal| number(literal))
    };
    let path = |suffix: &str| format!("{program}.{}{suffix}", planned.name);
    let ranged = |suffix: &str, range: Option<(f64, f64)>| {
        let mut rule = serde_json::json!({ "path": path(suffix), "role": "operator" });
        if let Some((low, high)) = range.filter(|(low, high)| low < high) {
            rule["min"] = serde_json::json!(low);
            rule["max"] = serde_json::json!(high);
        }
        rule
    };
    let output = limit("OutputMinimum").zip(limit("OutputMaximum"));
    vec![
        serde_json::json!({ "path": path("_Manual"), "role": "operator" }),
        ranged("_Setpoint", measurement_range),
        ranged("_ManualOutput", output),
    ]
}

/// The design's choice for the loop's measurement, the instance path it names.
pub(super) fn measurement_of(design: Option<&DesignEntry>) -> Option<&str> {
    design?
        .dependencies
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(MEASUREMENT))
        .map(|(_, target)| target.as_str())
}

/// The path of a member beside a loop's PID, or of any member of an instance.
pub(super) fn member_of(instance: &str, member: &str) -> String {
    pair_member_path(instance, member)
}

/// The namespaces the loop's code names without their prefix (`PIDMode#Auto`, `E_Quality#Good`).
pub(super) const LOOP_NAMESPACES: [&str; 2] = ["Control", "Plant"];
