//! Component definitions, written `hmi/pairs/<name>.toml` (specification 39, §8.5): the function
//! block a component shows, its panel and symbol, the members of the panel's fields and the
//! symbol's properties, its settings' metadata, its hardware terminals, the instances it depends
//! on, and its rules for Create PLC code. One parser serves the runtime, the language server and
//! `hmi check`.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use toml::Spanned;

use super::load::{check_format, error_at, line_at, toml_error, PageFileError};
use super::types::HmiScalar;
use super::write::{entry, key, number, string};
use super::SUPPORTED_FORMAT;

/// Where a component's call goes in the target program's body (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiCallPosition {
    /// Among the measurements, first.
    Measurements,
    /// Among the controllers, after the measurements.
    Controllers,
    /// Among the actuators, last.
    Actuators,
}

/// A component's rules for Create PLC code (§10.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HmiPairProgram {
    /// Where its call goes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<HmiCallPosition>,
    /// Instance initial values of members, by member.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub initial: BTreeMap<String, HmiScalar>,
    /// The task period range it needs, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle: Option<[u32; 2]>,
}

/// One allowed value of a setting, with the words the dialog shows for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HmiChoice {
    /// The value: a boolean, a number, or a text such as `T#1h` or an enumeration value.
    pub value: HmiScalar,
    /// Its words.
    pub title: String,
}

/// A setting's metadata (Plant library specification §10): the public RETAIN member it describes,
/// its plain title and unit, and its limits or choices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HmiSetting {
    /// The member, relative to the instance (`StartTimeout`, `Outer.ProportionalGain`).
    pub member: String,
    /// Its title in plain words.
    pub title: String,
    /// Its unit; `$Member` takes the unit from that member of the instance. For a `TIME` member it
    /// is the unit the dialog edits in: `ms`, `s`, `min` or `h`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The smallest value, in its unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// The largest value, in its unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// The step of a value control, in its unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// The allowed values, when there is a short list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<HmiChoice>,
    /// Shown in the builder's inspector (§21.3 item 2).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub key: bool,
    /// One line of help under the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

/// The direction of a hardware terminal, seen from the PLC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiTerminalDirection {
    /// A signal the block reads.
    Input,
    /// A signal the block writes.
    Output,
}

/// A hardware terminal of a component (§8.1 of the catalogue, §21.3 item 2): an input or output of
/// the block that Connect hardware connects to a PLC signal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HmiTerminal {
    /// The block's input or output.
    pub member: String,
    /// Its title.
    pub title: String,
    /// Read or written by the block.
    pub direction: HmiTerminalDirection,
    /// Needed for the component to be connected.
    pub required: bool,
    /// Needed only while this BOOL input of the block is TRUE (`HasFeedback`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_when: Option<String>,
}

/// An instance a component depends on (§10.2): an interface input of the block that takes another
/// instance (`Measurement`, `Member1`), or with `drives` an output of the block that is assigned to
/// that input of the chosen instance (`Output` drives `AutoPosition`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HmiDependency {
    /// The block's interface input, or its output with `drives`.
    pub member: String,
    /// Its title.
    pub title: String,
    /// The interface the chosen instance implements, with its namespace.
    pub interface: String,
    /// Needed before Create PLC code writes the call.
    pub required: bool,
    /// The chosen instance's input this block's `member` output is assigned to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drives: Option<String>,
}

