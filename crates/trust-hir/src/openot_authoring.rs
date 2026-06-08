//! OpenOT authoring attribute vocabulary and validation.
//!
//! The vocabulary mirrors `open-ot-ref/docs/authoring-attributes.md` and is used
//! by HIR diagnostics, IDE completions/inlay hints, and runtime lowering.

use std::collections::{BTreeMap, BTreeSet};

use text_size::TextRange;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::db::FileId;
use crate::diagnostics::{Diagnostic, DiagnosticCode};
use crate::semantic::{DeclarationCatalog, DeclarationKind};
use crate::symbols::SymbolTable;
use crate::types::{Type, TypeId};

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
    /// Emits condition lifecycle command records.
    Condition,
    /// Emits `BatchEvent`.
    Batch,
    /// Emits `RecipeLoaded`.
    RecipeLoaded,
    /// Emits `RecipeApproved`.
    RecipeApproved,
    /// Emits `MaterialAddition`.
    MaterialAddition,
    /// Emits `OperatorAction`.
    OperatorAction,
    /// Emits `OperatorLogin`.
    OperatorLogin,
    /// Emits `OperatorLogout`.
    OperatorLogout,
    /// Emits `SecurityAccessFailure`.
    SecurityFailure,
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
            "condition" => Some(Self::Condition),
            "batch" => Some(Self::Batch),
            "recipe-loaded" => Some(Self::RecipeLoaded),
            "recipe-approved" => Some(Self::RecipeApproved),
            "material-addition" => Some(Self::MaterialAddition),
            "operator-action" => Some(Self::OperatorAction),
            "operator-login" => Some(Self::OperatorLogin),
            "operator-logout" => Some(Self::OperatorLogout),
            "security-failure" => Some(Self::SecurityFailure),
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
            Self::Condition => "condition",
            Self::Batch => "batch",
            Self::RecipeLoaded => "recipe-loaded",
            Self::RecipeApproved => "recipe-approved",
            Self::MaterialAddition => "material-addition",
            Self::OperatorAction => "operator-action",
            Self::OperatorLogin => "operator-login",
            Self::OperatorLogout => "operator-logout",
            Self::SecurityFailure => "security-failure",
        }
    }
}

/// Supported OpenOT kind values.
pub const KINDS: &[&str] = &[
    "value",
    "state",
    "alarm",
    "message",
    "condition",
    "batch",
    "recipe-loaded",
    "recipe-approved",
    "material-addition",
    "operator-action",
    "operator-login",
    "operator-logout",
    "security-failure",
];
/// Keys accepted on `value` attributes.
pub const VALUE_KEYS: &[&str] = &[
    "unit",
    "deadband",
    "quality",
    "semanticrole",
    "previous",
    "sampling",
    "interval",
];
/// Unit symbols accepted by the current OpenOT reference registry.
pub const UNIT_VALUES: &[&str] = &["1", "L", "degC", "bar", "rpm", "s", "ms", "kg", "m", "%"];
/// Keys accepted on `state` attributes.
pub const STATE_KEYS: &[&str] = &["category", "model"];
/// Keys accepted on `alarm` attributes.
pub const ALARM_KEYS: &[&str] = &["class", "severity", "cause"];
/// Keys accepted on `message` attributes.
pub const MESSAGE_KEYS: &[&str] = &["template", "severity", "arg1", "arg2", "arg3", "arg4"];
/// Keys accepted on condition lifecycle command attributes.
pub const CONDITION_KEYS: &[&str] = &[
    "of",
    "event",
    "by",
    "seconds",
    "reason",
    "comment",
    "new-priority",
    "previous-priority",
];
/// Condition lifecycle events supported by this authoring slice.
pub const CONDITION_EVENT_VALUES: &[&str] = &[
    "acknowledge",
    "confirm",
    "shelve",
    "unshelve",
    "suppress",
    "unsuppress",
    "out-of-service",
    "in-service",
    "reset",
    "comment",
    "priority-changed",
];
/// Keys accepted on `batch` attributes.
pub const BATCH_KEYS: &[&str] = &["batchid", "recipe"];
/// Keys accepted on `recipe-loaded` attributes.
pub const RECIPE_LOADED_KEYS: &[&str] = &["recipe", "version", "batch"];
/// Keys accepted on `recipe-approved` attributes.
pub const RECIPE_APPROVED_KEYS: &[&str] = &["recipe", "version", "auth", "by"];
/// Keys accepted on `material-addition` attributes.
pub const MATERIAL_ADDITION_KEYS: &[&str] = &["batch", "material", "quantity", "unit"];
/// Keys accepted on `operator-action` attributes.
pub const OPERATOR_ACTION_KEYS: &[&str] = &[
    "action",
    "actor",
    "context1",
    "context2",
    "context3",
    "context4",
    "auth",
    "workstation",
];
/// Keys accepted on `operator-login` attributes.
pub const OPERATOR_LOGIN_KEYS: &[&str] = &["actor", "auth", "workstation", "role"];
/// Keys accepted on `operator-logout` attributes.
pub const OPERATOR_LOGOUT_KEYS: &[&str] = &["actor", "workstation"];
/// Keys accepted on `security-failure` attributes.
pub const SECURITY_FAILURE_KEYS: &[&str] = &["actor", "workstation", "reason"];
/// Authentication result values from OpenOT §6.4.
pub const AUTH_RESULT_VALUES: &[&str] = &["Granted", "Denied", "NotRequired", "Pending", "Expired"];
/// Batch state enum values from OpenOT §6.4.
pub const BATCH_STATE_VALUES: &[&str] = &[
    "Started",
    "Completed",
    "Held",
    "Resumed",
    "Aborted",
    "Paused",
];
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
const ISA_88_STATES: &[(u16, &str)] = &[
    (0, "Idle"),
    (1, "Running"),
    (2, "Complete"),
    (3, "Pausing"),
    (4, "Paused"),
    (5, "Holding"),
    (6, "Held"),
    (7, "Restarting"),
    (8, "Stopping"),
    (9, "Stopped"),
    (10, "Aborting"),
    (11, "Aborted"),
];
const PACKML_STATES: &[(u16, &str)] = &[
    (0, "Idle"),
    (1, "Starting"),
    (2, "Execute"),
    (3, "Completing"),
    (4, "Complete"),
    (5, "Holding"),
    (6, "Held"),
    (7, "Unholding"),
    (8, "Suspending"),
    (9, "Suspended"),
    (10, "Unsuspending"),
    (11, "Stopping"),
    (12, "Stopped"),
    (13, "Aborting"),
    (14, "Aborted"),
    (15, "Clearing"),
    (16, "Resetting"),
];
/// Condition class values.
pub const CLASS_VALUES: &[&str] = &["alarm", "interlock"];
/// Value quality values.
pub const QUALITY_VALUES: &[&str] = &["good", "uncertain", "bad", "unknown"];
/// Value semantic-role values.
pub const SEMANTIC_ROLE_VALUES: &[&str] = &[
    "actual", "setpoint", "command", "count", "position", "status",
];
/// Value sampling policy values.
pub const SAMPLING_VALUES: &[&str] = &["on-change", "deadband", "periodic", "hysteresis"];
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
    for program in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Program)
    {
        diagnostics.extend(validate_local_openot_references(&program));
    }
    diagnostics
}

