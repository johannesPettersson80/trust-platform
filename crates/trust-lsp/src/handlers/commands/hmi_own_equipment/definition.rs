//! The definition My equipment proposes for one of the project's own function blocks
//! (specification 39, §21.9): a pair with the plain device picture and an automatic panel, or, for
//! a block that extends one with a definition, that definition for the block. Written as the
//! files the builder creates when the block is first placed.

use std::fmt::Write;

use trust_runtime::hmi::catalogue::ValueKind;
use trust_runtime::hmi::page_files::{write_pair, HmiPair};

use super::blocks::{Member, OwnBlock, Section};

/// The picture a block has until its user chooses one (§21.9 item 1).
pub(super) const DEVICE: &str = "device";
/// The panel's width and grid, those of the Plant library's panels.
const WIDTH: u32 = 608;
const MARGIN: u32 = 24;
const GAP: u32 = 16;
/// Most cells in one row of each kind.
const STATUSES_PER_ROW: usize = 3;
const VALUES_PER_ROW: usize = 3;
const BUTTONS_PER_ROW: usize = 3;

/// The files proposed for a block, by path below the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Proposal {
    pub(super) pair_path: String,
    pub(super) pair: String,
    /// The automatic panel, none when the block's base definition brings its own.
    pub(super) panel: Option<(String, String)>,
}