/// A component definition file (§8.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HmiPair {
    /// Its name in the palette.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The function block type, with its namespace.
    pub block: String,
    /// The panel it opens.
    pub faceplate: String,
    /// The component type that shows the equipment on a page.
    pub symbol: String,
    /// The symbol variants it may take.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<String>,
    /// The members of the panel's fields whose member has another name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
    /// The member each property of the symbol shows, where it is not the property's own name in
    /// words (`out-of-service` shows `OutOfService`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bind: BTreeMap<String, String>,
    /// The quality member of a symbol property, by property.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub quality: BTreeMap<String, String>,
    /// The settings' metadata, in dialog order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settings: Vec<HmiSetting>,
    /// The hardware terminals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub io: Vec<HmiTerminal>,
    /// The instances it depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<HmiDependency>,
    /// Its rules for Create PLC code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<HmiPairProgram>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPair {
    #[allow(dead_code)]
    format: i64,
    #[serde(default)]
    title: Option<String>,
    block: String,
    faceplate: String,
    symbol: String,
    #[serde(default)]
    variants: Vec<String>,
    #[serde(default)]
    fields: BTreeMap<String, String>,
    #[serde(default)]
    bind: BTreeMap<String, String>,
    #[serde(default)]
    quality: BTreeMap<String, String>,
    #[serde(default)]
    settings: Vec<Spanned<RawSetting>>,
    #[serde(default)]
    io: Vec<Spanned<RawTerminal>>,
    #[serde(default)]
    dependencies: Vec<Spanned<RawDependency>>,
    #[serde(default)]
    program: Option<RawProgram>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSetting {
    member: String,
    title: String,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
    #[serde(default)]
    step: Option<f64>,
    #[serde(default)]
    choices: Vec<RawChoice>,
    #[serde(default)]
    key: bool,
    #[serde(default)]
    help: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawChoice {
    value: toml::Value,
    title: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTerminal {
    member: String,
    title: String,
    direction: String,
    #[serde(default)]
    required: Option<bool>,
    #[serde(default)]
    required_when: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDependency {
    member: String,
    title: String,
    interface: String,
    #[serde(default)]
    required: Option<bool>,
    #[serde(default)]
    drives: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProgram {
    #[serde(default)]
    position: Option<Spanned<String>>,
    #[serde(default)]
    initial: BTreeMap<String, Spanned<toml::Value>>,
    #[serde(default)]
    cycle: Option<Spanned<[u32; 2]>>,
}

/// Parses and validates one component definition file.
pub fn parse_pair(file: &str, text: &str) -> Result<HmiPair, Vec<PageFileError>> {
    check_format(file, text)?;
    let raw: RawPair =
        toml::from_str(text).map_err(|error| vec![toml_error(file, text, &error)])?;
    let mut errors = Vec::new();
    let settings = check_settings(file, text, raw.settings, &mut errors);
    let io = check_terminals(file, text, raw.io, &mut errors);
    let dependencies = check_dependencies(file, text, raw.dependencies, &mut errors);
    let program = match raw.program {
        None => None,
        Some(program) => match check_program(file, text, program) {
            Ok(program) => Some(program),
            Err(found) => {
                errors.extend(found);
                None
            }
        },
    };
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(HmiPair {
        title: raw.title,
        block: raw.block,
        faceplate: raw.faceplate,
        symbol: raw.symbol,
        variants: raw.variants,
        fields: raw.fields,
        bind: raw.bind,
        quality: raw.quality,
        settings,
        io,
        dependencies,
        program,
    })
}

/// What a pair member starts with to name a variable beside the instance, named after it (§8.5):
/// `{instance}_Setpoint` of `Main.TIC101` is `Main.TIC101_Setpoint`. A loop Create PLC code writes
/// as ordinary code keeps its values in such variables (§10.2).
pub const INSTANCE_PLACEHOLDER: &str = "{instance}";

/// Whether a pair member names a variable beside the instance (`{instance}_Setpoint`), not one of
/// its members.
pub fn beside_instance(member: &str) -> bool {
    member.starts_with(INSTANCE_PLACEHOLDER)
}

/// The path of a pair member of an instance: `Running` of `Main.P101` is `Main.P101.Running`,
/// `{instance}_Setpoint` of `Main.TIC101` is `Main.TIC101_Setpoint`.
pub fn pair_member_path(instance: &str, member: &str) -> String {
    match member.strip_prefix(INSTANCE_PLACEHOLDER) {
        Some(rest) => format!("{instance}{rest}"),
        None => format!("{instance}.{member}"),
    }
}

/// A member path below an instance: identifiers joined by dots, an index allowed; or a variable
/// beside it, `{instance}` followed by the rest of its name and its own member path.
pub fn is_pair_member(text: &str) -> bool {
    is_member(text)
}

fn is_member(text: &str) -> bool {
    let path = match text.strip_prefix(INSTANCE_PLACEHOLDER) {
        Some(rest) if rest.starts_with(|c: char| c == '_' || c.is_ascii_alphanumeric()) => {
            format!("Instance{rest}")
        }
        Some(_) => return false,
        None => format!("Instance.{text}"),
    };
    crate::hmi::member_paths::MemberPath::parse(&path).is_ok()
}

fn check_settings(
    file: &str,
    text: &str,
    raw: Vec<Spanned<RawSetting>>,
    errors: &mut Vec<PageFileError>,
) -> Vec<HmiSetting> {
    let mut seen = BTreeSet::new();
    let mut settings = Vec::new();
    for entry in raw {
        let line = line_at(text, entry.span().start);
        let setting = entry.into_inner();
        let mut fail = |message: String| errors.push(error_at(file, line, message));
        if !is_member(&setting.member) {
            fail(format!("`{}` is not a member of the block", setting.member));
            continue;
        }
        if !seen.insert(setting.member.to_ascii_lowercase()) {
            fail(format!(
                "the setting `{}` is described twice",
                setting.member
            ));
            continue;
        }
        if setting.title.trim().is_empty() {
            fail(format!("the setting `{}` needs a title", setting.member));
        }
        if let (Some(min), Some(max)) = (setting.min, setting.max) {
            if min > max {
                fail(format!(
                    "the setting `{}` has a minimum above its maximum",
                    setting.member
                ));
            }
        }
        if setting.step.is_some_and(|step| step <= 0.0) {
            fail(format!(
                "the step of `{}` is a positive number",
                setting.member
            ));
        }
        let mut choices = Vec::new();
        for choice in setting.choices {
            match scalar(choice.value) {
                Some(value) => choices.push(HmiChoice {
                    value,
                    title: choice.title,
                }),
                None => fail(format!(
                    "a choice of `{}` is a boolean, a number or a text",
                    setting.member
                )),
            }
        }
        settings.push(HmiSetting {
            member: setting.member,
            title: setting.title,
            unit: setting.unit,
            min: setting.min,
            max: setting.max,
            step: setting.step,
            choices,
            key: setting.key,
            help: setting.help,
        });
    }
    settings
}

fn check_terminals(
    file: &str,
    text: &str,
    raw: Vec<Spanned<RawTerminal>>,
    errors: &mut Vec<PageFileError>,
) -> Vec<HmiTerminal> {
    let mut terminals = Vec::new();
    for entry in raw {
        let line = line_at(text, entry.span().start);
        let terminal = entry.into_inner();
        let direction = match terminal.direction.as_str() {
            "input" => HmiTerminalDirection::Input,
            "output" => HmiTerminalDirection::Output,
            other => {
                errors.push(error_at(
                    file,
                    line,
                    format!("direction is `input` or `output`, not `{other}`"),
                ));
                continue;
            }
        };
        if terminal.required.is_some() && terminal.required_when.is_some() {
            errors.push(error_at(
                file,
                line,
                format!(
                    "`{}` has `required` or `required_when`, not both",
                    terminal.member
                ),
            ));
            continue;
        }
        terminals.push(HmiTerminal {
            member: terminal.member,
            title: terminal.title,
            direction,
            required: terminal
                .required
                .unwrap_or(terminal.required_when.is_none()),
            required_when: terminal.required_when,
        });
    }
    terminals
}

fn check_dependencies(
    file: &str,
    text: &str,
    raw: Vec<Spanned<RawDependency>>,
    errors: &mut Vec<PageFileError>,
) -> Vec<HmiDependency> {
    let mut dependencies = Vec::new();
    for entry in raw {
        let line = line_at(text, entry.span().start);
        let dependency = entry.into_inner();
        if dependency.interface.trim().is_empty() || dependency.interface.contains(' ') {
            errors.push(error_at(
                file,
                line,
                format!(
                    "`{}` names the interface the chosen instance implements",
                    dependency.member
                ),
            ));
            continue;
        }
        dependencies.push(HmiDependency {
            member: dependency.member,
            title: dependency.title,
            interface: dependency.interface,
            required: dependency.required.unwrap_or(true),
            drives: dependency.drives,
        });
    }
    dependencies
}

fn scalar(value: toml::Value) -> Option<HmiScalar> {
    match value {
        toml::Value::Boolean(flag) => Some(HmiScalar::Bool(flag)),
        toml::Value::Integer(whole) => Some(HmiScalar::Number(whole as f64)),
        toml::Value::Float(fraction) => Some(HmiScalar::Number(fraction)),
        toml::Value::String(words) => Some(HmiScalar::Text(words)),
        _ => None,
    }
}

fn check_program(
    file: &str,
    text: &str,
    raw: RawProgram,
) -> Result<HmiPairProgram, Vec<PageFileError>> {
    let position = match raw.position {
        None => None,
        Some(position) => Some(match position.get_ref().as_str() {
            "measurements" => HmiCallPosition::Measurements,
            "controllers" => HmiCallPosition::Controllers,
            "actuators" => HmiCallPosition::Actuators,
            other => {
                return Err(vec![error_at(
                    file,
                    line_at(text, position.span().start),
                    format!(
                        "position is `measurements`, `controllers` or `actuators`, not `{other}`"
                    ),
                )])
            }
        }),
    };
    if let Some(cycle) = &raw.cycle {
        let [shortest, longest] = *cycle.get_ref();
        if shortest > longest {
            return Err(vec![error_at(
                file,
                line_at(text, cycle.span().start),
                format!(
                    "the shortest cycle is longer than the longest: {shortest} ms > {longest} ms"
                ),
            )]);
        }
    }
    let mut initial = BTreeMap::new();
    for (member, value) in raw.initial {
        let line = line_at(text, value.span().start);
        let Some(scalar) = scalar(value.into_inner()) else {
            return Err(vec![error_at(
                file,
                line,
                format!("the initial value of `{member}` is a boolean, a number or a text"),
            )]);
        };
        initial.insert(member, scalar);
    }
    Ok(HmiPairProgram {
        position,
        initial,
        cycle: raw.cycle.map(Spanned::into_inner),
    })
}

fn written(value: &HmiScalar) -> String {
    match value {
        HmiScalar::Bool(flag) => flag.to_string(),
        HmiScalar::Number(value) => number(*value),
        HmiScalar::Text(words) => string(words),
    }
}

/// Writes a component definition in canonical TOML, in the order of §8.5.
pub fn write_pair(pair: &HmiPair) -> String {
    let mut out = String::new();
    entry(&mut out, "format", &SUPPORTED_FORMAT.to_string());
    if let Some(title) = &pair.title {
        entry(&mut out, "title", &string(title));
    }
    entry(&mut out, "block", &string(&pair.block));
    entry(&mut out, "faceplate", &string(&pair.faceplate));
    entry(&mut out, "symbol", &string(&pair.symbol));
    if !pair.variants.is_empty() {
        let variants: Vec<String> = pair
            .variants
            .iter()
            .map(|variant| string(variant))
            .collect();
        entry(&mut out, "variants", &format!("[{}]", variants.join(", ")));
    }
    for (table, entries) in [
        ("fields", &pair.fields),
        ("bind", &pair.bind),
        ("quality", &pair.quality),
    ] {
        if !entries.is_empty() {
            out.push_str(&format!("\n[{table}]\n"));
            for (name, member) in entries {
                entry(&mut out, &key(name), &string(member));
            }
        }
    }
    for setting in &pair.settings {
        out.push_str("\n[[settings]]\n");
        entry(&mut out, "member", &string(&setting.member));
        entry(&mut out, "title", &string(&setting.title));
        if let Some(unit) = &setting.unit {
            entry(&mut out, "unit", &string(unit));
        }
        for (name, value) in [
            ("min", setting.min),
            ("max", setting.max),
            ("step", setting.step),
        ] {
            if let Some(value) = value {
                entry(&mut out, name, &number(value));
            }
        }
        if !setting.choices.is_empty() {
            let choices: Vec<String> = setting
                .choices
                .iter()
                .map(|choice| {
                    format!(
                        "{{ value = {}, title = {} }}",
                        written(&choice.value),
                        string(&choice.title)
                    )
                })
                .collect();
            entry(&mut out, "choices", &format!("[{}]", choices.join(", ")));
        }
        if setting.key {
            entry(&mut out, "key", "true");
        }
        if let Some(help) = &setting.help {
            entry(&mut out, "help", &string(help));
        }
    }
    for terminal in &pair.io {
        out.push_str("\n[[io]]\n");
        entry(&mut out, "member", &string(&terminal.member));
        entry(&mut out, "title", &string(&terminal.title));
        let direction = match terminal.direction {
            HmiTerminalDirection::Input => "input",
            HmiTerminalDirection::Output => "output",
        };
        entry(&mut out, "direction", &string(direction));
        match &terminal.required_when {
            Some(condition) => entry(&mut out, "required_when", &string(condition)),
            None if !terminal.required => entry(&mut out, "required", "false"),
            None => {}
        }
    }
    for dependency in &pair.dependencies {
        out.push_str("\n[[dependencies]]\n");
        entry(&mut out, "member", &string(&dependency.member));
        entry(&mut out, "title", &string(&dependency.title));
        entry(&mut out, "interface", &string(&dependency.interface));
        if !dependency.required {
            entry(&mut out, "required", "false");
        }
        if let Some(drives) = &dependency.drives {
            entry(&mut out, "drives", &string(drives));
        }
    }
    if let Some(program) = &pair.program {
        out.push_str("\n[program]\n");
        if let Some(position) = program.position {
            let name = match position {
                HmiCallPosition::Measurements => "measurements",
                HmiCallPosition::Controllers => "controllers",
                HmiCallPosition::Actuators => "actuators",
            };
            entry(&mut out, "position", &string(name));
        }
        if let Some([shortest, longest]) = program.cycle {
            entry(&mut out, "cycle", &format!("[{shortest}, {longest}]"));
        }
        if !program.initial.is_empty() {
            out.push_str("\n[program.initial]\n");
            for (member, value) in &program.initial {
                entry(&mut out, &key(member), &written(value));
            }
        }
    }
    out
}