fn validate_local_openot_references(program: &SyntaxNode) -> Vec<Diagnostic> {
    let local_types = local_declared_types(program);
    let local_kinds = local_openot_kinds(program);
    let mut diagnostics = Vec::new();
    for var_decl in program
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
    {
        let attrs = parse_attribute_map_from_node(&var_decl);
        match attrs.kind() {
            Some(OotKind::Message) => {
                for key in ["arg1", "arg2", "arg3", "arg4"] {
                    validate_decl_ref(&attrs, key, &local_types, true, &mut diagnostics);
                }
            }
            Some(OotKind::Alarm) => {
                validate_decl_ref(&attrs, "cause", &local_types, false, &mut diagnostics);
            }
            Some(OotKind::Condition) => {
                validate_condition_parent_ref(&attrs, &local_kinds, &mut diagnostics);
                validate_decl_ref_with(
                    &attrs,
                    "by",
                    &local_types,
                    is_string_type_name,
                    "STRING",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "reason",
                    &local_types,
                    is_string_type_name,
                    "STRING",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "seconds",
                    &local_types,
                    is_udint_type_name,
                    "UDINT",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "comment",
                    &local_types,
                    is_string_type_name,
                    "STRING",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "new-priority",
                    &local_types,
                    is_uint_type_name,
                    "UINT",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "previous-priority",
                    &local_types,
                    is_uint_type_name,
                    "UINT",
                    &mut diagnostics,
                );
            }
            Some(OotKind::Batch) => {
                validate_decl_ref_with(
                    &attrs,
                    "batchid",
                    &local_types,
                    is_udint_type_name,
                    "UDINT",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "recipe",
                    &local_types,
                    is_udint_type_name,
                    "UDINT",
                    &mut diagnostics,
                );
            }
            Some(OotKind::RecipeLoaded) => {
                validate_decl_ref_with(
                    &attrs,
                    "recipe",
                    &local_types,
                    is_udint_type_name,
                    "UDINT",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "version",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "batch",
                    &local_types,
                    is_udint_type_name,
                    "UDINT",
                    &mut diagnostics,
                );
            }
            Some(OotKind::RecipeApproved) => {
                validate_decl_ref_with(
                    &attrs,
                    "recipe",
                    &local_types,
                    is_udint_type_name,
                    "UDINT",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "version",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
                validate_auth_binding(&attrs, "auth", &local_types, &mut diagnostics);
                validate_decl_ref_with(
                    &attrs,
                    "by",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
            }
            Some(OotKind::MaterialAddition) => {
                validate_decl_ref_with(
                    &attrs,
                    "batch",
                    &local_types,
                    is_udint_type_name,
                    "UDINT",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "material",
                    &local_types,
                    is_udint_type_name,
                    "UDINT",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "quantity",
                    &local_types,
                    is_lreal_type_name,
                    "LREAL",
                    &mut diagnostics,
                );
            }
            Some(OotKind::OperatorAction) => {
                validate_decl_ref_with(
                    &attrs,
                    "action",
                    &local_types,
                    is_udint_type_name,
                    "UDINT",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "actor",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
                for key in ["context1", "context2", "context3", "context4"] {
                    validate_decl_ref_with(
                        &attrs,
                        key,
                        &local_types,
                        is_udint_type_name,
                        "UDINT",
                        &mut diagnostics,
                    );
                }
                validate_auth_binding(&attrs, "auth", &local_types, &mut diagnostics);
                validate_decl_ref_with(
                    &attrs,
                    "workstation",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
            }
            Some(OotKind::OperatorLogin) => {
                validate_decl_ref_with(
                    &attrs,
                    "actor",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
                validate_auth_binding(&attrs, "auth", &local_types, &mut diagnostics);
                validate_decl_ref_with(
                    &attrs,
                    "workstation",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "role",
                    &local_types,
                    is_uint_type_name,
                    "UINT",
                    &mut diagnostics,
                );
            }
            Some(OotKind::OperatorLogout) => {
                validate_decl_ref_with(
                    &attrs,
                    "actor",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "workstation",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
            }
            Some(OotKind::SecurityFailure) => {
                validate_decl_ref_with(
                    &attrs,
                    "actor",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "workstation",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
                validate_decl_ref_with(
                    &attrs,
                    "reason",
                    &local_types,
                    is_string_96_type_name,
                    "STRING[<=96]",
                    &mut diagnostics,
                );
            }
            _ => {}
        }
    }
    diagnostics
}

fn validate_condition_parent_ref(
    attrs: &AttributeMap,
    local_kinds: &BTreeMap<String, OotKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entry) = last_entry(attrs, "of") else {
        return;
    };
    match local_kinds.get(&entry.value.to_ascii_lowercase()) {
        Some(OotKind::Alarm) => {}
        Some(other) => diagnostics.push(openot_error(
            entry.range,
            format!(
                "OpenOT condition 'of' must reference an alarm, got '{}'",
                other.as_str()
            ),
        )),
        None => diagnostics.push(openot_error(
            entry.range,
            format!("OpenOT 'of' references unknown variable '{}'", entry.value),
        )),
    }
}

fn validate_decl_ref(
    attrs: &AttributeMap,
    key: &str,
    local_types: &BTreeMap<String, String>,
    require_value_encodable: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entry) = last_entry(attrs, key) else {
        return;
    };
    let Some(st_type) = local_types.get(&entry.value.to_ascii_lowercase()) else {
        diagnostics.push(openot_error(
            entry.range,
            format!(
                "OpenOT '{key}' references unknown variable '{}'",
                entry.value
            ),
        ));
        return;
    };
    if require_value_encodable && !is_supported_value_type_name(st_type) {
        diagnostics.push(openot_error(
            entry.range,
            format!(
                "OpenOT '{key}' references unsupported argument type '{}'",
                st_type
            ),
        ));
    }
}

fn validate_decl_ref_with(
    attrs: &AttributeMap,
    key: &str,
    local_types: &BTreeMap<String, String>,
    predicate: fn(&str) -> bool,
    expected: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entry) = last_entry(attrs, key) else {
        return;
    };
    let Some(st_type) = local_types.get(&entry.value.to_ascii_lowercase()) else {
        diagnostics.push(openot_error(
            entry.range,
            format!(
                "OpenOT '{key}' references unknown variable '{}'",
                entry.value
            ),
        ));
        return;
    };
    if !predicate(st_type) {
        diagnostics.push(openot_error(
            entry.range,
            format!("OpenOT '{key}' references type '{st_type}', expected {expected}"),
        ));
    }
}

fn validate_auth_binding(
    attrs: &AttributeMap,
    key: &str,
    local_types: &BTreeMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entry) = last_entry(attrs, key) else {
        return;
    };
    if auth_result_code(&entry.value).is_some() {
        return;
    }
    let Some(st_type) = local_types.get(&entry.value.to_ascii_lowercase()) else {
        diagnostics.push(openot_error(
            entry.range,
            format!(
                "OpenOT '{key}' must be an authResult symbol or reference a UINT variable, got '{}'",
                entry.value
            ),
        ));
        return;
    };
    if !is_uint_type_name(st_type) {
        diagnostics.push(openot_error(
            entry.range,
            format!("OpenOT '{key}' references type '{st_type}', expected UINT"),
        ));
    }
}

fn local_declared_types(program: &SyntaxNode) -> BTreeMap<String, String> {
    let mut types = BTreeMap::new();
    for var_decl in program
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
    {
        let Some(type_ref) = var_decl
            .children()
            .find(|child| child.kind() == SyntaxKind::TypeRef)
        else {
            continue;
        };
        let st_type = compact_node_text(&type_ref);
        for name in declaration_names(&var_decl) {
            types.insert(name.text.to_ascii_lowercase(), st_type.clone());
        }
    }
    types
}

fn local_openot_kinds(program: &SyntaxNode) -> BTreeMap<String, OotKind> {
    let mut kinds = BTreeMap::new();
    for var_decl in program
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
    {
        let attrs = parse_attribute_map_from_node(&var_decl);
        let Some(kind) = attrs.kind() else {
            continue;
        };
        for name in declaration_names(&var_decl) {
            kinds.insert(name.text.to_ascii_lowercase(), kind);
        }
    }
    kinds
}

/// Collect OpenOT diagnostics that need resolved HIR symbols.
#[must_use]
pub fn collect_openot_semantic_diagnostics(
    root: &SyntaxNode,
    symbols: &SymbolTable,
    catalog: &DeclarationCatalog,
    file_id: FileId,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for var_decl in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
    {
        let attrs = parse_attribute_map_from_node(&var_decl);
        match attrs.kind() {
            Some(OotKind::State) => {
                let Some(model_entry) = last_entry(&attrs, "model") else {
                    continue;
                };
                let Some(category) =
                    last_entry(&attrs, "category").and_then(|entry| category_code(&entry.value))
                else {
                    continue;
                };
                if category != STATE_CATEGORY_PROCEDURAL {
                    continue;
                }
                let Some(model_states) = procedural_model_states(&model_entry.value) else {
                    continue;
                };
                for name in declaration_names(&var_decl) {
                    let Some(declaration) =
                        find_declaration(catalog, file_id, name.range, &name.text)
                    else {
                        continue;
                    };
                    let resolved = symbols.resolve_alias_type(declaration.type_id());
                    let Some(Type::Enum { values, .. }) = symbols.type_by_id(resolved) else {
                        continue;
                    };
                    for (variant_name, raw_value) in values {
                        let Ok(value) = u16::try_from(*raw_value) else {
                            diagnostics.push(openot_error(
                                model_entry.range,
                                format!(
                                    "OpenOT procedural model '{}' state '{}' has out-of-range value {}",
                                    model_entry.value, variant_name, raw_value
                                ),
                            ));
                            continue;
                        };
                        if !model_states.iter().any(|(expected_value, expected_label)| {
                            *expected_value == value && *expected_label == variant_name.as_str()
                        }) {
                            diagnostics.push(openot_error(
                                model_entry.range,
                                format!(
                                    "OpenOT procedural model '{}' does not define state '{} := {}'",
                                    model_entry.value, variant_name, value
                                ),
                            ));
                        }
                    }
                }
            }
            Some(OotKind::Batch) => {
                let range = last_entry(&attrs, OOT_KEY)
                    .map_or_else(|| TextRange::empty(0.into()), |entry| entry.range);
                for name in declaration_names(&var_decl) {
                    let Some(declaration) =
                        find_declaration(catalog, file_id, name.range, &name.text)
                    else {
                        continue;
                    };
                    let resolved = symbols.resolve_alias_type(declaration.type_id());
                    let Some(Type::Enum { values, .. }) = symbols.type_by_id(resolved) else {
                        diagnostics.push(openot_error(
                            range,
                            "OpenOT batch logging requires an enum matching batchState",
                        ));
                        continue;
                    };
                    for (variant_name, raw_value) in values {
                        let Ok(value) = u16::try_from(*raw_value) else {
                            diagnostics.push(openot_error(
                                range,
                                format!(
                                    "OpenOT batchState '{}' has out-of-range value {}",
                                    variant_name, raw_value
                                ),
                            ));
                            continue;
                        };
                        if !batch_state_values()
                            .iter()
                            .any(|(expected_value, expected_label)| {
                                *expected_value == value && *expected_label == variant_name.as_str()
                            })
                        {
                            diagnostics.push(openot_error(
                                range,
                                format!(
                                    "OpenOT batchState does not define state '{} := {}'",
                                    variant_name, value
                                ),
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
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
                    "OpenOT value logging supports BOOL, integer, REAL, LREAL, and bounded STRING types only",
                ));
            }
        }
    }
    if kind == OotKind::Condition {
        if let Some(ty) = declared_type {
            if !ty.trim().eq_ignore_ascii_case("BOOL") {
                diagnostics.push(openot_error(
                    oot_entry.range,
                    "OpenOT condition lifecycle commands must be BOOL variables",
                ));
            }
        }
    }
    if matches!(
        kind,
        OotKind::RecipeLoaded
            | OotKind::RecipeApproved
            | OotKind::MaterialAddition
            | OotKind::OperatorAction
            | OotKind::OperatorLogin
            | OotKind::OperatorLogout
            | OotKind::SecurityFailure
    ) {
        if let Some(ty) = declared_type {
            if !ty.trim().eq_ignore_ascii_case("BOOL") {
                diagnostics.push(openot_error(
                    oot_entry.range,
                    format!(
                        "OpenOT {} events must be BOOL trigger variables",
                        kind.as_str()
                    ),
                ));
            }
        }
    }

    let allowed = allowed_keys(kind);
    let allowed_set = allowed.iter().copied().collect::<BTreeSet<_>>();
    for entry in attrs.entries.iter().filter(|entry| entry.key != OOT_KEY) {
        if kind == OotKind::Condition && INTERNAL_ID_KEYS.contains(&entry.key.as_str()) {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "OpenOT condition lifecycle inherits the parent alarm identity; '{}' is not allowed",
                    entry.key
                ),
            ));
            continue;
        }
        if matches!(
            kind,
            OotKind::Batch
                | OotKind::RecipeLoaded
                | OotKind::RecipeApproved
                | OotKind::MaterialAddition
                | OotKind::OperatorAction
                | OotKind::OperatorLogin
                | OotKind::OperatorLogout
                | OotKind::SecurityFailure
        ) && INTERNAL_ID_KEYS.contains(&entry.key.as_str())
            && entry.key != "sourceid"
        {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "OpenOT {} events use bound field identities; '{}' is not allowed",
                    kind.as_str(),
                    entry.key
                ),
            ));
            continue;
        }
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
    if kind == OotKind::Value {
        validate_value_sampling(attrs, &mut diagnostics);
    }
    if kind == OotKind::Condition {
        validate_condition_lifecycle(attrs, &mut diagnostics);
    }
    match kind {
        OotKind::Batch => validate_required_keys(attrs, &["batchid"], "batch", &mut diagnostics),
        OotKind::RecipeLoaded => validate_required_keys(
            attrs,
            &["recipe", "version"],
            "recipe-loaded",
            &mut diagnostics,
        ),
        OotKind::RecipeApproved => validate_required_keys(
            attrs,
            &["recipe", "version"],
            "recipe-approved",
            &mut diagnostics,
        ),
        OotKind::MaterialAddition => validate_required_keys(
            attrs,
            &["batch", "material", "quantity"],
            "material-addition",
            &mut diagnostics,
        ),
        OotKind::OperatorAction => validate_required_keys(
            attrs,
            &["action", "actor"],
            "operator-action",
            &mut diagnostics,
        ),
        OotKind::OperatorLogin => {
            validate_required_keys(
                attrs,
                &["actor", "auth"],
                "operator-login",
                &mut diagnostics,
            );
        }
        OotKind::OperatorLogout => {
            validate_required_keys(attrs, &["actor"], "operator-logout", &mut diagnostics);
        }
        OotKind::SecurityFailure => {
            validate_required_keys(attrs, &["actor"], "security-failure", &mut diagnostics);
        }
        _ => {}
    }

    diagnostics
}

/// Whether the OpenOT ST producer has a faithful value encoder for this built-in type.
#[must_use]
pub fn is_supported_value_type_id(type_id: TypeId) -> bool {
    matches!(
        type_id,
        TypeId::BOOL
            | TypeId::SINT
            | TypeId::USINT
            | TypeId::INT
            | TypeId::UINT
            | TypeId::DINT
            | TypeId::UDINT
            | TypeId::LINT
            | TypeId::ULINT
            | TypeId::REAL
            | TypeId::LREAL
            | TypeId::STRING
    )
}

/// Whether the OpenOT ST producer has a faithful value encoder for this declared type name.
#[must_use]
pub fn is_supported_value_type_name(ty: &str) -> bool {
    let trimmed = ty.trim();
    if trimmed.to_ascii_uppercase().starts_with("STRING[") {
        return true;
    }
    TypeId::from_builtin_name(trimmed).is_some_and(is_supported_value_type_id)
}

fn is_string_type_name(ty: &str) -> bool {
    let upper = ty.trim().to_ascii_uppercase();
    upper == "STRING" || upper.starts_with("STRING[")
}

fn is_string_96_type_name(ty: &str) -> bool {
    let upper = ty.trim().to_ascii_uppercase();
    if upper == "STRING" {
        return true;
    }
    let Some(length) = upper
        .strip_prefix("STRING[")
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|digits| digits.trim().parse::<usize>().ok())
    else {
        return false;
    };
    length <= 96
}

