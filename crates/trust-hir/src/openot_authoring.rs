//! OpenOT authoring attribute vocabulary and validation.
//!
//! The vocabulary mirrors `open-ot-ref/docs/authoring-attributes.md` and is used
//! by HIR diagnostics, IDE completions/inlay hints, and runtime lowering.

use std::collections::{BTreeMap, BTreeSet};

use text_size::TextRange;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::diagnostics::{Diagnostic, DiagnosticCode};
use crate::types::TypeId;

/// OpenOT attribute kinds supported by the truST authoring layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OotKind {
    /// Emits `ValueChanged`.
    Value,
    /// Emits `StateTransition`.
    State,
    /// Emits condition active/cleared records.
    Alarm,
    /// Emits `Message`.
    Message,
}

impl OotKind {
    /// Parse an OpenOT kind value.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "value" => Some(Self::Value),
            "state" => Some(Self::State),
            "alarm" => Some(Self::Alarm),
            "message" => Some(Self::Message),
            _ => None,
        }
    }

    /// Canonical lower-case spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::State => "state",
            Self::Alarm => "alarm",
            Self::Message => "message",
        }
    }
}

/// Supported OpenOT kind values.
pub const KINDS: &[&str] = &["value", "state", "alarm", "message"];
/// Keys accepted on `value` attributes.
pub const VALUE_KEYS: &[&str] = &["unit", "deadband"];
/// Keys accepted on `state` attributes.
pub const STATE_KEYS: &[&str] = &["category", "model"];
/// Keys accepted on `alarm` attributes.
pub const ALARM_KEYS: &[&str] = &["class", "severity"];
/// Keys accepted on `message` attributes.
pub const MESSAGE_KEYS: &[&str] = &["template"];
/// State category values.
pub const CATEGORY_VALUES: &[&str] = &["process", "mode", "procedural"];
/// Machine-local process/equipment state category.
pub const STATE_CATEGORY_PROCESS: u16 = 0;
/// Machine-local operating mode category.
pub const STATE_CATEGORY_MODE: u16 = 1;
/// Canonical procedural state category.
pub const STATE_CATEGORY_PROCEDURAL: u16 = 2;
/// Procedural model values.
pub const MODEL_VALUES: &[&str] = &["ISA-88", "PackML"];
/// Condition class values.
pub const CLASS_VALUES: &[&str] = &["alarm", "interlock"];
/// Severity range used by the OpenOT authoring layer.
pub const SEVERITY_MIN: u16 = 1;
/// Severity range used by the OpenOT authoring layer.
pub const SEVERITY_MAX: u16 = 1000;

const OOT_KEY: &str = "oot";
const INTERNAL_ID_KEYS: &[&str] = &[
    "id",
    "sourceid",
    "valueid",
    "machineid",
    "statemachineid",
    "conditionid",
];

/// A parsed pragma attribute entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeEntry {
    /// Lower-case key.
    pub key: String,
    /// Raw string value.
    pub value: String,
    /// Source range for diagnostics.
    pub range: TextRange,
}

/// Parsed OpenOT attribute map from one declaration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttributeMap {
    entries: Vec<AttributeEntry>,
    values: BTreeMap<String, String>,
}

impl AttributeMap {
    /// Return the last value for a key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(&key.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// Return the parsed kind, if any.
    #[must_use]
    pub fn kind(&self) -> Option<OotKind> {
        self.get(OOT_KEY).and_then(OotKind::parse)
    }

    /// Return whether the attribute map contains `oot`.
    #[must_use]
    pub fn contains_oot(&self) -> bool {
        self.values.contains_key(OOT_KEY)
    }

    /// Return the map as owned key/value pairs for callers that do not need
    /// source ranges.
    #[must_use]
    pub fn to_btree_map(&self) -> BTreeMap<String, String> {
        self.values.clone()
    }
}

/// Parse all `{attribute ...}` pragma tokens attached to a syntax node.
#[must_use]
pub fn parse_attribute_map_from_node(node: &SyntaxNode) -> AttributeMap {
    let mut map = AttributeMap::default();
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::Pragma)
    {
        let range = token.text_range();
        for entry in parse_attribute_entries_from_text(token.text(), range) {
            map.values.insert(entry.key.clone(), entry.value.clone());
            map.entries.push(entry);
        }
    }
    map
}

/// Parse attribute key/value pairs from pragma text.
#[must_use]
pub fn parse_attribute_entries_from_text(text: &str, range: TextRange) -> Vec<AttributeEntry> {
    let mut entries = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('}') else {
            break;
        };
        let body = &rest[..end];
        if body.trim_start().starts_with("attribute") {
            for part in body.split(',') {
                let tokens = quoted_tokens(part);
                if tokens.len() >= 2 {
                    entries.push(AttributeEntry {
                        key: tokens[0].to_ascii_lowercase(),
                        value: tokens[1].clone(),
                        range,
                    });
                } else if tokens.len() == 1 {
                    if let Some((_, value)) = part.split_once(":=") {
                        entries.push(AttributeEntry {
                            key: tokens[0].to_ascii_lowercase(),
                            value: value.trim().trim_matches('\'').trim().to_string(),
                            range,
                        });
                    }
                }
            }
        }
        rest = &rest[end + 1..];
    }
    entries
}

