use std::collections::BTreeMap;
use std::path::Path;

use text_size::TextRange;
use trust_hir::db::FileId;
use trust_hir::openot_authoring as hir_openot;
use trust_hir::openot_authoring::OotKind;
use trust_hir::semantic::{DeclarationCatalog, DeclarationKind};
use trust_hir::{Type, TypeId};
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::definition::canonical_unit_id;
use super::instrumentation::id_or_default;
use super::types::*;
use super::DEFAULT_STATE_CATEGORY;

pub(super) fn collect_program_annotations(
    program: &SyntaxNode,
    source_path: Option<&str>,
    file_id: FileId,
    catalog: &DeclarationCatalog,
    symbols: &trust_hir::symbols::SymbolTable,
    counters: &mut AnnotationCounters,
    default_source_id: u32,
) -> Vec<Annotation> {
    let mut pending = Vec::<(AnnotationDraft, BTreeMap<String, String>)>::new();
    let mut local_types = BTreeMap::<String, String>::new();
    for var_block in program
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
    {
        for var_decl in var_block
            .children()
            .filter(|child| child.kind() == SyntaxKind::VarDecl)
        {
            let Some(type_ref) = var_decl
                .children()
                .find(|child| child.kind() == SyntaxKind::TypeRef)
            else {
                continue;
            };
            let st_type = type_ref_text(&type_ref);
            for name in declaration_names(&var_decl) {
                local_types.insert(name.text.to_ascii_lowercase(), st_type.clone());
            }
        }
    }
    let source_descriptor = source_descriptor(source_path, program);
    for var_block in program
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
    {
        for var_decl in var_block
            .children()
            .filter(|child| child.kind() == SyntaxKind::VarDecl)
        {
            let attrs = hir_openot::parse_attribute_map_from_node(&var_decl).to_btree_map();
            let Some(kind) = attrs.get("oot").and_then(|value| OotKind::parse(value)) else {
                continue;
            };
            let Some(type_ref) = var_decl
                .children()
                .find(|child| child.kind() == SyntaxKind::TypeRef)
            else {
                continue;
            };
            let st_type = type_ref_text(&type_ref);
            let initializer = var_decl
                .children()
                .find(|child| is_expression_kind(child.kind()))
                .map(|expr| compact_source_text(&expr));
            let source_id = attrs
                .get("sourceid")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(default_source_id);

            for name in declaration_names(&var_decl) {
                let declaration =
                    find_declaration(catalog, file_id, name.range, name.text.as_str());
                let enum_state = if matches!(kind, OotKind::State | OotKind::Batch) {
                    declaration.and_then(|entry| {
                        resolve_enum_state(symbols, entry.type_id(), st_type.as_str())
                    })
                } else {
                    None
                };
                pending.push((
                    AnnotationDraft {
                        kind,
                        var_name: name.text,
                        st_type: st_type.clone(),
                        initializer: initializer.clone(),
                        source_id,
                        source: source_descriptor.clone(),
                        enum_state,
                        message_args: message_args_from_attrs(&attrs, &local_types),
                        cause_operand: cause_operand_from_attrs(&attrs),
                        condition_parent: attrs.get("of").cloned(),
                        condition_event_id: attrs
                            .get("event")
                            .and_then(|event| hir_openot::condition_lifecycle_event_id(event)),
                        condition_ack_by: attrs.get("by").cloned(),
                        condition_shelve_secs: attrs.get("seconds").cloned(),
                        condition_reason: attrs.get("reason").cloned(),
                        condition_comment: attrs.get("comment").cloned(),
                        condition_previous_priority: attrs.get("previous-priority").cloned(),
                        condition_new_priority: attrs.get("new-priority").cloned(),
                        batch_id: attrs.get("batchid").cloned(),
                        recipe_id: attrs.get("recipe").cloned(),
                        recipe_version: attrs.get("version").cloned(),
                        procedure_batch_id: attrs.get("batch").cloned(),
                        auth_result: attrs.get("auth").cloned(),
                        procedure_ack_by: attrs.get("by").cloned(),
                        material_id: attrs.get("material").cloned(),
                        quantity: attrs.get("quantity").cloned(),
                        material_unit_id: attrs
                            .get("unit")
                            .and_then(|symbol| canonical_unit_id(symbol)),
                        regulated_action_id: attrs.get("action").cloned(),
                        regulated_actor: attrs.get("actor").cloned(),
                        regulated_context_refs: context_refs_from_attrs(&attrs),
                        regulated_workstation: attrs.get("workstation").cloned(),
                        regulated_role: attrs.get("role").cloned(),
                        regulated_reason: attrs.get("reason").cloned(),
                        signature_action_id: attrs.get("action").cloned(),
                        signature_actor: attrs.get("actor").cloned(),
                        signature_meaning: attrs
                            .get("meaning")
                            .and_then(|meaning| hir_openot::signature_meaning_code(meaning)),
                        signature_attests: attrs.get("attests").cloned(),
                    },
                    attrs.clone(),
                ));
            }
        }
    }

    let attestable_ids = assign_attestable_ids(&pending);
    let mut annotations = Vec::new();
    let mut index = BTreeMap::<String, AnnotationIndexEntry>::new();
    for (draft, attrs) in &pending {
        if draft.kind == OotKind::Condition {
            continue;
        }
        let attestable_id = attestable_ids
            .get(&draft.var_name.to_ascii_lowercase())
            .copied()
            .unwrap_or(0);
        let signature_attests_id = draft
            .signature_attests
            .as_ref()
            .and_then(|name| attestable_ids.get(&name.to_ascii_lowercase()).copied())
            .unwrap_or(0);
        let annotation = annotation_from_parts(
            draft.clone(),
            attrs,
            counters,
            attestable_id,
            signature_attests_id,
        );
        index.insert(
            annotation.var_name.to_ascii_lowercase(),
            AnnotationIndexEntry {
                kind: annotation.kind,
                id: annotation.id,
                source_id: annotation.source_id,
                attestable_id: annotation.attestable_id,
            },
        );
        annotations.push(annotation);
    }

    for (draft, _) in pending {
        if draft.kind != OotKind::Condition {
            continue;
        }
        let Some(parent_name) = draft.condition_parent.as_ref() else {
            continue;
        };
        let Some(parent) = index.get(&parent_name.to_ascii_lowercase()) else {
            continue;
        };
        if parent.kind != OotKind::Alarm {
            continue;
        }
        annotations.push(condition_annotation_from_parts(draft, parent));
    }
    annotations
}

