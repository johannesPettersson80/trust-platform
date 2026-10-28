//! Read-only presentation defaults shared by the editor and runtime (§5.9).
use super::{
    bundle::HmiBundle,
    catalogue::{Catalogue, ValueKind},
    page_files::{parse_page, parse_popup, HmiBindingDirection, HmiObject},
};
use serde::Serialize;
use std::collections::BTreeMap;

mod equipment;
pub use equipment::EquipmentPresentation;
pub(crate) use equipment::{alarm_value_format, equipment_presentation};

/// A bound value's inherited display format. Authoring properties remain unchanged.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ValueFormat {
    /// Engineering unit declared by another display of this value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Decimal places declared by another display of this value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
}

/// Project defaults, indexed by the IEC path in ASCII lower case.
pub fn value_formats(bundle: &HmiBundle, catalogue: &Catalogue) -> BTreeMap<String, ValueFormat> {
    let mut formats = BTreeMap::<String, ValueFormat>::new();
    let mut quantities = Vec::<Vec<String>>::new();
    for (file, bytes) in &bundle.files {
        if !file.ends_with(".toml") {
            continue;
        }
        let text = String::from_utf8_lossy(bytes);
        let page = if file.starts_with("hmi/pages/") {
            parse_page(file, &text)
        } else if file.starts_with("hmi/popups/") {
            parse_popup(file, &text).map(|popup| popup.page)
        } else {
            continue;
        };
        let Ok(mut page) = page else {
            continue;
        };
        // Layer order never changes a value's default format.
        page.objects.sort_by(|a, b| a.id.cmp(&b.id));
        for object in &page.objects {
            if object.kind == "controller" {
                quantities.extend(controller_quantity(object, &page.objects));
            }
            let Some(component) = catalogue.component(&object.kind) else {
                continue;
            };
            for (name, binding) in &object.bind {
                if binding.direction == HmiBindingDirection::Write || binding.path.starts_with('$')
                {
                    continue;
                }
                let numeric = component.property(name).is_some_and(|property| {
                    property.accepts.iter().any(|kind| {
                        matches!(
                            kind,
                            ValueKind::Number
                                | ValueKind::Value
                                | ValueKind::Array
                                | ValueKind::Structure
                        )
                    })
                });
                if !numeric {
                    continue;
                }
                let unit = if object.kind == "trend" {
                    binding.unit.as_ref().or(object.unit.as_ref())
                } else {
                    object.unit.as_ref().or(binding.unit.as_ref())
                }
                .filter(|unit| !unit.trim().is_empty());
                if unit.is_none() && object.decimals.is_none() {
                    continue;
                }
                let format = formats
                    .entry(binding.path.to_ascii_lowercase())
                    .or_default();
                if format.unit.is_none() {
                    format.unit = unit.cloned();
                }
                if format.decimals.is_none() {
                    format.decimals = object.decimals;
                }
            }
        }
    }
    // One quantity, one format: each of its values takes what it lacks from the others, in order.
    for paths in quantities {
        let mut shared = ValueFormat::default();
        for format in paths.iter().filter_map(|path| formats.get(path)) {
            shared.unit = shared.unit.or_else(|| format.unit.clone());
            shared.decimals = shared.decimals.or(format.decimals);
        }
        if shared == ValueFormat::default() {
            continue;
        }
        for path in paths {
            let format = formats.entry(path).or_default();
            format.unit = format.unit.take().or_else(|| shared.unit.clone());
            format.decimals = format.decimals.or(shared.decimals);
        }
    }
    formats
}

/// A loop controller's measurement, the value of the instrument it reads and its setpoint: one
/// quantity, which the controller compares (§5.9). The paths in ASCII lower case.
fn controller_quantity(controller: &HmiObject, objects: &[HmiObject]) -> Option<Vec<String>> {
    let (measurement, setpoint) = (controller.bind.get("pv")?, controller.bind.get("sp")?);
    let instrument = controller
        .measures
        .as_deref()
        .and_then(|id| objects.iter().find(|object| object.id == id))
        .and_then(|instrument| instrument.bind.get("value"));
    let paths: Vec<String> = [Some(measurement), instrument, Some(setpoint)]
        .into_iter()
        .flatten()
        .filter(|binding| !binding.path.starts_with('$'))
        .map(|binding| binding.path.to_ascii_lowercase())
        .collect();
    (paths.len() > 1).then_some(paths)
}

#[cfg(test)]
mod tests;
