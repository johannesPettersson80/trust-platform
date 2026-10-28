//! Canonical writing of page files (specification 39, §15.2).

use std::fmt::Write as _;

use super::load::SUPPORTED_FORMAT;
use super::types::{
    HmiArrange, HmiBinding, HmiBindingDirection, HmiButtonAction, HmiClick, HmiNavigationPosition,
    HmiNavigationStyle, HmiObject, HmiPage, HmiPipe, HmiPipeStyle, HmiPopup, HmiRole, HmiScalar,
    HmiScreen, HmiSymbolStyle, HmiTheme,
};

/// Writes a page in canonical TOML: fixed key order, objects in z-order, bindings sorted by
/// property name, one value per line.
///
/// Writing a loaded canonical page without changes reproduces it byte for byte, so a change to
/// one value changes one line.
pub fn write_page(page: &HmiPage) -> String {
    let mut out = String::new();
    entry(&mut out, "format", &SUPPORTED_FORMAT.to_string());
    entry(&mut out, "title", &string(&page.title));
    if let Some(order) = page.order {
        entry(&mut out, "order", &order.to_string());
    }
    optional(&mut out, "icon", page.icon.as_deref());
    optional(&mut out, "group", page.group.as_deref());
    flag(&mut out, "hidden", page.hidden);
    flag(&mut out, "incomplete", page.incomplete);
    optional(&mut out, "pid", page.pid.as_deref());
    super::regions::write(&mut out, &page.regions);
    write_body(&mut out, page);
    out
}

/// Writes a popup file in canonical TOML: its title, size and pin, then its objects and pipes
/// as a page's (§4.9).
pub fn write_popup(popup: &HmiPopup) -> String {
    let mut out = String::new();
    entry(&mut out, "format", &SUPPORTED_FORMAT.to_string());
    entry(&mut out, "title", &string(&popup.page.title));
    entry(
        &mut out,
        "size",
        &format!("[{}, {}]", popup.size[0], popup.size[1]),
    );
    flag(&mut out, "pinnable", popup.pinnable);
    write_body(&mut out, &popup.page);
    out
}

/// The objects in z-order, then the pipes.
pub(super) fn write_body(out: &mut String, page: &HmiPage) {
    for object in &page.objects {
        write_object(out, object);
    }
    for pipe in &page.pipes {
        write_pipe(out, pipe);
    }
}

/// Writes the screen layout file in canonical TOML. Top-bar options are written only when they
/// are on, the side-bar position only when it is not the default left, and the project's
/// colours per theme, sorted by token name.
pub fn write_screen(screen: &HmiScreen) -> String {
    let mut out = String::new();
    entry(&mut out, "format", &SUPPORTED_FORMAT.to_string());
    entry(
        &mut out,
        "size",
        &format!("[{}, {}]", screen.size[0], screen.size[1]),
    );
    let theme = match screen.theme {
        HmiTheme::Light => "light",
        HmiTheme::Dark => "dark",
    };
    entry(&mut out, "theme", &string(theme));
    optional(&mut out, "start_page", screen.start_page.as_deref());
    let symbols = match screen.symbols {
        HmiSymbolStyle::Flat => "flat",
    };
    entry(&mut out, "symbols", &string(symbols));
    if screen.languages != ["en"] {
        let tags: Vec<String> = screen.languages.iter().map(|tag| string(tag)).collect();
        entry(&mut out, "languages", &format!("[{}]", tags.join(", ")));
    }
    if let Some(top_bar) = &screen.top_bar {
        out.push_str("\n[top_bar]\n");
        entry(&mut out, "title", &string(&top_bar.title));
        optional(&mut out, "logo", top_bar.logo.as_deref());
        flag(&mut out, "clock", top_bar.clock);
        flag(&mut out, "alarms", top_bar.alarms);
        flag(&mut out, "user", top_bar.user);
        flag(&mut out, "page_title", top_bar.page_title);
    }
    out.push_str("\n[navigation]\n");
    let style = match screen.navigation.style {
        HmiNavigationStyle::Side => "side",
        HmiNavigationStyle::Tabs => "tabs",
        HmiNavigationStyle::None => "none",
    };
    entry(&mut out, "style", &string(style));
    if screen.navigation.style == HmiNavigationStyle::Side
        && screen.navigation.position == HmiNavigationPosition::Right
    {
        entry(&mut out, "position", &string("right"));
    }
    for (theme, colors) in [
        ("dark", &screen.colors.dark),
        ("light", &screen.colors.light),
    ] {
        if !colors.is_empty() {
            let _ = write!(out, "\n[colors.{theme}]\n");
            for (name, value) in colors {
                entry(&mut out, &key(name), &string(value));
            }
        }
    }
    out
}