fn is_udint_type_name(ty: &str) -> bool {
    ty.trim().eq_ignore_ascii_case("UDINT")
}

fn is_uint_type_name(ty: &str) -> bool {
    ty.trim().eq_ignore_ascii_case("UINT")
}

fn is_lreal_type_name(ty: &str) -> bool {
    ty.trim().eq_ignore_ascii_case("LREAL")
}

/// Keys accepted by a kind, excluding `oot` and internal id-pinning keys.
#[must_use]
pub fn allowed_keys(kind: OotKind) -> &'static [&'static str] {
    match kind {
        OotKind::Value => VALUE_KEYS,
        OotKind::State => STATE_KEYS,
        OotKind::Alarm => ALARM_KEYS,
        OotKind::Message => MESSAGE_KEYS,
        OotKind::Condition => CONDITION_KEYS,
        OotKind::Batch => BATCH_KEYS,
        OotKind::RecipeLoaded => RECIPE_LOADED_KEYS,
        OotKind::RecipeApproved => RECIPE_APPROVED_KEYS,
        OotKind::MaterialAddition => MATERIAL_ADDITION_KEYS,
        OotKind::OperatorAction => OPERATOR_ACTION_KEYS,
        OotKind::OperatorLogin => OPERATOR_LOGIN_KEYS,
        OotKind::OperatorLogout => OPERATOR_LOGOUT_KEYS,
        OotKind::SecurityFailure => SECURITY_FAILURE_KEYS,
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

/// Event id for a supported condition lifecycle command.
#[must_use]
pub fn condition_lifecycle_event_id(value: &str) -> Option<u32> {
    match value.to_ascii_lowercase().as_str() {
        "acknowledge" => Some(0x0202),
        "confirm" => Some(0x0203),
        "shelve" => Some(0x0204),
        "unshelve" => Some(0x0205),
        "suppress" => Some(0x0206),
        "unsuppress" => Some(0x0207),
        "out-of-service" => Some(0x0208),
        "in-service" => Some(0x0209),
        "comment" => Some(0x020A),
        "reset" => Some(0x020B),
        "priority-changed" => Some(0x020C),
        _ => None,
    }
}

/// Numeric code for an authentication result.
#[must_use]
pub fn auth_result_code(value: &str) -> Option<u16> {
    match value.to_ascii_lowercase().as_str() {
        "granted" => Some(0),
        "denied" => Some(1),
        "notrequired" | "not-required" | "not_required" => Some(2),
        "pending" => Some(3),
        "expired" => Some(4),
        _ => value.parse::<u16>().ok().filter(|code| *code <= 4),
    }
}

/// Numeric code for a value quality.
#[must_use]
pub fn quality_code(value: &str) -> Option<u16> {
    match value.to_ascii_lowercase().as_str() {
        "good" => Some(0),
        "uncertain" => Some(1),
        "bad" => Some(2),
        "unknown" => Some(3),
        _ => value.parse::<u16>().ok().filter(|code| *code <= 3),
    }
}

/// Numeric code for a value semantic role.
#[must_use]
pub fn semantic_role_code(value: &str) -> Option<u16> {
    match value.to_ascii_lowercase().as_str() {
        "actual" => Some(0),
        "setpoint" => Some(1),
        "command" => Some(2),
        "count" => Some(3),
        "position" => Some(4),
        "status" => Some(5),
        _ => value.parse::<u16>().ok().filter(|code| *code <= 5),
    }
}

/// Canonical sampling policy spelling.
#[must_use]
pub fn sampling_policy(value: &str) -> Option<&'static str> {
    match value.to_ascii_lowercase().as_str() {
        "on-change" => Some("on-change"),
        "deadband" => Some("deadband"),
        "periodic" => Some("periodic"),
        "hysteresis" => Some("hysteresis"),
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
        (OotKind::State, "model")
            if !MODEL_VALUES
                .iter()
                .any(|value| value.eq_ignore_ascii_case(&entry.value)) =>
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
        (OotKind::Condition, "event") if condition_lifecycle_event_id(&entry.value).is_none() => {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "unknown OpenOT condition event '{}'; expected {}",
                    entry.value,
                    format_allowed(CONDITION_EVENT_VALUES)
                ),
            ));
        }
        (OotKind::Alarm, "severity") | (OotKind::Message, "severity") => match entry
            .value
            .parse::<u16>()
        {
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
        (OotKind::Value, "quality") if quality_code(&entry.value).is_none() => {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "unknown OpenOT quality '{}'; expected {} or 0..=3",
                    entry.value,
                    format_allowed(QUALITY_VALUES)
                ),
            ));
        }
        (OotKind::Value, "semanticrole") if semantic_role_code(&entry.value).is_none() => {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "unknown OpenOT semanticRole '{}'; expected {} or 0..=5",
                    entry.value,
                    format_allowed(SEMANTIC_ROLE_VALUES)
                ),
            ));
        }
        (OotKind::Value, "previous")
            if !matches!(
                entry.value.to_ascii_lowercase().as_str(),
                "true" | "false" | "yes" | "no"
            ) =>
        {
            diagnostics.push(openot_error(
                entry.range,
                "OpenOT previous must be 'true' or 'false'",
            ));
        }
        (OotKind::Value, "unit") if entry.value.trim().is_empty() => {
            diagnostics.push(openot_error(
                entry.range,
                "OpenOT 'unit' value must not be empty",
            ));
        }
        (OotKind::Value, "unit")
            if !UNIT_VALUES
                .iter()
                .any(|value| value.eq_ignore_ascii_case(&entry.value)) =>
        {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "unknown OpenOT unit '{}'; expected {}",
                    entry.value,
                    format_allowed(UNIT_VALUES)
                ),
            ));
        }
        (OotKind::MaterialAddition, "unit") if entry.value.trim().is_empty() => {
            diagnostics.push(openot_error(
                entry.range,
                "OpenOT 'unit' value must not be empty",
            ));
        }
        (OotKind::MaterialAddition, "unit")
            if !UNIT_VALUES
                .iter()
                .any(|value| value.eq_ignore_ascii_case(&entry.value)) =>
        {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "unknown OpenOT unit '{}'; expected {}",
                    entry.value,
                    format_allowed(UNIT_VALUES)
                ),
            ));
        }
        (OotKind::Value, "sampling") if sampling_policy(&entry.value).is_none() => {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "unknown OpenOT sampling '{}'; expected {}",
                    entry.value,
                    format_allowed(SAMPLING_VALUES)
                ),
            ));
        }
        (OotKind::Value, "interval") => match entry.value.parse::<u64>() {
            Ok(value) if value > 0 => {}
            _ => diagnostics.push(openot_error(
                entry.range,
                "OpenOT interval must be a positive integer number of milliseconds",
            )),
        },
        (OotKind::Message, "template") if entry.value.trim().is_empty() => {
            diagnostics.push(openot_error(
                entry.range,
                format!("OpenOT '{}' value must not be empty", entry.key),
            ));
        }
        (
            OotKind::Condition,
            "of" | "by" | "seconds" | "reason" | "comment" | "new-priority" | "previous-priority",
        ) if entry.value.trim().is_empty() => {
            diagnostics.push(openot_error(
                entry.range,
                format!("OpenOT '{}' value must name a variable", entry.key),
            ));
        }
        (OotKind::Batch, "batchid" | "recipe")
        | (OotKind::RecipeLoaded, "recipe" | "version" | "batch")
        | (OotKind::RecipeApproved, "recipe" | "version" | "auth" | "by")
        | (OotKind::MaterialAddition, "batch" | "material" | "quantity")
        | (
            OotKind::OperatorAction,
            "action" | "actor" | "context1" | "context2" | "context3" | "context4" | "auth"
            | "workstation",
        )
        | (OotKind::OperatorLogin, "actor" | "auth" | "workstation" | "role")
        | (OotKind::OperatorLogout, "actor" | "workstation")
        | (OotKind::SecurityFailure, "actor" | "workstation" | "reason")
            if entry.value.trim().is_empty() =>
        {
            diagnostics.push(openot_error(
                entry.range,
                format!("OpenOT '{}' value must name a variable", entry.key),
            ));
        }
        (OotKind::Message, "arg1" | "arg2" | "arg3" | "arg4") | (OotKind::Alarm, "cause")
            if entry.value.trim().is_empty() =>
        {
            diagnostics.push(openot_error(
                entry.range,
                format!("OpenOT '{}' value must name a variable", entry.key),
            ));
        }
        _ => {}
    }
}