fn assign_attestable_ids(
    pending: &[(AnnotationDraft, BTreeMap<String, String>)],
) -> BTreeMap<String, u32> {
    let mut ids = BTreeMap::new();
    let mut next_id = 1u32;
    for (draft, _) in pending {
        if draft.kind != OotKind::ESignature {
            continue;
        }
        let Some(target) = draft.signature_attests.as_ref() else {
            continue;
        };
        let key = target.to_ascii_lowercase();
        if ids.contains_key(&key) || next_id > 32 {
            continue;
        }
        ids.insert(key, next_id);
        next_id += 1;
    }
    ids
}

fn source_descriptor(source_path: Option<&str>, program: &SyntaxNode) -> SourceDescriptor {
    let program_name = program_name(program).unwrap_or_else(|| "Program".to_string());
    let file_stem = source_path
        .and_then(|path| {
            Path::new(path)
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
        })
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "source".to_string());
    let name = format!("{file_stem}.{program_name}");
    SourceDescriptor {
        name,
        path: vec![file_stem, program_name],
        hierarchy: vec!["file".to_string(), "program".to_string()],
    }
}

fn program_name(program: &SyntaxNode) -> Option<String> {
    program
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map(|node| node_name_text(&node))
        .filter(|name| !name.is_empty())
}