pub(super) fn flag(out: &mut String, key: &str, on: bool) {
    if on {
        entry(out, key, "true");
    }
}

fn write_object(out: &mut String, object: &HmiObject) {
    out.push_str("\n[[object]]\n");
    entry(out, "id", &string(&object.id));
    optional(out, "tag", object.tag.as_deref());
    entry(out, "type", &string(&object.kind));
    optional(out, "variant", object.variant.as_deref());
    optional(out, "block", object.block.as_deref());
    optional(out, "faceplate", object.faceplate.as_deref());
    optional(out, "instance", object.instance.as_deref());
    entry(out, "at", &format!("[{}, {}]", object.at[0], object.at[1]));
    entry(
        out,
        "size",
        &format!("[{}, {}]", object.size[0], object.size[1]),
    );
    if object.rotation != 0 {
        entry(out, "rotation", &object.rotation.to_string());
    }
    optional(out, "label", object.label.as_deref());
    optional(out, "text", object.text.as_deref());
    optional(out, "image", object.image.as_deref());
    optional(out, "unit", object.unit.as_deref());
    if let Some(decimals) = object.decimals {
        entry(out, "decimals", &decimals.to_string());
    }
    if let Some(min) = object.min {
        entry(out, "min", &number(min));
    }
    if let Some(max) = object.max {
        entry(out, "max", &number(max));
    }
    if let Some(limits) = &object.limits {
        let values = [
            ("low-low", limits.low_low),
            ("low", limits.low),
            ("high", limits.high),
            ("high-high", limits.high_high),
        ]
        .into_iter()
        .filter_map(|(name, value)| value.map(|value| format!("{name} = {}", number(value))))
        .collect::<Vec<_>>();
        entry(out, "limits", &format!("{{ {} }}", values.join(", ")));
    }
    if let Some(span) = object.span {
        entry(out, "span", &span.to_string());
    }
    if let Some(arrange) = object.arrange {
        let name = match arrange {
            HmiArrange::Row => "row",
            HmiArrange::Column => "column",
            HmiArrange::Grid => "grid",
        };
        entry(out, "arrange", &string(name));
    }
    if let Some(gap) = object.gap {
        entry(out, "gap", &gap.to_string());
    }
    if let Some(role) = object.role {
        let name = match role {
            HmiRole::Operator => "operator",
            HmiRole::Engineer => "engineer",
        };
        entry(out, "role", &string(name));
    }
    optional(out, "confirm", object.confirm.as_deref());
    if let Some(action) = object.action {
        let name = match action {
            HmiButtonAction::Set => "set",
            HmiButtonAction::Reset => "reset",
            HmiButtonAction::Toggle => "toggle",
        };
        entry(out, "action", &string(name));
    }
    if let Some(options) = &object.options {
        let listed: Vec<String> = options.iter().map(|option| string(option)).collect();
        entry(out, "options", &format!("[{}]", listed.join(", ")));
    }
    if let Some(pulse) = object.pulse {
        entry(out, "pulse", &number(pulse));
    }
    if let Some(expect) = &object.expect {
        let value = match expect {
            HmiScalar::Bool(flag) => flag.to_string(),
            HmiScalar::Number(value) => number(*value),
            HmiScalar::Text(text) => string(text),
        };
        entry(out, "expect", &value);
    }
    if let Some(timeout) = object.timeout {
        entry(out, "timeout", &number(timeout));
    }
    optional(out, "attach", object.attach.as_deref());
    optional(out, "measures", object.measures.as_deref());
    optional(out, "acts_on", object.acts_on.as_deref());
    optional(out, "group", object.group.as_deref());
    if object.locked {
        entry(out, "locked", "true");
    }
    if let Some(command) = &object.command {
        write_command(out, command);
    }
    if !object.bind.is_empty() {
        out.push_str("\n[object.bind]\n");
        for (name, binding) in &object.bind {
            entry(out, &key(name), &binding_value(binding));
        }
    }
    if let Some(click) = &object.click {
        out.push_str("\n[object.click]\n");
        let (target, name) = match click {
            HmiClick::Faceplate { name, .. } => ("faceplate", name),
            HmiClick::Popup(name) => ("popup", name),
            HmiClick::Page(name) => ("page", name),
        };
        entry(out, target, &string(name));
        if let HmiClick::Faceplate {
            instance: Some(instance),
            ..
        } = click
        {
            entry(out, "instance", &string(instance));
        }
    }
}

