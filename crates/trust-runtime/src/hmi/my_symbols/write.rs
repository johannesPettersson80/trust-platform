//! Canonical symbol definitions for the visual editor (specification 39 §14.4.7).

use std::fmt::Write;

use super::HmiSymbol;
use crate::hmi::catalogue::ValueKind;

fn quoted(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

/// Writes only the definition. The SVG artwork stays in its own unchanged file.
/// Callers validate the result with `parse_symbol` and the original drawing before applying it.
pub fn write_symbol(symbol: &HmiSymbol) -> String {
    let mut out = format!(
        "format = 1\ntitle = {}\nsize = [{}, {}]\n",
        quoted(&symbol.title),
        symbol.size[0],
        symbol.size[1]
    );
    if !symbol.properties.is_empty() {
        out.push_str("\n[properties]\n");
        for (name, kind) in &symbol.properties {
            let kind = match kind {
                ValueKind::Boolean => "boolean",
                ValueKind::Number => "number",
                ValueKind::Text => "text",
                _ => "unsupported",
            };
            let _ = writeln!(out, "{} = {}", quoted(name), quoted(kind));
        }
    }
    for port in &symbol.ports {
        let _ = write!(
            out,
            "\n[[port]]\nname = {}\nat = [{}, {}]\ndirection = {}\n",
            quoted(&port.name),
            port.at[0],
            port.at[1],
            quoted(&port.direction)
        );
    }
    for part in &symbol.parts {
        let _ = write!(out, "\n[[part]]\nid = {}\n", quoted(&part.part));
        for (key, value) in [
            ("spin", &part.spin),
            ("stroke", &part.stroke),
            ("angle", &part.angle),
            ("level", &part.level),
            ("travel", &part.travel),
            ("show", &part.show),
            ("blink", &part.blink),
            ("alarm", &part.alarm),
            ("active", &part.active),
            ("equals", &part.equals),
        ] {
            if let Some(value) = value {
                let _ = writeln!(out, "{key} = {}", quoted(value));
            }
        }
        if let Some([low, high]) = part.range {
            let _ = writeln!(out, "range = [{low}, {high}]");
        }
        if let Some([x, y]) = part.move_by {
            let _ = writeln!(out, "move = [{x}, {y}]");
        }
        if let Some(value) = part.value {
            let _ = writeln!(out, "value = {value}");
        }
    }
    out
}