fn message_args_from_attrs(
    attrs: &BTreeMap<String, String>,
    local_types: &BTreeMap<String, String>,
) -> Vec<MessageArgInfo> {
    let mut args = Vec::new();
    for key in ["arg1", "arg2", "arg3", "arg4"] {
        let Some(name) = attrs.get(key) else {
            continue;
        };
        let Some(st_type) = local_types.get(&name.to_ascii_lowercase()) else {
            continue;
        };
        args.push(MessageArgInfo {
            name: name.clone(),
            st_type: st_type.clone(),
        });
    }
    args
}

fn context_refs_from_attrs(attrs: &BTreeMap<String, String>) -> Vec<String> {
    ["context1", "context2", "context3", "context4"]
        .into_iter()
        .filter_map(|key| attrs.get(key).cloned())
        .collect()
}

fn cause_operand_from_attrs(attrs: &BTreeMap<String, String>) -> Option<CauseOperandInfo> {
    attrs.get("cause").map(|name| CauseOperandInfo {
        operand_id: 1,
        name: name.clone(),
    })
}

fn annotation_from_parts(
    draft: AnnotationDraft,
    attrs: &BTreeMap<String, String>,
    counters: &mut AnnotationCounters,
    attestable_id: u32,
    signature_attests_id: u32,
) -> Annotation {
    match draft.kind {
        OotKind::Value => {
            counters.values = counters.values.saturating_add(1);
            Annotation {
                kind: draft.kind,
                var_name: draft.var_name,
                st_type: draft.st_type,
                initializer: draft.initializer,
                id: id_or_default(attrs, 2000 + counters.values),
                source_id: draft.source_id,
                source: draft.source,
                deadband: attrs.get("deadband").cloned(),
                sampling: attrs
                    .get("sampling")
                    .and_then(|value| hir_openot::sampling_policy(value))
                    .map(str::to_string),
                interval_ms: attrs.get("interval").and_then(|value| value.parse().ok()),
                unit: attrs.get("unit").cloned(),
                quality: attrs
                    .get("quality")
                    .and_then(|value| hir_openot::quality_code(value)),
                semantic_role: attrs
                    .get("semanticrole")
                    .and_then(|value| hir_openot::semantic_role_code(value))
                    .unwrap_or(0),
                suppress_previous: attrs.get("previous").is_some_and(|value| {
                    matches!(value.to_ascii_lowercase().as_str(), "false" | "no")
                }),
                category: 0,
                model: None,
                condition_class: 0,
                severity: 0,
                message: None,
                message_severity: None,
                message_args: Vec::new(),
                cause_operand: None,
                enum_state: draft.enum_state,
                condition_event_id: None,
                condition_ack_by: None,
                condition_shelve_secs: None,
                condition_reason: None,
                condition_comment: None,
                condition_previous_priority: None,
                condition_new_priority: None,
                batch_id: None,
                recipe_id: None,
                recipe_version: None,
                procedure_batch_id: None,
                auth_result: draft.auth_result,
                procedure_ack_by: None,
                material_id: None,
                quantity: None,
                material_unit_id: None,
                regulated_action_id: None,
                regulated_actor: draft.regulated_actor,
                regulated_context_refs: Vec::new(),
                regulated_workstation: None,
                regulated_role: None,
                regulated_reason: draft.regulated_reason,
                signature_action_id: None,
                signature_actor: None,
                signature_meaning: None,
                signature_attests_id,
                attestable_id,
            }
        }
        OotKind::State => {
            counters.states = counters.states.saturating_add(1);
            Annotation {
                kind: draft.kind,
                var_name: draft.var_name,
                st_type: draft.st_type,
                initializer: draft.initializer,
                id: id_or_default(attrs, 7000 + counters.states),
                source_id: draft.source_id,
                source: draft.source,
                deadband: None,
                sampling: None,
                interval_ms: None,
                unit: None,
                quality: None,
                semantic_role: 0,
                suppress_previous: false,
                category: attrs
                    .get("category")
                    .map_or(DEFAULT_STATE_CATEGORY, |value| {
                        hir_openot::category_code(value).unwrap_or(DEFAULT_STATE_CATEGORY)
                    }),
                model: attrs.get("model").cloned(),
                condition_class: 0,
                severity: 0,
                message: None,
                message_severity: None,
                message_args: Vec::new(),
                cause_operand: None,
                enum_state: draft.enum_state,
                condition_event_id: None,
                condition_ack_by: None,
                condition_shelve_secs: None,
                condition_reason: None,
                condition_comment: None,
                condition_previous_priority: None,
                condition_new_priority: None,
                batch_id: None,
                recipe_id: None,
                recipe_version: None,
                procedure_batch_id: None,
                auth_result: None,
                procedure_ack_by: None,
                material_id: None,
                quantity: None,
                material_unit_id: None,
                regulated_action_id: None,
                regulated_actor: None,
                regulated_context_refs: Vec::new(),
                regulated_workstation: None,
                regulated_role: None,
                regulated_reason: None,
                signature_action_id: None,
                signature_actor: None,
                signature_meaning: None,
                signature_attests_id,
                attestable_id,
            }
        }
        OotKind::Alarm => {
            counters.alarms = counters.alarms.saturating_add(1);
            Annotation {
                kind: draft.kind,
                var_name: draft.var_name,
                st_type: draft.st_type,
                initializer: draft.initializer,
                id: id_or_default(attrs, 9000 + counters.alarms),
                source_id: draft.source_id,
                source: draft.source,
                deadband: None,
                sampling: None,
                interval_ms: None,
                unit: None,
                quality: None,
                semantic_role: 0,
                suppress_previous: false,
                category: 0,
                model: None,
                condition_class: attrs.get("class").map_or(0, |value| {
                    hir_openot::condition_class_code(value).unwrap_or(0)
                }),
                severity: attrs
                    .get("severity")
                    .and_then(|value| value.parse::<u16>().ok())
                    .unwrap_or(800),
                message: None,
                message_severity: None,
                message_args: Vec::new(),
                cause_operand: draft.cause_operand,
                enum_state: draft.enum_state,
                condition_event_id: None,
                condition_ack_by: None,
                condition_shelve_secs: None,
                condition_reason: None,
                condition_comment: None,
                condition_previous_priority: None,
                condition_new_priority: None,
                batch_id: None,
                recipe_id: None,
                recipe_version: None,
                procedure_batch_id: None,
                auth_result: None,
                procedure_ack_by: None,
                material_id: None,
                quantity: None,
                material_unit_id: None,
                regulated_action_id: None,
                regulated_actor: None,
                regulated_context_refs: Vec::new(),
                regulated_workstation: None,
                regulated_role: None,
                regulated_reason: None,
                signature_action_id: None,
                signature_actor: None,
                signature_meaning: None,
                signature_attests_id,
                attestable_id,
            }
        }
        OotKind::Message => {
            counters.messages = counters.messages.saturating_add(1);
            Annotation {
                kind: draft.kind,
                var_name: draft.var_name,
                st_type: draft.st_type,
                initializer: draft.initializer,
                id: id_or_default(attrs, 10_000 + counters.messages),
                source_id: draft.source_id,
                source: draft.source,
                deadband: None,
                sampling: None,
                interval_ms: None,
                unit: None,
                quality: None,
                semantic_role: 0,
                suppress_previous: false,
                category: 0,
                model: None,
                condition_class: 0,
                severity: 0,
                message: attrs.get("template").cloned(),
                message_severity: attrs
                    .get("severity")
                    .and_then(|value| value.parse::<u16>().ok()),
                message_args: draft.message_args,
                cause_operand: None,
                enum_state: draft.enum_state,
                condition_event_id: None,
                condition_ack_by: None,
                condition_shelve_secs: None,
                condition_reason: None,
                condition_comment: None,
                condition_previous_priority: None,
                condition_new_priority: None,
                batch_id: None,
                recipe_id: None,
                recipe_version: None,
                procedure_batch_id: None,
                auth_result: None,
                procedure_ack_by: None,
                material_id: None,
                quantity: None,
                material_unit_id: None,
                regulated_action_id: None,
                regulated_actor: None,
                regulated_context_refs: Vec::new(),
                regulated_workstation: None,
                regulated_role: None,
                regulated_reason: None,
                signature_action_id: None,
                signature_actor: None,
                signature_meaning: None,
                signature_attests_id,
                attestable_id,
            }
        }
        OotKind::Batch
        | OotKind::RecipeLoaded
        | OotKind::RecipeApproved
        | OotKind::MaterialAddition
        | OotKind::OperatorAction
        | OotKind::OperatorLogin
        | OotKind::OperatorLogout
        | OotKind::SecurityFailure
        | OotKind::ESignature => Annotation {
            kind: draft.kind,
            var_name: draft.var_name,
            st_type: draft.st_type,
            initializer: draft.initializer,
            id: 0,
            source_id: draft.source_id,
            source: draft.source,
            deadband: None,
            sampling: None,
            interval_ms: None,
            unit: None,
            quality: None,
            semantic_role: 0,
            suppress_previous: false,
            category: 0,
            model: None,
            condition_class: 0,
            severity: 0,
            message: None,
            message_severity: None,
            message_args: Vec::new(),
            cause_operand: None,
            enum_state: draft.enum_state,
            condition_event_id: None,
            condition_ack_by: None,
            condition_shelve_secs: None,
            condition_reason: None,
            condition_comment: None,
            condition_previous_priority: None,
            condition_new_priority: None,
            batch_id: draft.batch_id,
            recipe_id: draft.recipe_id,
            recipe_version: draft.recipe_version,
            procedure_batch_id: draft.procedure_batch_id,
            auth_result: draft.auth_result,
            procedure_ack_by: draft.procedure_ack_by,
            material_id: draft.material_id,
            quantity: draft.quantity,
            material_unit_id: draft.material_unit_id,
            regulated_action_id: draft.regulated_action_id,
            regulated_actor: draft.regulated_actor,
            regulated_context_refs: draft.regulated_context_refs,
            regulated_workstation: draft.regulated_workstation,
            regulated_role: draft.regulated_role,
            regulated_reason: draft.regulated_reason,
            signature_action_id: draft.signature_action_id,
            signature_actor: draft.signature_actor,
            signature_meaning: draft.signature_meaning,
            signature_attests_id,
            attestable_id,
        },
        OotKind::Condition => {
            unreachable!("condition annotations require a resolved parent alarm index")
        }
    }
}