/// Collect strict OpenOT authoring diagnostics for a syntax tree.
#[must_use]
pub fn collect_openot_attribute_diagnostics(root: &SyntaxNode) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for var_decl in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
    {
        let attrs = parse_attribute_map_from_node(&var_decl);
        if !attrs.contains_oot() {
            continue;
        }
        let declared_type = var_decl
            .children()
            .find(|child| child.kind() == SyntaxKind::TypeRef)
            .map(|node| compact_node_text(&node));
        diagnostics.extend(validate_attribute_map(&attrs, declared_type.as_deref()));
    }
    diagnostics
}

/// Validate a parsed OpenOT attribute map.
#[must_use]
pub fn validate_attribute_map(
    attrs: &AttributeMap,
    declared_type: Option<&str>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let Some(oot_entry) = attrs.entries.iter().find(|entry| entry.key == OOT_KEY) else {
        return diagnostics;
    };
    let Some(kind) = OotKind::parse(&oot_entry.value) else {
        diagnostics.push(openot_error(
            oot_entry.range,
            format!(
                "unknown OpenOT kind '{}'; expected {}",
                oot_entry.value,
                format_allowed(KINDS)
            ),
        ));
        return diagnostics;
    };
    if kind == OotKind::Value {
        if let Some(ty) = declared_type {
            if !is_supported_value_type_name(ty) {
                diagnostics.push(openot_error(
                    oot_entry.range,
                    "OpenOT value logging supports REAL and DINT only",
                ));
            }
        }
    }

    let allowed = allowed_keys(kind);
    let allowed_set = allowed.iter().copied().collect::<BTreeSet<_>>();
    for entry in attrs.entries.iter().filter(|entry| entry.key != OOT_KEY) {
        if INTERNAL_ID_KEYS.contains(&entry.key.as_str()) {
            validate_u32_key(entry, &mut diagnostics);
            continue;
        }
        if !allowed_set.contains(entry.key.as_str()) {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "unknown OpenOT key '{}' for kind '{}'; expected {}",
                    entry.key,
                    kind.as_str(),
                    format_allowed(allowed)
                ),
            ));
            continue;
        }
        validate_key_value(kind, entry, declared_type, &mut diagnostics);
    }
    if kind == OotKind::State {
        validate_state_category_model(attrs, &mut diagnostics);
    }

    diagnostics
}

/// Whether the OpenOT ST producer has a faithful value encoder for this built-in type.
#[must_use]
pub fn is_supported_value_type_id(type_id: TypeId) -> bool {
    matches!(type_id, TypeId::REAL | TypeId::DINT)
}

/// Whether the OpenOT ST producer has a faithful value encoder for this declared type name.
#[must_use]
pub fn is_supported_value_type_name(ty: &str) -> bool {
    TypeId::from_builtin_name(ty.trim()).is_some_and(is_supported_value_type_id)
}

/// Keys accepted by a kind, excluding `oot` and internal id-pinning keys.
#[must_use]
pub fn allowed_keys(kind: OotKind) -> &'static [&'static str] {
    match kind {
        OotKind::Value => VALUE_KEYS,
        OotKind::State => STATE_KEYS,
        OotKind::Alarm => ALARM_KEYS,
        OotKind::Message => MESSAGE_KEYS,
    }
}

/// Numeric code for a state category.
#[must_use]
pub fn category_code(value: &str) -> Option<u16> {
    match value.to_ascii_lowercase().as_str() {
        "process" => Some(STATE_CATEGORY_PROCESS),
        "mode" => Some(STATE_CATEGORY_MODE),
        "procedural" => Some(STATE_CATEGORY_PROCEDURAL),
        _ => None,
    }
}

/// Numeric code for a condition class.
#[must_use]
pub fn condition_class_code(value: &str) -> Option<u16> {
    match value.to_ascii_lowercase().as_str() {
        "alarm" => Some(0),
        "interlock" => Some(1),
        _ => None,
    }
}