/// A name's words: `FB_MixingTank` is FB, Mixing, Tank; `PIDOutput` is PID, Output.
fn words(name: &str) -> Vec<String> {
    let characters: Vec<char> = name.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    for (index, &character) in characters.iter().enumerate() {
        if matches!(character, '_' | ' ' | '-' | '.') {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }
        let before = index.checked_sub(1).map(|at| characters[at]);
        let after = characters.get(index + 1);
        let boundary = !current.is_empty()
            && character.is_ascii_uppercase()
            && before.is_some_and(|before| {
                before.is_ascii_lowercase()
                    || before.is_ascii_digit()
                    || (before.is_ascii_uppercase() && after.is_some_and(char::is_ascii_lowercase))
            });
        if boundary {
            words.push(std::mem::take(&mut current));
        }
        current.push(character);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// What a block's files are named: its name without its namespace, in lower case with hyphens
/// (`FB_Mixer` is `fb-mixer`).
pub(super) fn stem(block: &str) -> String {
    let short = block.rsplit('.').next().unwrap_or(block);
    words(short).join("-").to_ascii_lowercase()
}

/// A name as words, an acronym kept: `FB_MixingTank` is "Mixing tank", `PIDOutput` "PID output".
pub(super) fn title(name: &str) -> String {
    let short = name.rsplit('.').next().unwrap_or(name);
    let short = short.strip_prefix("FB_").unwrap_or(short);
    let acronym = |word: &str| {
        word.len() > 1
            && word
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    };
    words(short)
        .iter()
        .enumerate()
        .map(|(index, word)| {
            if acronym(word) {
                word.clone()
            } else if index == 0 {
                let mut characters = word.chars();
                characters.next().map_or_else(String::new, |first| {
                    first
                        .to_uppercase()
                        .chain(characters.flat_map(char::to_lowercase))
                        .collect()
                })
            } else {
                word.to_lowercase()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn quoted(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

/// The proposal for a block: its base's definition for itself, or the plain device with an
/// automatic panel.
pub(super) fn propose(block: &OwnBlock, base: Option<&HmiPair>) -> Proposal {
    let stem = stem(&block.name);
    let pair_path = format!("hmi/pairs/{stem}.toml");
    if let Some(base) = base {
        let pair = HmiPair {
            title: Some(title(&block.name)),
            block: block.name.clone(),
            ..base.clone()
        };
        return Proposal {
            pair_path,
            pair: write_pair(&pair),
            panel: None,
        };
    }
    let mut pair = format!(
        "format = 1\ntitle = {}\nblock = {}\nfaceplate = {}\nsymbol = {}\n",
        quoted(&title(&block.name)),
        quoted(&block.name),
        quoted(&stem),
        quoted(DEVICE)
    );
    let bound = device_bindings(&block.members);
    if !bound.is_empty() {
        pair.push_str("\n[bind]\n");
        for (property, member) in &bound {
            let _ = writeln!(pair, "{} = {}", quoted(property), quoted(member));
        }
    }
    for member in block
        .members
        .iter()
        .filter(|member| member.setting && single(member))
    {
        let _ = write!(
            pair,
            "\n[[settings]]\nmember = {}\ntitle = {}\n",
            quoted(&member.name),
            quoted(&title(&member.name))
        );
    }
    Proposal {
        pair_path,
        pair,
        panel: Some((format!("hmi/panels/{stem}.toml"), panel(block))),
    }
}

/// The device picture's properties the block has a BOOL member for, by name (`out-of-service`
/// takes `OutOfService`).
fn device_bindings(members: &[Member]) -> Vec<(&'static str, String)> {
    ["running", "fault", "manual", "out-of-service"]
        .into_iter()
        .filter_map(|property| {
            let wanted = property.replace('-', "");
            members
                .iter()
                .find(|member| {
                    member.name.eq_ignore_ascii_case(&wanted)
                        && kind(member) == Some(ValueKind::Boolean)
                })
                .map(|member| (property, member.name.clone()))
        })
        .collect()
}

/// The kind of value a member holds: the first the declarations give, before the generic
/// `value` every single value also is.
fn kind(member: &Member) -> Option<ValueKind> {
    member.kinds.first().copied()
}

/// A member holding one value a field can take.
fn single(member: &Member) -> bool {
    matches!(
        kind(member),
        Some(
            ValueKind::Boolean
                | ValueKind::Number
                | ValueKind::Text
                | ValueKind::Enumeration
                | ValueKind::Time
        )
    )
}

/// A field's kind as panel files write it.
fn kind_word(kind: Option<ValueKind>) -> String {
    kind.and_then(|kind| serde_json::to_value(kind).ok())
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "value".to_string())
}

/// The automatic panel (§21.9 item 2): the block's outputs, BOOL ones as lamps and the others as
/// values, then its commands as buttons; its inputs when it has no outputs. Settings appear in
/// the panel's Settings dialog.
fn panel(block: &OwnBlock) -> String {
    let readable = |section: Section| {
        block
            .members
            .iter()
            .filter(move |member| member.section == section && single(member) && !member.setting)
    };
    let mut shown: Vec<&Member> = readable(Section::Output)
        .chain(readable(Section::Public))
        .collect();
    if shown.is_empty() {
        shown = readable(Section::Input).collect();
    }
    let lamps: Vec<&Member> = shown
        .iter()
        .copied()
        .filter(|member| kind(member) == Some(ValueKind::Boolean))
        .collect();
    let values: Vec<&Member> = shown
        .iter()
        .copied()
        .filter(|member| kind(member) != Some(ValueKind::Boolean))
        .collect();
    let mut objects = String::new();
    let mut y = MARGIN;
    for chunk in lamps.chunks(STATUSES_PER_ROW) {
        let top = y;
        row(
            &mut objects,
            &mut y,
            STATUSES_PER_ROW.min(lamps.len()),
            40,
            &|index, x, width, height| {
                chunk.get(index).map_or(String::new(), |member| {
                let alarm = ["fault", "alarm", "trip", "error"].iter().any(|word| member.name.to_ascii_lowercase().contains(word));
                format!(
                    "\n[[object]]\nid = {}\ntype = \"status\"\nlabel = {}\n{}at = [{x}, {top}]\nsize = [{width}, {height}]\n\n[object.bind]\nstate = {}\n",
                    quoted(&stem(&member.name)),
                    quoted(&title(&member.name)),
                    if alarm { "variant = \"alarm\"\n" } else { "" },
                    quoted(&format!("${}", member.name))
                )
            })
            },
        );
    }
    for chunk in values.chunks(VALUES_PER_ROW) {
        let top = y;
        row(
            &mut objects,
            &mut y,
            VALUES_PER_ROW.min(values.len()),
            88,
            &|index, x, width, height| {
                chunk.get(index).map_or(String::new(), |member| {
                let integer = ["SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT", "BYTE", "WORD", "DWORD", "LWORD"]
                    .contains(&member.type_name.to_ascii_uppercase().as_str());
                let decimals = if kind(member) == Some(ValueKind::Number) { format!("decimals = {}\n", if integer { 0 } else { 2 }) } else { String::new() };
                format!(
                    "\n[[object]]\nid = {}\ntype = \"value\"\nlabel = {}\n{decimals}at = [{x}, {top}]\nsize = [{width}, {height}]\n\n[object.bind]\nvalue = {}\n",
                    quoted(&stem(&member.name)),
                    quoted(&title(&member.name)),
                    quoted(&format!("${}", member.name))
                )
            })
            },
        );
    }
    for chunk in block.commands.chunks(BUTTONS_PER_ROW) {
        let top = y;
        row(
            &mut objects,
            &mut y,
            BUTTONS_PER_ROW.min(block.commands.len()),
            48,
            &|index, x, width, height| {
                chunk.get(index).map_or(String::new(), |command| {
                let stop = command.method.eq_ignore_ascii_case("Stop");
                format!(
                    "\n[[object]]\nid = {}\ntype = \"button\"\ntext = {}\n{}at = [{x}, {top}]\nsize = [{width}, {height}]\n\n[object.command]\ninterface = {}\nmethod = {}\ninstance = \"$instance\"\n",
                    quoted(&format!("{}-command", stem(&command.method))),
                    quoted(&title(&command.method)),
                    if stop { "variant = \"stop\"\n" } else { "" },
                    quoted(&command.interface),
                    quoted(&command.method)
                )
            })
            },
        );
    }
    let height = if objects.is_empty() {
        2 * MARGIN
    } else {
        y - GAP + MARGIN
    };
    let mut text = format!(
        "format = 1\ntitle = {}\nsize = [{WIDTH}, {height}]\npinnable = true\n",
        quoted(&title(&block.name))
    );
    if block
        .members
        .iter()
        .any(|member| member.setting && single(member))
    {
        text.push_str("settings = true\n");
    }
    for member in &shown {
        let _ = write!(
            text,
            "\n[fields.{}]\naccepts = [{}]\n",
            member.name,
            quoted(&kind_word(kind(member)))
        );
    }
    text.push_str(&objects);
    text
}

/// One row of the panel's grid: `count` cells of equal width across the inner width, each drawn
/// by `cell` from its index, place and size; `y` moves below it.
fn row(
    objects: &mut String,
    y: &mut u32,
    count: usize,
    height: u32,
    cell: &dyn Fn(usize, u32, u32, u32) -> String,
) {
    let inner = WIDTH - 2 * MARGIN;
    let width = (inner - GAP * (count as u32 - 1)) / count as u32;
    for index in 0..count {
        objects.push_str(&cell(
            index,
            MARGIN + index as u32 * (width + GAP),
            width,
            height,
        ));
    }
    *y += height + GAP;
}