fn validate_condition_lifecycle(attrs: &AttributeMap, diagnostics: &mut Vec<Diagnostic>) {
    let range = attrs
        .entries
        .iter()
        .find(|entry| entry.key == OOT_KEY)
        .map_or_else(|| TextRange::empty(0.into()), |entry| entry.range);
    for key in ["of", "event"] {
        if last_entry(attrs, key).is_none() {
            diagnostics.push(openot_error(
                range,
                format!("OpenOT condition lifecycle requires '{key}'"),
            ));
        }
    }

    let Some(event_entry) = last_entry(attrs, "event") else {
        return;
    };
    let Some(allowed_keys) = condition_event_allowed_keys(&event_entry.value) else {
        return;
    };

    for entry in attrs.entries.iter().filter(|entry| entry.key != OOT_KEY) {
        if INTERNAL_ID_KEYS.contains(&entry.key.as_str()) {
            continue;
        }
        if CONDITION_KEYS.contains(&entry.key.as_str())
            && !allowed_keys.contains(&entry.key.as_str())
        {
            diagnostics.push(openot_error(
                entry.range,
                format!(
                    "OpenOT condition event '{}' does not allow '{}'",
                    event_entry.value, entry.key
                ),
            ));
        }
    }

    for required in condition_event_required_keys(&event_entry.value) {
        if last_entry(attrs, required).is_none() {
            diagnostics.push(openot_error(
                event_entry.range,
                format!(
                    "OpenOT condition event '{}' requires '{}'",
                    event_entry.value, required
                ),
            ));
        }
    }
}