fn validate_key_value(
    kind: OotKind,
    entry: &AttributeEntry,
    declared_type: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match (kind, entry.key.as_str()) {
        (OotKind::State, "category") if category_code(&entry.value).is_none() => {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "unknown OpenOT category '{}'; expected {}",
                    entry.value,
                    format_allowed(CATEGORY_VALUES)
                ),
            ));
        }
        (OotKind::State, "model") => {
            if !MODEL_VALUES
                .iter()
                .any(|value| value.eq_ignore_ascii_case(&entry.value))
            {
                diagnostics.push(openot_error(
                    entry.range,
                    format!(
                        "unknown OpenOT model '{}'; expected {}",
                        entry.value,
                        format_allowed(MODEL_VALUES)
                    ),
                ));
            }
        }
        (OotKind::Alarm, "class") if condition_class_code(&entry.value).is_none() => {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "unknown OpenOT class '{}'; expected {}",
                    entry.value,
                    format_allowed(CLASS_VALUES)
                ),
            ));
        }
        (OotKind::Alarm, "severity") => match entry.value.parse::<u16>() {
            Ok(value) if (SEVERITY_MIN..=SEVERITY_MAX).contains(&value) => {}
            _ => diagnostics.push(openot_error(
                entry.range,
                format!("OpenOT severity must be an integer in {SEVERITY_MIN}..={SEVERITY_MAX}"),
            )),
        },
        (OotKind::Value, "deadband") => {
            if entry.value.parse::<f64>().is_err() {
                diagnostics.push(openot_error(
                    entry.range,
                    "OpenOT deadband must be a numeric string",
                ));
            }
            if let Some(ty) = declared_type {
                if !ty.trim().eq_ignore_ascii_case("REAL") {
                    diagnostics.push(openot_error(
                        entry.range,
                        "OpenOT deadband is only valid on REAL value attributes",
                    ));
                }
            }
        }
        (OotKind::Value, "unit") | (OotKind::Message, "template")
            if entry.value.trim().is_empty() =>
        {
            diagnostics.push(openot_error(
                entry.range,
                format!("OpenOT '{}' value must not be empty", entry.key),
            ));
        }
        _ => {}
    }
}

fn validate_state_category_model(attrs: &AttributeMap, diagnostics: &mut Vec<Diagnostic>) {
    let category_entry = last_entry(attrs, "category");
    let model_entry = last_entry(attrs, "model");

    if let Some(model_entry) = model_entry {
        match category_entry.and_then(|entry| category_code(&entry.value)) {
            Some(STATE_CATEGORY_PROCEDURAL) => {}
            Some(_) => diagnostics.push(openot_error(
                model_entry.range,
                "OpenOT 'model' is only valid with category := 'procedural'",
            )),
            None if category_entry.is_none() => diagnostics.push(openot_error(
                model_entry.range,
                "OpenOT 'model' requires category := 'procedural'",
            )),
            None => {}
        }
    }

    if let Some(category_entry) = category_entry {
        if category_code(&category_entry.value) == Some(STATE_CATEGORY_PROCEDURAL)
            && model_entry.is_none()
        {
            diagnostics.push(openot_error(
                category_entry.range,
                "OpenOT category := 'procedural' requires a 'model'",
            ));
        }
    }
}

fn last_entry<'a>(attrs: &'a AttributeMap, key: &str) -> Option<&'a AttributeEntry> {
    attrs.entries.iter().rev().find(|entry| entry.key == key)
}

fn validate_u32_key(entry: &AttributeEntry, diagnostics: &mut Vec<Diagnostic>) {
    if entry.value.parse::<u32>().is_err() {
        diagnostics.push(openot_error(
            entry.range,
            format!("OpenOT '{}' must be a UDINT-compatible integer", entry.key),
        ));
    }
}

fn openot_error(range: TextRange, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(DiagnosticCode::InvalidOpenOtAttribute, range, message)
}

fn format_allowed(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| format!("'{value}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn compact_node_text(node: &SyntaxNode) -> String {
    let mut text = String::new();
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if token.kind() != SyntaxKind::Pragma {
            text.push_str(token.text());
        }
    }
    text.trim().to_string()
}

fn quoted_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('\'') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('\'') else {
            break;
        };
        tokens.push(after_start[..end].to_string());
        rest = &after_start[end + 1..];
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_syntax::parser::parse;

    #[test]
    fn validates_known_and_unknown_state_category() {
        let source = r#"
PROGRAM Main
VAR
    Step : INT {attribute 'oot' := 'state', 'category' := 'banana'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown OpenOT category")));
    }

    #[test]
    fn accepts_documented_values() {
        let source = r#"
PROGRAM Main
VAR
    Level : REAL {attribute 'oot' := 'value', 'unit' := 'L', 'deadband' := '0.5'};
    BatchCount : DINT {attribute 'oot' := 'value'};
    Step : INT {attribute 'oot' := 'state', 'category' := 'procedural', 'model' := 'ISA-88'};
    Alarm : BOOL {attribute 'oot' := 'alarm', 'class' := 'interlock', 'severity' := '900'};
    Started : BOOL {attribute 'oot' := 'message', 'template' := 'batch started'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn rejects_value_attributes_for_unsupported_value_types() {
        let source = r#"
PROGRAM Main
VAR
    LongReal : LREAL {attribute 'oot' := 'value'};
    LongCount : LINT {attribute 'oot' := 'value'};
    UnsignedCount : ULINT {attribute 'oot' := 'value'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        let unsupported = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .message
                    .contains("OpenOT value logging supports REAL and DINT only")
            })
            .count();
        assert_eq!(unsupported, 3, "{diagnostics:#?}");
    }

    #[test]
    fn rejects_model_without_explicit_procedural_category() {
        let source = r#"
PROGRAM Main
VAR
    Step : INT {attribute 'oot' := 'state', 'model' := 'ISA-88'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        assert!(diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("requires category := 'procedural'")));
    }

    #[test]
    fn rejects_procedural_category_without_model() {
        let source = r#"
PROGRAM Main
VAR
    Step : INT {attribute 'oot' := 'state', 'category' := 'procedural'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires a 'model'")));
    }
}
