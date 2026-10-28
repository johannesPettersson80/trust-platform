//! A symbol's definition, `hmi/symbols/<name>.toml` (specification 39, §14.4 item 2): its title
//! and size, the bindings it takes, its ports and the parts the bindings drive.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use toml::Spanned;

use crate::hmi::catalogue::ValueKind;
use crate::hmi::page_files::{error_at, line_at, PageFileError, SUPPORTED_FORMAT};

use super::{SymbolPart, SymbolPort};

/// The definition as read, before the drawing is known.
pub(super) struct Definition {
    pub(super) title: String,
    pub(super) size: [u32; 2],
    pub(super) properties: BTreeMap<String, ValueKind>,
    pub(super) ports: Vec<(SymbolPort, usize)>,
    pub(super) parts: Vec<(SymbolPart, usize)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDefinition {
    format: Spanned<i64>,
    title: String,
    size: [u32; 2],
    #[serde(default)]
    properties: BTreeMap<String, Spanned<String>>,
    #[serde(default, rename = "port")]
    ports: Vec<RawPort>,
    #[serde(default, rename = "part")]
    parts: Vec<RawPart>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPort {
    name: Spanned<String>,
    at: Spanned<[f64; 2]>,
    direction: Spanned<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPart {
    id: Spanned<String>,
    spin: Option<Spanned<String>>,
    stroke: Option<Spanned<String>>,
    angle: Option<Spanned<String>>,
    level: Option<Spanned<String>>,
    travel: Option<Spanned<String>>,
    show: Option<Spanned<String>>,
    blink: Option<Spanned<String>>,
    alarm: Option<Spanned<String>>,
    active: Option<Spanned<String>>,
    equals: Option<Spanned<String>>,
    range: Option<[f64; 2]>,
    /// For `travel`: how far the part moves at full travel, in the drawing's units.
    #[serde(rename = "move")]
    move_by: Option<Spanned<[f64; 2]>>,
    /// For `equals`: the number the part is shown at.
    value: Option<Spanned<f64>>,
}

/// The ways a part is driven, with the kind of property each takes; `show` takes any.
const DRIVES: [(&str, Option<ValueKind>); 10] = [
    ("spin", Some(ValueKind::Boolean)),
    ("stroke", Some(ValueKind::Boolean)),
    ("angle", Some(ValueKind::Number)),
    ("level", Some(ValueKind::Number)),
    ("travel", Some(ValueKind::Number)),
    ("show", None),
    ("blink", Some(ValueKind::Boolean)),
    ("alarm", Some(ValueKind::Boolean)),
    ("active", Some(ValueKind::Boolean)),
    ("equals", Some(ValueKind::Number)),
];

/// Reads a definition, with each of its problems on its line; a definition that is not TOML of
/// this form is refused. The parts' ids and the ports' places are checked against the drawing
/// afterwards, each on the line kept with it.
pub(super) fn read(
    file: &str,
    text: &str,
) -> Result<(Definition, Vec<PageFileError>), Vec<PageFileError>> {
    let raw: RawDefinition = toml::from_str(text).map_err(|error| {
        let line = error.span().map_or(1, |span| line_at(text, span.start));
        vec![error_at(file, line, error.message().to_string())]
    })?;
    let mut errors = Vec::new();
    let mut report = |offset: usize, message: String| {
        errors.push(error_at(file, line_at(text, offset), message))
    };
    if *raw.format.get_ref() != SUPPORTED_FORMAT {
        report(
            raw.format.span().start,
            format!("format must be {SUPPORTED_FORMAT}"),
        );
    }
    let mut properties = BTreeMap::new();
    for (name, kind) in &raw.properties {
        match kind.get_ref().as_str() {
            "boolean" => properties.insert(name.clone(), ValueKind::Boolean),
            "number" => properties.insert(name.clone(), ValueKind::Number),
            "text" => properties.insert(name.clone(), ValueKind::Text),
            other => {
                report(
                    kind.span().start,
                    format!(
                        "the property \"{name}\" must be boolean, number or text, not \"{other}\""
                    ),
                );
                None
            }
        };
    }
    let mut ports = Vec::new();
    let mut port_names = BTreeSet::new();
    for port in raw.ports {
        let direction = port.direction.get_ref();
        if !matches!(direction.as_str(), "left" | "right" | "up" | "down") {
            report(
                port.direction.span().start,
                format!("direction must be left, right, up or down, not \"{direction}\""),
            );
        }
        if !port_names.insert(port.name.get_ref().clone()) {
            report(
                port.name.span().start,
                format!("the port \"{}\" is named twice", port.name.get_ref()),
            );
        }
        let line = line_at(text, port.at.span().start);
        ports.push((
            SymbolPort {
                name: port.name.into_inner(),
                at: port.at.into_inner(),
                direction: direction.clone(),
            },
            line,
        ));
    }
    let mut parts = Vec::new();
    for part in raw.parts {
        let drives = [
            &part.spin,
            &part.stroke,
            &part.angle,
            &part.level,
            &part.travel,
            &part.show,
            &part.blink,
            &part.alarm,
            &part.active,
            &part.equals,
        ];
        let given: Vec<(usize, &Spanned<String>)> = drives
            .iter()
            .enumerate()
            .filter_map(|(index, drive)| drive.as_ref().map(|drive| (index, drive)))
            .collect();
        match given.as_slice() {
            [(index, property)] => {
                let (way, needs) = DRIVES[*index];
                match properties.get(property.get_ref()) {
                    None if !raw.properties.contains_key(property.get_ref()) => report(
                        property.span().start,
                        format!("the symbol has no property \"{}\"", property.get_ref()),
                    ),
                    Some(kind) if needs.is_some_and(|needs| needs != *kind) => report(
                        property.span().start,
                        format!(
                            "{way} needs a {} property; \"{}\" is a {}",
                            kind_word(needs.unwrap_or(ValueKind::Value)),
                            property.get_ref(),
                            kind_word(*kind)
                        ),
                    ),
                    _ => {}
                }
            }
            [] => report(
                part.id.span().start,
                format!(
                    "the part \"{}\" needs one of spin, stroke, angle, level, travel, show, blink, \
                     alarm, active or equals",
                    part.id.get_ref()
                ),
            ),
            _ => report(
                part.id.span().start,
                format!("the part \"{}\" is driven one way only", part.id.get_ref()),
            ),
        }
        // `equals` shows its part at one number; `travel` moves its part by a displacement.
        match (&part.equals, &part.value) {
            (Some(property), None) => report(
                property.span().start,
                format!(
                    "the part \"{}\" is shown when its number equals value: give value",
                    part.id.get_ref()
                ),
            ),
            (None, Some(value)) => report(
                value.span().start,
                "value belongs to a part shown by equals".to_string(),
            ),
            _ => {}
        }
        match (&part.travel, &part.move_by) {
            (Some(property), None) => report(
                property.span().start,
                format!(
                    "the part \"{}\" travels: give move = [x, y], how far it moves at full travel",
                    part.id.get_ref()
                ),
            ),
            (None, Some(move_by)) => report(
                move_by.span().start,
                "move belongs to a part driven by travel".to_string(),
            ),
            _ => {}
        }
        let line = line_at(text, part.id.span().start);
        let take = |drive: Option<Spanned<String>>| drive.map(Spanned::into_inner);
        parts.push((
            SymbolPart {
                part: part.id.into_inner(),
                spin: take(part.spin),
                stroke: take(part.stroke),
                angle: take(part.angle),
                level: take(part.level),
                travel: take(part.travel),
                show: take(part.show),
                blink: take(part.blink),
                alarm: take(part.alarm),
                active: take(part.active),
                equals: take(part.equals),
                range: part.range,
                move_by: part.move_by.map(Spanned::into_inner),
                value: part.value.map(Spanned::into_inner),
            },
            line,
        ));
    }
    Ok((
        Definition {
            title: raw.title,
            size: raw.size,
            properties,
            ports,
            parts,
        },
        errors,
    ))
}

fn kind_word(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::Boolean => "boolean",
        ValueKind::Number => "number",
        ValueKind::Text => "text",
        _ => "value",
    }
}