fn condition_annotation_from_parts(
    draft: AnnotationDraft,
    parent: &AnnotationIndexEntry,
) -> Annotation {
    let _parent_attestable_id = parent.attestable_id;
    Annotation {
        kind: draft.kind,
        var_name: draft.var_name,
        st_type: draft.st_type,
        initializer: draft.initializer,
        id: parent.id,
        source_id: parent.source_id,
        source: draft.source,
        deadband: None,
        sampling: None,
        interval_ms: None,
        unit: None,
        quality: None,
        semantic_role: 0,
        suppress_previous: false,
        category: 0,
        model: None,
        condition_class: 0,
        severity: 0,
        message: None,
        message_severity: None,
        message_args: Vec::new(),
        cause_operand: None,
        enum_state: draft.enum_state,
        condition_event_id: draft.condition_event_id,
        condition_ack_by: draft.condition_ack_by,
        condition_shelve_secs: draft.condition_shelve_secs,
        condition_reason: draft.condition_reason,
        condition_comment: draft.condition_comment,
        condition_previous_priority: draft.condition_previous_priority,
        condition_new_priority: draft.condition_new_priority,
        batch_id: None,
        recipe_id: None,
        recipe_version: None,
        procedure_batch_id: None,
        auth_result: None,
        procedure_ack_by: None,
        material_id: None,
        quantity: None,
        material_unit_id: None,
        regulated_action_id: None,
        regulated_actor: None,
        regulated_context_refs: Vec::new(),
        regulated_workstation: None,
        regulated_role: None,
        regulated_reason: None,
        signature_action_id: None,
        signature_actor: None,
        signature_meaning: None,
        signature_attests_id: 0,
        attestable_id: 0,
    }
}