fn validate_required_keys(
    attrs: &AttributeMap,
    required_keys: &[&str],
    kind: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let range = attrs
        .entries
        .iter()
        .find(|entry| entry.key == OOT_KEY)
        .map_or_else(|| TextRange::empty(0.into()), |entry| entry.range);
    for key in required_keys {
        if last_entry(attrs, key).is_none() {
            diagnostics.push(openot_error(
                range,
                format!("OpenOT {kind} requires '{key}'"),
            ));
        }
    }
}

fn condition_event_allowed_keys(event: &str) -> Option<&'static [&'static str]> {
    match event.to_ascii_lowercase().as_str() {
        "acknowledge" | "confirm" | "reset" | "out-of-service" => Some(&["of", "event", "by"]),
        "shelve" => Some(&["of", "event", "by", "seconds"]),
        "suppress" => Some(&["of", "event", "reason"]),
        "unshelve" | "unsuppress" | "in-service" => Some(&["of", "event"]),
        "comment" => Some(&["of", "event", "comment", "by"]),
        "priority-changed" => Some(&["of", "event", "new-priority", "previous-priority", "by"]),
        _ => None,
    }
}

fn condition_event_required_keys(event: &str) -> &'static [&'static str] {
    match event.to_ascii_lowercase().as_str() {
        "comment" => &["comment"],
        "priority-changed" => &["new-priority", "previous-priority"],
        _ => &[],
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

fn validate_value_sampling(attrs: &AttributeMap, diagnostics: &mut Vec<Diagnostic>) {
    let sampling_entry = last_entry(attrs, "sampling");
    let sampling = sampling_entry.and_then(|entry| sampling_policy(&entry.value));
    let deadband_entry = last_entry(attrs, "deadband");
    let interval_entry = last_entry(attrs, "interval");

    match sampling {
        Some("periodic") if interval_entry.is_none() => {
            if let Some(entry) = sampling_entry {
                diagnostics.push(openot_error(
                    entry.range,
                    "OpenOT sampling := 'periodic' requires an 'interval' in milliseconds",
                ));
            }
        }
        Some("hysteresis") if deadband_entry.is_none() => {
            if let Some(entry) = sampling_entry {
                diagnostics.push(openot_error(
                    entry.range,
                    "OpenOT sampling := 'hysteresis' requires a REAL 'deadband'",
                ));
            }
        }
        Some("deadband") if deadband_entry.is_none() => {
            if let Some(entry) = sampling_entry {
                diagnostics.push(openot_error(
                    entry.range,
                    "OpenOT sampling := 'deadband' requires a REAL 'deadband'",
                ));
            }
        }
        _ => {}
    }

    if let Some(entry) = interval_entry {
        if sampling != Some("periodic") {
            diagnostics.push(openot_error(
                entry.range,
                "OpenOT 'interval' is only valid with sampling := 'periodic'",
            ));
        }
    }
}

fn last_entry<'a>(attrs: &'a AttributeMap, key: &str) -> Option<&'a AttributeEntry> {
    attrs.entries.iter().rev().find(|entry| entry.key == key)
}

