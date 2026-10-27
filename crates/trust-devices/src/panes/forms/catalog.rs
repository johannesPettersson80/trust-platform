//! The communication catalog as the forms read it: the protocols of a `comm.schema` answer, with
//! the words of the add picker and of the forms that the answer carries (specification 11,
//! "Communication schema, capabilities, and Test"). A runtime whose catalog predates those words
//! gets the words of the same protocol from the tool's own catalog (specification 25 §5.14).

use std::collections::BTreeMap;

use serde_json::Value;

use crate::gather::Gathered;
use crate::view::types::VisibleWhen;

/// The protocols of a communication catalog, with the add picker's groups.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    /// The picker's groups, in their canonical order.
    pub groups: Vec<PickerGroup>,
    /// The protocols, in the catalog's order.
    pub protocols: Vec<CatalogProtocol>,
}

/// A group of the add picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerGroup {
    /// Its stable identity.
    pub id: String,
    /// Its name.
    pub label: String,
    /// Whether the dialog shows it only on request.
    pub advanced: bool,
    /// The protocols it holds, in their order.
    pub protocols: Vec<String>,
}

/// A protocol's entry in the add picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerText {
    /// The entry's title.
    pub title: String,
    /// Its badge.
    pub badge: String,
    /// What the protocol is for, in the picker's words.
    pub purpose: String,
}

/// A protocol of the catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogProtocol {
    /// Its ID, such as `modbus_tcp`.
    pub id: String,
    /// Its title, the form's word.
    pub title: String,
    /// What it is for, the form's words.
    pub purpose: String,
    /// Whether `Test` can test it.
    pub supports_test: bool,
    /// The actions it allows, such as `disable`.
    pub actions: Vec<String>,
    /// Its fields.
    pub fields: Vec<CatalogField>,
    /// Its entry in the add picker; `None` for a protocol the catalog gives none.
    pub picker: Option<PickerText>,
}

/// A field of a protocol.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogField {
    /// Its ID.
    pub id: String,
    /// Its label.
    pub label: String,
    /// Its schema type, such as `number` or `json_array`.
    pub field_type: String,
    /// Whether it is required.
    pub required: bool,
    /// Whether it belongs under `Advanced`.
    pub advanced: bool,
    /// Whether it holds a secret.
    pub secret: bool,
    /// Its help.
    pub help: String,
    /// Its default.
    pub default: Value,
    /// The options of a choice.
    pub options: Vec<String>,
    /// The label of each option.
    pub option_labels: BTreeMap<String, String>,
    /// When it is shown.
    pub visible_when: Option<VisibleWhen>,
    /// The words and the seed of a list's editor.
    pub list: Option<ListTexts>,
}

/// The words and the seed of a list editor, as the catalog gives them.
#[derive(Debug, Clone, PartialEq)]
pub struct ListTexts {
    /// What one item is called.
    pub item_noun: String,
    /// What shows while the list is empty.
    pub empty_text: String,
    /// The item that adding appends when the list holds none to copy; null when none can be
    /// added.
    pub item_template: Value,
    /// The type of each item of a list of numbers, such as `unsigned`.
    pub item_type: Option<String>,
    /// The properties of each item of a list of objects, in their order.
    pub item_fields: Vec<ItemField>,
    /// The groups of properties of which an item sets exactly one.
    pub item_one_of: Vec<OneOf>,
}

impl ListTexts {
    /// Whether the catalog describes the list's items, which a new item then starts from.
    #[must_use]
    pub fn describes_items(&self) -> bool {
        self.item_type.is_some() || !self.item_fields.is_empty()
    }
}

/// A property of the items of a list of objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemField {
    /// Its key in an item.
    pub id: String,
    /// Its label.
    pub label: String,
    /// `string` or `enum`.
    pub field_type: String,
    /// Whether an item needs it.
    pub required: bool,
    /// The choices of an `enum`.
    pub options: Vec<String>,
}

/// Properties of which an item sets exactly one; a new item holds the first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneOf {
    /// The properties.
    pub fields: Vec<String>,
    /// The protocol's boolean field that, while on, lets an item set none of them.
    pub unless: Option<String>,
}

impl Catalog {
    /// The catalog of a `comm.schema` answer; `None` when the answer holds no list of protocols.
    #[must_use]
    pub fn parse(answer: &Value) -> Option<Self> {
        let protocols = answer.get("protocols")?.as_array()?;
        Some(Self {
            groups: answer
                .get("picker_groups")
                .and_then(Value::as_array)
                .map(|groups| groups.iter().filter_map(PickerGroup::parse).collect())
                .unwrap_or_default(),
            protocols: protocols
                .iter()
                .filter_map(CatalogProtocol::parse)
                .collect(),
        })
    }