fn find_declaration<'a>(
    catalog: &'a DeclarationCatalog,
    file_id: FileId,
    range: TextRange,
    name: &str,
) -> Option<&'a trust_hir::semantic::DeclarationRecord> {
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

fn resolve_enum_state(
    symbols: &trust_hir::symbols::SymbolTable,
    type_id: TypeId,
    declared_type: &str,
) -> Option<EnumStateInfo> {
    let resolved = symbols.resolve_alias_type(type_id);
    let Type::Enum { name, values, .. } = symbols.type_by_id(resolved)? else {
        return None;
    };
    let mut variants = Vec::with_capacity(values.len());
    for (variant_name, value) in values {
        let value = u16::try_from(*value).ok()?;
        variants.push(EnumVariantInfo {
            name: variant_name.to_string(),
            value,
        });
    }
    (!variants.is_empty()).then(|| EnumStateInfo {
        type_name: declared_type.to_string(),
        enum_set_name: name.to_string(),
        variants,
    })
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

fn type_ref_text(type_ref: &SyntaxNode) -> String {
    for child in type_ref.children() {
        if matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName) {
            return node_name_text(&child);
        }
    }
    for token in type_ref
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if let Some(name) = builtin_type_name(token.kind()) {
            return name.to_string();
        }
    }
    compact_source_text(type_ref)
}

