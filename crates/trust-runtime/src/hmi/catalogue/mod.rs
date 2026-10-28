//! The component catalogue (specification 39, §5.6).
//!
//! Every component type is defined once, in `components.json`. The runtime and the language
//! server read it here; the editor and the drawing module bundle the same file.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::page_files::HmiBindingDirection;
use crate::value::Value;

#[cfg(test)]
mod tests;

/// The catalogue file as shipped.
pub const CATALOGUE_JSON: &str = include_str!("components.json");

/// All component definitions.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    /// Version of the catalogue file.
    pub format: u32,
    /// Definitions in the order the editor offers them.
    pub components: Vec<ComponentDef>,
}

/// One component type.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentDef {
    /// Type name used in page files, for example `bar`.
    #[serde(rename = "type")]
    pub kind: String,
    /// What the component is for.
    pub category: ComponentCategory,
    /// Delivery phase that introduces it (§18).
    pub phase: u8,
    /// Size of a new object in pixels.
    pub size: [u32; 2],
    /// Symbol variants.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<String>,
    /// Bindable properties by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, PropertyDef>,
    /// Rule for any further property, for components with named pens such as the trend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional: Option<PropertyDef>,
    /// The ports its pipes connect to (§6, item 1); none for displays and controls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<String>,
    /// Whether it attaches to the equipment or the pipe it measures, as an instrument does (§6,
    /// item 5).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub attaches: bool,
    /// For equipment, the heading of §5.2 it is listed under, grouping it in the editor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

impl ComponentDef {
    /// The visibility binding every object may have (specification 39, §18 phase 5 item 2): the
    /// object is shown only while a BOOL of the program is TRUE.
    pub fn with_visibility(&mut self) {
        self.properties
            .entry(VISIBLE.to_string())
            .or_insert_with(|| PropertyDef {
                accepts: vec![ValueKind::Boolean],
                directions: vec![HmiBindingDirection::Read],
            });
    }
}

/// The name of the visibility binding.
pub const VISIBLE: &str = "visible";

/// What a component is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentCategory {
    /// Shows values.
    Display,
    /// Sends commands and values.
    Control,
    /// Arranges a page.
    Layout,
    /// A process or motion symbol.
    Equipment,
}

/// A bindable property.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyDef {
    /// Kinds of value the property accepts.
    pub accepts: Vec<ValueKind>,
    /// Directions a binding of the property may have.
    pub directions: Vec<HmiBindingDirection>,
}

/// A kind of value a property accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValueKind {
    /// Integer, real and bit-string types.
    Number,
    /// `BOOL`.
    Boolean,
    /// Strings and characters.
    Text,
    /// Durations, dates and times of day.
    Time,
    /// An enumeration.
    Enumeration,
    /// A structure.
    Structure,
    /// An array.
    Array,
    /// Any single value: number, boolean, text, time or enumeration.
    Value,
}

impl Catalogue {
    /// The catalogue shipped with this truST.
    pub fn shipped() -> &'static Catalogue {
        static SHIPPED: OnceLock<Catalogue> = OnceLock::new();
        SHIPPED.get_or_init(|| {
            let mut catalogue: Catalogue = serde_json::from_str(CATALOGUE_JSON)
                .expect("the shipped component catalogue is valid");
            catalogue
                .components
                .iter_mut()
                .for_each(ComponentDef::with_visibility);
            catalogue
        })
    }

    /// The definition of a component type.
    pub fn component(&self, kind: &str) -> Option<&ComponentDef> {
        self.components
            .iter()
            .find(|component| component.kind == kind)
    }
}

impl ComponentDef {
    /// The rule for a property: a listed property, or the rule for further properties.
    pub fn property(&self, name: &str) -> Option<&PropertyDef> {
        self.properties.get(name).or(self.additional.as_ref())
    }
}

impl ValueKind {
    /// Every kind, in the order the catalogue documents them.
    pub const ALL: [ValueKind; 8] = [
        Self::Number,
        Self::Boolean,
        Self::Text,
        Self::Time,
        Self::Enumeration,
        Self::Structure,
        Self::Array,
        Self::Value,
    ];

    /// The kinds a runtime value is of.
    pub fn kinds_of(value: &Value) -> Vec<ValueKind> {
        Self::ALL
            .into_iter()
            .filter(|kind| kind.matches(value))
            .collect()
    }

    /// Whether a runtime value is of this kind.
    pub fn matches(self, value: &Value) -> bool {
        match self {
            Self::Number => matches!(
                value,
                Value::SInt(_)
                    | Value::Int(_)
                    | Value::DInt(_)
                    | Value::LInt(_)
                    | Value::USInt(_)
                    | Value::UInt(_)
                    | Value::UDInt(_)
                    | Value::ULInt(_)
                    | Value::Real(_)
                    | Value::LReal(_)
                    | Value::Byte(_)
                    | Value::Word(_)
                    | Value::DWord(_)
                    | Value::LWord(_)
            ),
            Self::Boolean => matches!(value, Value::Bool(_)),
            Self::Text => matches!(
                value,
                Value::String(_) | Value::WString(_) | Value::Char(_) | Value::WChar(_)
            ),
            Self::Time => matches!(
                value,
                Value::Time(_)
                    | Value::LTime(_)
                    | Value::Date(_)
                    | Value::LDate(_)
                    | Value::Tod(_)
                    | Value::LTod(_)
                    | Value::Dt(_)
                    | Value::Ldt(_)
            ),
            Self::Enumeration => matches!(value, Value::Enum(_)),
            Self::Structure => matches!(value, Value::Struct(_)),
            Self::Array => matches!(value, Value::Array(_)),
            Self::Value => [
                Self::Number,
                Self::Boolean,
                Self::Text,
                Self::Time,
                Self::Enumeration,
            ]
            .into_iter()
            .any(|kind| kind.matches(value)),
        }
    }

    /// The kind in plain words, for messages.
    pub fn words(self) -> &'static str {
        match self {
            Self::Number => "a number",
            Self::Boolean => "a boolean",
            Self::Text => "text",
            Self::Time => "a time",
            Self::Enumeration => "an enumeration",
            Self::Structure => "a structure",
            Self::Array => "an array",
            Self::Value => "a single value",
        }
    }
}