    /// The catalog of the runtime at the control endpoint `endpoint` (specification 25 §5.14):
    /// the one it served this gather, else the tool's own. A served catalog without the words of
    /// the picker and the forms takes them from the tool's own.
    #[must_use]
    pub fn for_runtime(gathered: &Gathered, endpoint: Option<&str>) -> Option<Self> {
        let own = gathered.project.catalog.as_ref().ok().and_then(Self::parse);
        let served = endpoint
            .map(str::trim)
            .filter(|endpoint| !endpoint.is_empty())
            .and_then(|endpoint| {
                gathered
                    .targets
                    .iter()
                    .find(|report| report.target.endpoint.trim() == endpoint)
            })
            .and_then(|report| match &report.live.schema {
                Some(Ok(answer)) => Self::parse(answer),
                _ => None,
            });
        match (served, own) {
            (Some(served), Some(own)) => Some(served.with_words_of(&own)),
            (served, own) => served.or(own),
        }
    }

    /// The protocol `id`.
    #[must_use]
    pub fn protocol(&self, id: &str) -> Option<&CatalogProtocol> {
        self.protocols.iter().find(|protocol| protocol.id == id)
    }

    /// This catalog, with the words it lacks taken from the same protocols and fields of `own`.
    fn with_words_of(mut self, own: &Self) -> Self {
        if self.groups.is_empty() {
            self.groups.clone_from(&own.groups);
        }
        for protocol in &mut self.protocols {
            let Some(known) = own.protocol(&protocol.id) else {
                continue;
            };
            if protocol.picker.is_none() {
                protocol.picker.clone_from(&known.picker);
            }
            for field in &mut protocol.fields {
                let Some(known) = known.field(&field.id) else {
                    continue;
                };
                if field.option_labels.is_empty() {
                    field.option_labels.clone_from(&known.option_labels);
                }
                if field.list.is_none() {
                    field.list.clone_from(&known.list);
                }
            }
        }
        self
    }
}

impl PickerGroup {
    fn parse(value: &Value) -> Option<Self> {
        Some(Self {
            id: value.get("id")?.as_str()?.to_owned(),
            label: text(value, "label"),
            advanced: flag(value, "advanced"),
            protocols: strings(value, "protocols"),
        })
    }
}

impl PickerText {
    fn parse(value: &Value) -> Option<Self> {
        let word = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        Some(Self {
            title: word("title")?,
            badge: word("badge")?,
            purpose: word("purpose")?,
        })
    }
}

impl CatalogProtocol {
    fn parse(value: &Value) -> Option<Self> {
        Some(Self {
            id: value.get("id")?.as_str()?.to_owned(),
            title: text(value, "title"),
            purpose: text(value, "purpose"),
            supports_test: flag(value, "supports_test"),
            actions: strings(value, "actions"),
            fields: value
                .get("fields")
                .and_then(Value::as_array)
                .map(|fields| fields.iter().filter_map(CatalogField::parse).collect())
                .unwrap_or_default(),
            picker: value.get("picker").and_then(PickerText::parse),
        })
    }

    /// The field `id`.
    #[must_use]
    pub fn field(&self, id: &str) -> Option<&CatalogField> {
        self.fields.iter().find(|field| field.id == id)
    }

    /// Whether the protocol allows `action`.
    #[must_use]
    pub fn allows(&self, action: &str) -> bool {
        self.actions.iter().any(|allowed| allowed == action)
    }
}

impl CatalogField {
    fn parse(value: &Value) -> Option<Self> {
        let field_type = text(value, "type");
        Some(Self {
            id: value.get("id")?.as_str()?.to_owned(),
            label: text(value, "label"),
            secret: flag(value, "secret") || field_type == "secret",
            field_type,
            required: flag(value, "required"),
            advanced: flag(value, "advanced"),
            help: text(value, "help"),
            default: value.get("default").cloned().unwrap_or(Value::Null),
            options: strings(value, "options"),
            option_labels: value
                .get("option_labels")
                .and_then(Value::as_object)
                .map(|labels| {
                    labels
                        .iter()
                        .filter_map(|(option, label)| {
                            Some((option.clone(), label.as_str()?.to_owned()))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            visible_when: value.get("visible_when").and_then(|rule| {
                Some(VisibleWhen {
                    field: rule.get("field")?.as_str()?.to_owned(),
                    equals: rule.get("equals").map(value_text).unwrap_or_default(),
                })
            }),
            list: value.get("list").and_then(ListTexts::parse),
        })
    }
}

impl ListTexts {
    fn parse(list: &Value) -> Option<Self> {
        let objects = |key: &str| {
            list.get(key)
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default()
        };
        Some(Self {
            item_noun: list.get("item_noun")?.as_str()?.to_owned(),
            empty_text: text(list, "empty_text"),
            item_template: list.get("item_template").cloned().unwrap_or(Value::Null),
            item_type: list
                .get("item_type")
                .and_then(Value::as_str)
                .map(str::to_owned),
            item_fields: objects("item_fields")
                .iter()
                .filter_map(|field| {
                    Some(ItemField {
                        id: field.get("id")?.as_str()?.to_owned(),
                        label: text(field, "label"),
                        field_type: text(field, "type"),
                        required: flag(field, "required"),
                        options: strings(field, "options"),
                    })
                })
                .collect(),
            item_one_of: objects("item_one_of")
                .iter()
                .map(|group| OneOf {
                    fields: strings(group, "fields"),
                    unless: group
                        .get("unless")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                })
                .collect(),
        })
    }
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// A value as the text that a visibility predicate compares.
pub(super) fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