fn procedural_model_states(model: &str) -> Option<&'static [(u16, &'static str)]> {
    match model.to_ascii_lowercase().as_str() {
        "isa-88" => Some(ISA_88_STATES),
        "packml" => Some(PACKML_STATES),
        _ => None,
    }
}

fn batch_state_values() -> &'static [(u16, &'static str)] {
    &[
        (0, "Started"),
        (1, "Completed"),
        (2, "Held"),
        (3, "Resumed"),
        (4, "Aborted"),
        (5, "Paused"),
    ]
}

fn declaration_names(var_decl: &SyntaxNode) -> Vec<NameInfo> {
    let mut names = Vec::new();
    for child in var_decl.children() {
        match child.kind() {
            SyntaxKind::Name => {
                if let Some(name) = name_info(&child) {
                    names.push(name);
                }
            }
            SyntaxKind::TypeRef => break,
            _ => {}
        }
    }
    names
}

fn name_info(node: &SyntaxNode) -> Option<NameInfo> {
    let token = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| {
            matches!(
                token.kind(),
                SyntaxKind::Ident | SyntaxKind::KwEn | SyntaxKind::KwEno | SyntaxKind::KwStep
            )
        })?;
    Some(NameInfo {
        text: token.text().to_string(),
        range: token.text_range(),
    })
}

fn find_declaration<'a>(
    catalog: &'a DeclarationCatalog,
    file_id: FileId,
    range: TextRange,
    name: &str,
) -> Option<&'a crate::semantic::DeclarationRecord> {
    catalog.entries().iter().find(|entry| {
        matches!(
            entry.kind(),
            DeclarationKind::Variable | DeclarationKind::Constant | DeclarationKind::Parameter
        ) && entry.source().file_id() == file_id
            && entry.source().range() == range
            && entry
                .qualified_name()
                .parts()
                .last()
                .is_some_and(|part| part.eq_ignore_ascii_case(name))
    })
}

