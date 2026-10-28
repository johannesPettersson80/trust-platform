//! Panel files, written `hmi/panels/<name>.toml` (specification 39, §8.7): a popup with a
//! description and the fields it needs, its objects binding fields as `$Name`, optionally docked
//! on pages as a sidebar (§21.1 item 3). Component definitions are in `pairs`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use toml::Spanned;

use super::docked::HmiDock;
use super::lines::problem_line;
use super::load::{check_format, error_at, line_at, toml_error, PageFileError, RawObject};
use super::pipes::RawPipe;
use super::popups::popup_from;
use super::types::HmiPopup;
use super::write::{entry, flag, key, optional, string, write_body};
use super::SUPPORTED_FORMAT;

/// The kinds of value a field may take, as a component property's (§5.6).
const KINDS: &[&str] = &[
    "number",
    "boolean",
    "text",
    "time",
    "enumeration",
    "structure",
    "array",
    "value",
];

/// A field a faceplate needs (§8.7, item 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HmiField {
    /// The kinds of value it takes; any when empty.
    #[serde(default)]
    pub accepts: Vec<String>,
}

/// A faceplate file: a popup bound to one instance through its fields (§8.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HmiFaceplate {
    /// Offers the Settings dialog for its instance, from its component definition's settings
    /// (§21.4).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub settings: bool,
    /// The instance whose settings the dialog edits, filled in when the runtime serves the panel
    /// bound to its instance; never written in a panel file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_instance: Option<String>,
    /// Where it docks on pages as a sidebar (§21.1 item 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<HmiDock>,
    /// Its title, size, pin, objects and pipes.
    #[serde(flatten)]
    pub popup: HmiPopup,
    /// The line under its title in the header (§8.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Its fields by name.
    #[serde(default)]
    pub fields: BTreeMap<String, HmiField>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFaceplate {
    #[serde(default)]
    settings: bool,
    #[serde(default)]
    panel: Option<Spanned<HmiDock>>,
    #[allow(dead_code)]
    format: i64,
    title: String,
    #[serde(default)]
    description: Option<String>,
    size: Spanned<[u32; 2]>,
    #[serde(default)]
    pinnable: Option<bool>,
    #[serde(default)]
    fields: BTreeMap<String, RawField>,
    #[serde(default, rename = "object")]
    objects: Vec<RawObject>,
    #[serde(default, rename = "pipe")]
    pipes: Vec<RawPipe>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawField {
    #[serde(default)]
    accepts: Option<Spanned<Vec<String>>>,
}

/// Parses and validates one faceplate file: a popup whose `$` bindings name its fields.
pub fn parse_faceplate(file: &str, text: &str) -> Result<HmiFaceplate, Vec<PageFileError>> {
    check_format(file, text)?;
    let raw: RawFaceplate =
        toml::from_str(text).map_err(|error| vec![toml_error(file, text, &error)])?;
    let mut fields = BTreeMap::new();
    for (name, field) in raw.fields {
        let accepts = match field.accepts {
            None => Vec::new(),
            Some(accepts) => {
                let line = line_at(text, accepts.span().start);
                let kinds = accepts.into_inner();
                if let Some(unknown) = kinds.iter().find(|kind| !KINDS.contains(&kind.as_str())) {
                    return Err(vec![error_at(
                        file,
                        line,
                        format!("`{unknown}` is not a kind of value: {}", KINDS.join(", ")),
                    )]);
                }
                kinds
            }
        };
        fields.insert(name, HmiField { accepts });
    }
    let popup = popup_from(
        file,
        text,
        raw.title,
        raw.size,
        raw.pinnable,
        raw.objects,
        raw.pipes,
    )?;
    let errors: Vec<PageFileError> = popup
        .page
        .objects
        .iter()
        .flat_map(|object| {
            object
                .bind
                .iter()
                .map(move |(property, binding)| (object, property, binding))
        })
        .filter_map(|(object, property, binding)| {
            let name = binding.path.strip_prefix('$')?;
            (!fields.contains_key(name)).then(|| {
                error_at(
                    file,
                    problem_line(text, &object.id, Some(property)),
                    format!(
                        "`${name}` is not a field of this faceplate; declare it as [fields.{name}]"
                    ),
                )
            })
        })
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    let panel = match raw.panel {
        None => None,
        Some(panel) => {
            let line = line_at(text, panel.span().start);
            let panel = panel.into_inner();
            if let Err(message) = panel.validate() {
                return Err(vec![error_at(file, line, message)]);
            }
            Some(panel)
        }
    };
    Ok(HmiFaceplate {
        settings: raw.settings,
        settings_instance: None,
        panel,
        popup,
        description: raw.description,
        fields,
    })
}

/// Writes a faceplate file in canonical TOML: its title, description, size and pin, its fields
/// by name, then its objects and pipes.
pub fn write_faceplate(faceplate: &HmiFaceplate) -> String {
    let mut out = String::new();
    entry(&mut out, "format", &SUPPORTED_FORMAT.to_string());
    entry(&mut out, "title", &string(&faceplate.popup.page.title));
    optional(&mut out, "description", faceplate.description.as_deref());
    flag(&mut out, "settings", faceplate.settings);
    entry(
        &mut out,
        "size",
        &format!("[{}, {}]", faceplate.popup.size[0], faceplate.popup.size[1]),
    );
    flag(&mut out, "pinnable", faceplate.popup.pinnable);
    if let Some(panel) = &faceplate.panel {
        write_dock(&mut out, panel);
    }
    for (name, field) in &faceplate.fields {
        out.push_str(&format!("\n[fields.{}]\n", key(name)));
        if !field.accepts.is_empty() {
            let kinds: Vec<String> = field.accepts.iter().map(|kind| string(kind)).collect();
            entry(&mut out, "accepts", &format!("[{}]", kinds.join(", ")));
        }
    }
    write_body(&mut out, &faceplate.popup.page);
    out
}

fn write_dock(out: &mut String, panel: &HmiDock) {
    use super::docked::{HmiDockEdge, HmiDockScope};
    out.push_str("\n[panel]\n");
    let scope = match panel.scope {
        HmiDockScope::Manual => "manual",
        HmiDockScope::All => "all",
        HmiDockScope::Selected => "selected",
    };
    entry(out, "scope", &string(scope));
    if !panel.pages.is_empty() {
        let pages: Vec<String> = panel.pages.iter().map(|page| string(page)).collect();
        entry(out, "pages", &format!("[{}]", pages.join(", ")));
    }
    let dock = match panel.dock {
        HmiDockEdge::Left => "left",
        HmiDockEdge::Right => "right",
        HmiDockEdge::Top => "top",
        HmiDockEdge::Bottom => "bottom",
    };
    entry(out, "dock", &string(dock));
    if panel.order != 0 {
        entry(out, "order", &panel.order.to_string());
    }
}