fn write_command(out: &mut String, command: &crate::hmi::commands::capability::MethodCommand) {
    use crate::hmi::commands::capability::Argument;
    out.push_str("\n[object.command]\n");
    entry(out, "interface", &string(&command.interface));
    entry(out, "method", &string(&command.method));
    entry(out, "instance", &string(&command.instance));
    if !command.arguments.is_empty() {
        out.push_str("\n[object.command.arguments]\n");
        for (name, argument) in &command.arguments {
            let value = match argument {
                Argument::Input { input } => format!("{{ input = {input} }}"),
                Argument::Constant(HmiScalar::Bool(value)) => value.to_string(),
                Argument::Constant(HmiScalar::Number(value)) => number(*value),
                Argument::Constant(HmiScalar::Text(value)) => string(value),
            };
            entry(out, &key(name), &value);
        }
    }
}

fn write_pipe(out: &mut String, pipe: &HmiPipe) {
    out.push_str("\n[[pipe]]\n");
    entry(out, "from", &string(&pipe.from));
    entry(out, "to", &string(&pipe.to));
    if pipe.style == HmiPipeStyle::Signal {
        entry(out, "style", &string("signal"));
    }
    if !pipe.waypoints.is_empty() {
        let points: Vec<String> = pipe
            .waypoints
            .iter()
            .map(|[x, y]| format!("[{x}, {y}]"))
            .collect();
        entry(out, "waypoints", &format!("[{}]", points.join(", ")));
    }
    if !pipe.bind.is_empty() {
        out.push_str("\n[pipe.bind]\n");
        for (name, binding) in &pipe.bind {
            entry(out, &key(name), &binding_value(binding));
        }
    }
}

/// A binding is written as its path when it has neither a unit nor a non-default direction.
fn binding_value(binding: &HmiBinding) -> String {
    let direction = match binding.direction {
        HmiBindingDirection::Read => None,
        HmiBindingDirection::Write => Some("write"),
        HmiBindingDirection::ReadWrite => Some("read-write"),
    };
    if binding.unit.is_none()
        && direction.is_none()
        && binding.range.is_none()
        && binding.quality.is_none()
    {
        return string(&binding.path);
    }
    let mut parts = vec![format!("path = {}", string(&binding.path))];
    if let Some(quality) = &binding.quality {
        parts.push(format!("quality = {}", string(quality)));
    }
    if let Some(unit) = &binding.unit {
        parts.push(format!("unit = {}", string(unit)));
    }
    if let Some(direction) = direction {
        parts.push(format!("direction = {}", string(direction)));
    }
    if let Some([low, high]) = binding.range {
        parts.push(format!("range = [{}, {}]", number(low), number(high)));
    }
    format!("{{ {} }}", parts.join(", "))
}

pub(super) fn entry(out: &mut String, key: &str, value: &str) {
    let _ = writeln!(out, "{key} = {value}");
}

pub(super) fn optional(out: &mut String, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        entry(out, key, &string(value));
    }
}

/// A bare key when it can be one, a quoted key otherwise.
pub(super) fn key(name: &str) -> String {
    let bare = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if bare {
        name.to_string()
    } else {
        string(name)
    }
}

/// A TOML basic string.
pub(crate) fn string(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for c in value.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            '\r' => quoted.push_str("\\r"),
            c if c.is_control() => {
                let _ = write!(quoted, "\\u{:04X}", u32::from(c));
            }
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// Whole numbers without decimals, other values in their shortest exact form.
pub(super) fn number(value: f64) -> String {
    if value.is_nan() {
        "nan".to_string()
    } else if value.is_infinite() {
        if value > 0.0 { "inf" } else { "-inf" }.to_string()
    } else if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}