#[derive(Debug, Clone)]
struct NameInfo {
    text: String,
    range: TextRange,
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
    Pressure : REAL {attribute 'oot' := 'value', 'sampling' := 'periodic', 'interval' := '1000'};
    Flow : REAL {attribute 'oot' := 'value', 'sampling' := 'hysteresis', 'deadband' := '1.5'};
    BatchCount : DINT {attribute 'oot' := 'value'};
    Enabled : BOOL {attribute 'oot' := 'value'};
    Total : ULINT {attribute 'oot' := 'value'};
    Ratio : LREAL {attribute 'oot' := 'value'};
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
    fn rejects_unknown_value_unit() {
        let source = r#"
PROGRAM Main
VAR
    Level : REAL {attribute 'oot' := 'value', 'unit' := 'banana'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown OpenOT unit")));
    }

    #[test]
    fn validates_value_sampling_policy_rules() {
        let source = r#"
PROGRAM Main
VAR
    Unknown : REAL {attribute 'oot' := 'value', 'sampling' := 'banana'};
    MissingInterval : REAL {attribute 'oot' := 'value', 'sampling' := 'periodic'};
    MissingDeadband : REAL {attribute 'oot' := 'value', 'sampling' := 'hysteresis'};
    DeadbandWithoutValue : REAL {attribute 'oot' := 'value', 'sampling' := 'deadband'};
    IntervalWithoutPeriodic : REAL {attribute 'oot' := 'value', 'interval' := '1000'};
    BadInterval : REAL {attribute 'oot' := 'value', 'sampling' := 'periodic', 'interval' := '0'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        for expected in [
            "unknown OpenOT sampling",
            "requires an 'interval'",
            "requires a REAL 'deadband'",
            "requires a REAL 'deadband'",
            "only valid with sampling := 'periodic'",
            "positive integer",
        ] {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "missing {expected}; got {diagnostics:#?}"
            );
        }
    }

    #[test]
    fn validates_message_args_and_alarm_cause_references() {
        let source = r#"
PROGRAM Main
VAR
    Level : REAL {attribute 'oot' := 'value'};
    Count : DINT {attribute 'oot' := 'value'};
    Alarm : BOOL {attribute 'oot' := 'alarm', 'cause' := 'Level'};
    Started : BOOL {attribute 'oot' := 'message', 'arg1' := 'Level', 'arg2' := 'Count', 'severity' := '500'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn validates_condition_lifecycle_references() {
        let source = r#"
PROGRAM Main
    VAR
        OperatorName : STRING[32];
        ReasonText : STRING[32];
        ShelveSecs : UDINT;
        CommentText : STRING[32];
        PreviousPriority : UINT;
        NewPriority : UINT;
        HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'class' := 'alarm'};
        AckHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'acknowledge', 'by' := OperatorName};
        ConfirmHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'confirm', 'by' := OperatorName};
        ShelveHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'shelve', 'by' := OperatorName, 'seconds' := ShelveSecs};
        UnshelveHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'unshelve'};
        SuppressHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'suppress', 'reason' := ReasonText};
        UnsuppressHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'unsuppress'};
        OosHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'out-of-service', 'by' := OperatorName};
        InServiceHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'in-service'};
        ResetHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'reset', 'by' := OperatorName};
        CommentHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'comment', 'comment' := CommentText, 'by' := OperatorName};
        PriorityHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'priority-changed', 'previous-priority' := PreviousPriority, 'new-priority' := NewPriority, 'by' := OperatorName};
    END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn validates_batch_recipe_references() {
        let source = r#"
TYPE BatchState : (Started := 0, Completed := 1, Held := 2, Resumed := 3, Aborted := 4, Paused := 5) END_TYPE
PROGRAM Main
VAR
    RecipeId : UDINT;
    RecipeVersion : STRING[16];
    BatchId : UDINT;
    MaterialId : UDINT;
    Quantity : LREAL;
    AuthResult : UINT;
    Approver : STRING[32];
    State : BatchState {attribute 'oot' := 'batch', 'batchId' := BatchId, 'recipe' := RecipeId};
    Loaded : BOOL {attribute 'oot' := 'recipe-loaded', 'recipe' := RecipeId, 'version' := RecipeVersion, 'batch' := BatchId};
    Approved : BOOL {attribute 'oot' := 'recipe-approved', 'recipe' := RecipeId, 'version' := RecipeVersion, 'auth' := AuthResult, 'by' := Approver};
    Addition : BOOL {attribute 'oot' := 'material-addition', 'batch' := BatchId, 'material' := MaterialId, 'quantity' := Quantity, 'unit' := 'kg'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn validates_operator_regulated_references() {
        let source = r#"
PROGRAM Main
VAR
    ActionId : UDINT;
    ContextA : UDINT;
    ContextB : UDINT;
    OperatorName : STRING[32];
    Workstation : STRING[32];
    Role : UINT;
    Auth : UINT;
    ReasonText : STRING[32];
    Action : BOOL {attribute 'oot' := 'operator-action', 'action' := ActionId, 'actor' := OperatorName, 'context1' := ContextA, 'context2' := ContextB, 'auth' := 'Granted', 'workstation' := Workstation};
    Login : BOOL {attribute 'oot' := 'operator-login', 'actor' := OperatorName, 'auth' := Auth, 'workstation' := Workstation, 'role' := Role};
    Logout : BOOL {attribute 'oot' := 'operator-logout', 'actor' := OperatorName, 'workstation' := Workstation};
    Failure : BOOL {attribute 'oot' := 'security-failure', 'actor' := OperatorName, 'workstation' := Workstation, 'reason' := ReasonText};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn rejects_invalid_batch_recipe_attributes() {
        let source = r#"
PROGRAM Main
VAR
    RecipeId : UDINT;
    RecipeVersionWide : STRING[128];
    BatchId : UDINT;
    MaterialId : UDINT;
    Quantity : LREAL;
    BadQuantity : REAL;
    BadAuth : DINT;
    BadBy : DINT;
    MissingBatchId : INT {attribute 'oot' := 'batch'};
    MissingVersion : BOOL {attribute 'oot' := 'recipe-loaded', 'recipe' := RecipeId};
    OversizeVersion : BOOL {attribute 'oot' := 'recipe-approved', 'recipe' := RecipeId, 'version' := RecipeVersionWide};
    BadAuthType : BOOL {attribute 'oot' := 'recipe-approved', 'recipe' := RecipeId, 'version' := RecipeVersionWide, 'auth' := BadAuth, 'by' := BadBy};
    BadQuantityType : BOOL {attribute 'oot' := 'material-addition', 'batch' := BatchId, 'material' := MaterialId, 'quantity' := BadQuantity};
    InapplicableReason : BOOL {attribute 'oot' := 'recipe-loaded', 'recipe' := RecipeId, 'version' := RecipeVersionWide, 'reason' := BadBy};
    DeferredEffectiveTime : BOOL {attribute 'oot' := 'recipe-loaded', 'recipe' := RecipeId, 'version' := RecipeVersionWide, 'effectiveTime' := BatchId};
    DeferredCorrection : BOOL {attribute 'oot' := 'material-addition', 'batch' := BatchId, 'material' := MaterialId, 'quantity' := Quantity, 'correctionOf' := BatchId};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        for expected in [
            "OpenOT batch requires 'batchid'",
            "OpenOT recipe-loaded requires 'version'",
            "expected STRING[<=96]",
            "expected UINT",
            "expected STRING[<=96]",
            "expected LREAL",
            "unknown OpenOT key 'reason' for kind 'recipe-loaded'",
            "unknown OpenOT key 'effectivetime' for kind 'recipe-loaded'",
            "unknown OpenOT key 'correctionof' for kind 'material-addition'",
        ] {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "missing {expected}; got {diagnostics:#?}"
            );
        }
    }

    #[test]
    fn rejects_invalid_operator_regulated_attributes() {
        let source = r#"
PROGRAM Main
VAR
    ActionId : UDINT;
    BadContext : DINT;
    BadActor : STRING[128];
    Workstation : STRING[32];
    BadRole : UDINT;
    ReasonText : STRING[32];
    MissingAuth : BOOL {attribute 'oot' := 'operator-login', 'actor' := BadActor};
    BadContextType : BOOL {attribute 'oot' := 'operator-action', 'action' := ActionId, 'actor' := BadActor, 'context1' := BadContext};
    BadRoleType : BOOL {attribute 'oot' := 'operator-login', 'actor' := BadActor, 'auth' := 'Granted', 'role' := BadRole};
    InapplicableReason : BOOL {attribute 'oot' := 'operator-action', 'action' := ActionId, 'actor' := BadActor, 'reason' := ReasonText};
    ProgramDownload : BOOL {attribute 'oot' := 'program-download', 'actor' := BadActor};
    BadId : BOOL {attribute 'oot' := 'operator-logout', 'actor' := BadActor, 'valueid' := '7'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        for expected in [
            "OpenOT operator-login requires 'auth'",
            "expected STRING[<=96]",
            "expected UDINT",
            "expected UINT",
            "unknown OpenOT key 'reason' for kind 'operator-action'",
            "unknown OpenOT kind 'program-download'",
            "use bound field identities; 'valueid' is not allowed",
        ] {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "missing {expected}; got {diagnostics:#?}"
            );
        }
    }

    #[test]
    fn rejects_batch_enum_that_is_not_batch_state() {
        use crate::db::SemanticDatabase;
        use crate::{Project, SourceKey};

        let source = r#"
TYPE BatchState : (Started := 0, Filling := 1) END_TYPE
PROGRAM Main
VAR
    BatchId : UDINT;
    State : BatchState {attribute 'oot' := 'batch', 'batchId' := BatchId};
END_VAR
END_PROGRAM
"#;
        let mut project = Project::new();
        let file_id = project.set_source_text(SourceKey::from_virtual("main.st"), source.into());
        let diagnostics = project.database().diagnostics(file_id);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::InvalidOpenOtAttribute
                    && diagnostic
                        .message
                        .contains("OpenOT batchState does not define state 'Filling := 1'")
            }),
            "{diagnostics:#?}"
        );
    }

    #[test]
    fn rejects_invalid_condition_lifecycle_attributes() {
        let source = r#"
PROGRAM Main
VAR
    OperatorId : DINT;
    OperatorName : STRING[32];
    ReasonCode : DINT;
    CommentCode : DINT;
    PriorityCode : DINT;
    HighPhAlarm : BOOL {attribute 'oot' := 'alarm'};
    NotAlarm : BOOL {attribute 'oot' := 'message'};
    MissingParent : BOOL {attribute 'oot' := 'condition', 'event' := 'acknowledge'};
    MissingEvent : BOOL {attribute 'oot' := 'condition', 'of' := NotAlarm};
    BadEvent : BOOL {attribute 'oot' := 'condition', 'of' := NotAlarm, 'event' := 'bad-event'};
    BadParent : BOOL {attribute 'oot' := 'condition', 'of' := NotAlarm, 'event' := 'acknowledge'};
    BadBy : BOOL {attribute 'oot' := 'condition', 'of' := NotAlarm, 'event' := 'acknowledge', 'by' := OperatorId};
    BadReason : BOOL {attribute 'oot' := 'condition', 'of' := NotAlarm, 'event' := 'suppress', 'reason' := ReasonCode};
    BadSeconds : BOOL {attribute 'oot' := 'condition', 'of' := NotAlarm, 'event' := 'shelve', 'seconds' := ReasonCode};
    BadCommentType : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'comment', 'comment' := CommentCode};
    BadPriorityType : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'priority-changed', 'previous-priority' := PriorityCode, 'new-priority' := PriorityCode};
    MissingComment : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'comment'};
    MissingNewPriority : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'priority-changed', 'previous-priority' := OperatorId};
    MissingPreviousPriority : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'priority-changed', 'new-priority' := OperatorId};
    InapplicableReason : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'comment', 'comment' := OperatorName, 'reason' := OperatorName};
    BadType : DINT {attribute 'oot' := 'condition', 'of' := NotAlarm, 'event' := 'acknowledge'};
    BadId : BOOL {attribute 'oot' := 'condition', 'of' := NotAlarm, 'event' := 'acknowledge', 'sourceid' := '2'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        for expected in [
            "requires 'of'",
            "requires 'event'",
            "unknown OpenOT condition event",
            "must reference an alarm",
            "expected STRING",
            "expected STRING",
            "expected UDINT",
            "expected STRING",
            "expected UINT",
            "requires 'comment'",
            "requires 'new-priority'",
            "requires 'previous-priority'",
            "does not allow 'reason'",
            "must be BOOL",
            "inherits the parent alarm identity",
        ] {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "missing {expected}; got {diagnostics:#?}"
            );
        }
    }

    #[test]
    fn rejects_unknown_message_arg_reference() {
        let source = r#"
PROGRAM Main
VAR
    Started : BOOL {attribute 'oot' := 'message', 'arg1' := 'Missing'};
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let diagnostics = collect_openot_attribute_diagnostics(&parsed.syntax());
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("references unknown variable")));
    }

    #[test]
    fn rejects_value_attributes_for_unsupported_value_types() {
        let source = r#"
PROGRAM Main
VAR
    WideText : WSTRING {attribute 'oot' := 'value'};
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
                    .contains("OpenOT value logging supports BOOL")
            })
            .count();
        assert_eq!(unsupported, 1, "{diagnostics:#?}");
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

    #[test]
    fn rejects_procedural_model_enum_states_that_are_not_canonical() {
        use crate::db::SemanticDatabase;
        use crate::{Project, SourceKey};

        let source = r#"
TYPE Phase : (Idle := 0, Filling := 1) END_TYPE
PROGRAM Main
VAR
    Step : Phase {attribute 'oot' := 'state', 'category' := 'procedural', 'model' := 'ISA-88'};
END_VAR
END_PROGRAM
"#;
        let mut project = Project::new();
        let file_id = project.set_source_text(SourceKey::from_virtual("main.st"), source.into());

        let diagnostics = project.database().diagnostics(file_id);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::InvalidOpenOtAttribute
                    && diagnostic
                        .message
                        .contains("does not define state 'Filling := 1'")
            }),
            "{diagnostics:#?}"
        );
    }

    #[test]
    fn accepts_canonical_procedural_model_enum_states() {
        use crate::db::SemanticDatabase;
        use crate::{Project, SourceKey};

        let source = r#"
TYPE Phase : (Idle := 0, Running := 1, Complete := 2) END_TYPE
PROGRAM Main
VAR
    Step : Phase {attribute 'oot' := 'state', 'category' := 'procedural', 'model' := 'ISA-88'};
END_VAR
END_PROGRAM
"#;
        let mut project = Project::new();
        let file_id = project.set_source_text(SourceKey::from_virtual("main.st"), source.into());

        let diagnostics = project.database().diagnostics(file_id);
        assert!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == DiagnosticCode::InvalidOpenOtAttribute)
                .count()
                == 0,
            "{diagnostics:#?}"
        );
    }
}