fn node_name_text(node: &SyntaxNode) -> String {
    let mut text = String::new();
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            SyntaxKind::Ident
            | SyntaxKind::Dot
            | SyntaxKind::KwEn
            | SyntaxKind::KwEno
            | SyntaxKind::KwStep => text.push_str(token.text()),
            _ => {}
        }
    }
    text
}

fn compact_source_text(node: &SyntaxNode) -> String {
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

fn is_expression_kind(kind: SyntaxKind) -> bool {
    kind.is_expression_node() || kind.is_initializer_expression_node()
}

fn builtin_type_name(kind: SyntaxKind) -> Option<&'static str> {
    match kind {
        SyntaxKind::KwBool => Some("BOOL"),
        SyntaxKind::KwSInt => Some("SINT"),
        SyntaxKind::KwInt => Some("INT"),
        SyntaxKind::KwDInt => Some("DINT"),
        SyntaxKind::KwLInt => Some("LINT"),
        SyntaxKind::KwUSInt => Some("USINT"),
        SyntaxKind::KwUInt => Some("UINT"),
        SyntaxKind::KwUDInt => Some("UDINT"),
        SyntaxKind::KwULInt => Some("ULINT"),
        SyntaxKind::KwByte => Some("BYTE"),
        SyntaxKind::KwWord => Some("WORD"),
        SyntaxKind::KwDWord => Some("DWORD"),
        SyntaxKind::KwLWord => Some("LWORD"),
        SyntaxKind::KwReal => Some("REAL"),
        SyntaxKind::KwLReal => Some("LREAL"),
        SyntaxKind::KwString => Some("STRING"),
        _ => None,
    }
}

pub(super) fn text_size_to_usize(value: text_size::TextSize) -> usize {
    u32::from(value) as usize
}
