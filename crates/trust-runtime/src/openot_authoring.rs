//! OpenOT attribute authoring support.
//!
//! This module implements the first compiler-side lowering target for
//! declaration-adjacent `{attribute 'oot' := ...}` pragmas. The user source
//! remains pure ST; the compile session instruments hidden `OPENOT_Producer`
//! calls before bytecode is built.

use crate::harness::SourceFile;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use text_size::TextRange;
use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::openot_authoring as hir_openot;
use trust_hir::openot_authoring::OotKind;
use trust_hir::semantic::{DeclarationCatalog, DeclarationKind};
use trust_hir::{Project, SourceKey, Type, TypeId};
use trust_syntax::parser;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

const DEFAULT_SOURCE_ID: u32 = 1;
const PRODUCER_NAME: &str = "OotProducer";
const USE_SOURCE_TIME_NAME: &str = "OotUseSourceTimeInput";
const SOURCE_TIME_NAME: &str = "OotSourceTime";

/// Name of the hidden producer instance generated for attributed programs.
pub const GENERATED_PRODUCER_NAME: &str = PRODUCER_NAME;
/// Name of the hidden boolean that enables host-supplied source timestamps.
pub const GENERATED_USE_SOURCE_TIME_NAME: &str = USE_SOURCE_TIME_NAME;
/// Name of the hidden ULINT source timestamp in Unix nanoseconds.
pub const GENERATED_SOURCE_TIME_NAME: &str = SOURCE_TIME_NAME;

/// Instrument source files that contain OpenOT declaration attributes.
///
/// Files without OpenOT attributes are returned unchanged. The lowering is
/// intentionally conservative in this slice: it supports simple scalar
/// declarations inside `PROGRAM ... VAR ... END_VAR` blocks.
#[must_use]
pub fn instrument_source_files(sources: &[SourceFile]) -> Vec<SourceFile> {
    let model = collect_authoring_model(sources);
    sources
        .iter()
        .enumerate()
        .map(|(idx, source)| SourceFile {
            path: source.path.clone(),
            text: instrument_source_text(&source.text, &model.files[idx].programs),
        })
        .collect()
}

/// Generate a deterministic definition-file JSON document from OpenOT
/// declaration attributes.
///
/// The `contentHash` field is SHA-256 over the generated JSON with an empty
/// `contentHash`, encoded as lowercase hex.
pub fn definition_json_from_sources(sources: &[SourceFile]) -> Result<Value, String> {
    let validation_errors = validate_authoring_sources(sources);
    if !validation_errors.is_empty() {
        return Err(validation_errors.join("; "));
    }

    let annotations = collect_authoring_model(sources).annotations;
    if annotations.is_empty() {
        return Err("no OpenOT declaration attributes found".to_string());
    }

    let mut source_ids = BTreeSet::new();
    let mut values = Vec::new();
    let mut states = Vec::new();
    let mut conditions = Vec::new();
    let mut messages = Vec::new();
    let mut units_by_symbol = BTreeMap::<String, u16>::new();
    let mut enum_sets = BTreeMap::<String, Value>::new();

    for annotation in &annotations {
        source_ids.insert(annotation.source_id);
        match annotation.kind {
            OotKind::Value => {
                let unit = annotation.unit.as_ref().map(|symbol| {
                    if let Some(unit_id) = units_by_symbol.get(symbol) {
                        *unit_id
                    } else {
                        let unit_id =
                            u16::try_from(units_by_symbol.len() + 1).expect("unit id fits u16");
                        units_by_symbol.insert(symbol.clone(), unit_id);
                        unit_id
                    }
                });
                values.push(json!({
                    "valueId": annotation.id,
                    "name": annotation.var_name,
                    "dataType": annotation.tlv_type(),
                    "semanticRole": 0,
                    "unit": unit,
                    "deadband": annotation.deadband.as_ref().map(|decimal| json!({
                        "decimal": decimal,
                        "scaled": null
                    })),
                    "samplingPolicy": if annotation.deadband.is_some() { Value::Null } else { json!("on-change") }
                }));
            }
            OotKind::State => {
                let enum_set = annotation.enum_state.as_ref().map_or_else(
                    || format!("{}States", annotation.var_name),
                    |state| state.enum_set_name.clone(),
                );
                if let Some(enum_state) = &annotation.enum_state {
                    enum_sets
                        .entry(enum_state.enum_set_name.clone())
                        .or_insert_with(|| enum_state.definition_json());
                }
                states.push(json!({
                    "stateMachineId": annotation.id,
                    "name": annotation.var_name,
                    "category": annotation.category,
                    "proceduralModel": annotation.model,
                    "enumSet": enum_set
                }));
            }
            OotKind::Alarm => {
                conditions.push(json!({
                    "conditionId": annotation.id,
                    "name": annotation.var_name,
                    "conditionClass": annotation.condition_class,
                    "defaultSeverity": annotation.severity,
                    "causeOperands": []
                }));
            }
            OotKind::Message => {
                messages.push(json!({
                    "messageTemplateId": annotation.id,
                    "name": annotation.var_name,
                    "format": annotation
                        .message
                        .clone()
                        .unwrap_or_else(|| annotation.var_name.clone()),
                    "argTypes": []
                }));
            }
        }
    }

    let units = units_by_symbol
        .iter()
        .map(|(symbol, unit_id)| {
            json!({
                "unitId": unit_id,
                "symbol": symbol
            })
        })
        .collect::<Vec<_>>();

    let sources_json = source_ids
        .iter()
        .map(|source_id| {
            json!({
                "sourceId": source_id,
                "name": format!("source{source_id}"),
                "path": [format!("source{source_id}")],
                "hierarchy": [],
                "dynamic": false
            })
        })
        .collect::<Vec<_>>();

    let mut definition = json!({
        "header": {
            "wireVersion": 2,
            "semanticVersion": "1.0.0",
            "profiles": ["Core", "Full"],
            "conformanceLevel": "Producer-Full",
            "caps": {
                "crc": true,
                "sourceHighWater": true
            },
            "constraints": {
                "maxRecordSize": 512,
                "maxSlots": 16,
                "overflowPolicy": "overwrite-oldest"
            },
            "epochStrategy": "retain",
            "contentHash": ""
        },
        "eventTypes": [
            state_transition_event_type(),
            value_changed_event_type(),
            message_event_type(),
            condition_event_type(0x0200, "ConditionActive"),
            condition_event_type(0x0201, "ConditionCleared"),
            source_high_water_event_type()
        ],
        "sources": sources_json,
        "stateMachines": states,
        "conditions": conditions,
        "messageTemplates": messages,
        "values": values,
        "units": units,
        "enumSets": enum_sets.into_values().collect::<Vec<_>>(),
        "severityScale": {
            "name": "baseline",
            "low": { "min": 1, "max": 332 },
            "medium": { "min": 333, "max": 666 },
            "high": { "min": 667, "max": 1000 }
        }
    });

    let canonical = serde_json::to_vec(&definition).map_err(|err| err.to_string())?;
    let digest = Sha256::digest(canonical);
    let hash = hex_lower(&digest);
    definition["header"]["contentHash"] = json!(hash);
    Ok(definition)
}

fn validate_authoring_sources(sources: &[SourceFile]) -> Vec<String> {
    let mut errors = Vec::new();
    for (idx, source) in sources.iter().enumerate() {
        let parse = parser::parse(&source.text);
        for diagnostic in hir_openot::collect_openot_attribute_diagnostics(&parse.syntax()) {
            let label = source
                .path
                .as_deref()
                .map_or_else(|| format!("source {}", idx + 1), ToString::to_string);
            errors.push(format!("{label}: {}", diagnostic.message));
        }
    }
    errors
}

fn collect_authoring_model(sources: &[SourceFile]) -> AuthoringModel {
    let mut project = Project::new();
    let mut file_ids = Vec::with_capacity(sources.len());
    for (idx, source) in sources.iter().enumerate() {
        let key = match source.path.as_deref() {
            Some(path) => SourceKey::from_path(Path::new(path)),
            None => SourceKey::from_virtual(format!("openot_authoring_{idx}")),
        };
        file_ids.push(project.set_source_text(key, source.text.clone()));
    }

    let analyses = file_ids
        .iter()
        .map(|file_id| project.database().analyze(*file_id))
        .collect::<Vec<_>>();

    let mut counters = AnnotationCounters::default();
    let mut files = Vec::with_capacity(sources.len());
    let mut all_annotations = Vec::new();

    for (idx, source) in sources.iter().enumerate() {
        let parse = parser::parse(&source.text);
        if !parse.ok() {
            files.push(FileAuthoringModel::default());
            continue;
        }

        let syntax = parse.syntax();
        let analysis = analyses[idx].as_ref();
        let mut programs = Vec::new();
        for program in syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Program)
        {
            let annotations = collect_program_annotations(
                &program,
                file_ids[idx],
                analysis.declaration_catalog.as_ref(),
                analysis.symbols.as_ref(),
                &mut counters,
            );
            if annotations.is_empty() {
                continue;
            }
            let range = program.text_range();
            let program_model = ProgramInstrumentation {
                start: text_size_to_usize(range.start()),
                end: text_size_to_usize(range.end()),
                annotations: annotations.clone(),
            };
            all_annotations.extend(annotations);
            programs.push(program_model);
        }
        files.push(FileAuthoringModel { programs });
    }

    AuthoringModel {
        files,
        annotations: all_annotations,
    }
}

fn instrument_source_text(text: &str, programs: &[ProgramInstrumentation]) -> String {
    if programs.is_empty() {
        return text.to_string();
    }

    let mut output = String::with_capacity(text.len() + 2048);
    let mut cursor = 0usize;
    let mut changed = false;

    for program in programs {
        output.push_str(&text[cursor..program.start]);
        let block = &text[program.start..program.end];
        let instrumented = instrument_program_block(block, &program.annotations);
        if instrumented != block {
            changed = true;
        }
        output.push_str(&instrumented);
        cursor = program.end;
    }
    output.push_str(&text[cursor..]);

    if changed {
        output
    } else {
        text.to_string()
    }
}

fn instrument_program_block(block: &str, annotations: &[Annotation]) -> String {
    if annotations.is_empty() {
        return block.to_string();
    }

    let lines = block.lines().collect::<Vec<_>>();
    let Some(var_end) = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case("END_VAR"))
    else {
        return block.to_string();
    };
    let Some(end_program) = lines
        .iter()
        .rposition(|line| line.trim().eq_ignore_ascii_case("END_PROGRAM"))
    else {
        return block.to_string();
    };

    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if idx == var_end {
            for declaration in hidden_declarations(annotations) {
                out.push_str(&declaration);
                out.push('\n');
            }
        }
        if idx == end_program {
            for statement in hidden_statements(annotations) {
                out.push_str(&statement);
                out.push('\n');
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn collect_program_annotations(
    program: &SyntaxNode,
    file_id: FileId,
    catalog: &DeclarationCatalog,
    symbols: &trust_hir::symbols::SymbolTable,
    counters: &mut AnnotationCounters,
) -> Vec<Annotation> {
    let mut annotations = Vec::new();
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
                .unwrap_or(DEFAULT_SOURCE_ID);

            for name in declaration_names(&var_decl) {
                let declaration =
                    find_declaration(catalog, file_id, name.range, name.text.as_str());
                let enum_state = if kind == OotKind::State {
                    declaration.and_then(|entry| {
                        resolve_enum_state(symbols, entry.type_id(), st_type.as_str())
                    })
                } else {
                    None
                };
                annotations.push(annotation_from_parts(
                    AnnotationDraft {
                        kind,
                        var_name: name.text,
                        st_type: st_type.clone(),
                        initializer: initializer.clone(),
                        source_id,
                        enum_state,
                    },
                    &attrs,
                    counters,
                ));
            }
        }
    }
    annotations
}

fn annotation_from_parts(
    draft: AnnotationDraft,
    attrs: &BTreeMap<String, String>,
    counters: &mut AnnotationCounters,
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
                deadband: attrs.get("deadband").cloned(),
                unit: attrs.get("unit").cloned(),
                category: 0,
                model: None,
                condition_class: 0,
                severity: 0,
                message: None,
                enum_state: draft.enum_state,
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
                deadband: None,
                unit: None,
                category: attrs
                    .get("category")
                    .map_or(2, |value| hir_openot::category_code(value).unwrap_or(2)),
                model: attrs.get("model").cloned(),
                condition_class: 0,
                severity: 0,
                message: None,
                enum_state: draft.enum_state,
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
                deadband: None,
                unit: None,
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
                enum_state: draft.enum_state,
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
                deadband: None,
                unit: None,
                category: 0,
                model: None,
                condition_class: 0,
                severity: 0,
                message: attrs.get("template").cloned(),
                enum_state: draft.enum_state,
            }
        }
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
        _ => None,
    }
}

fn text_size_to_usize(value: text_size::TextSize) -> usize {
    u32::from(value) as usize
}

fn hidden_declarations(annotations: &[Annotation]) -> Vec<String> {
    let mut declarations = vec![
        format!("    {PRODUCER_NAME} : OPENOT_Producer;"),
        format!("    {USE_SOURCE_TIME_NAME} : BOOL := FALSE;"),
        format!("    {SOURCE_TIME_NAME} : ULINT := ULINT#0;"),
    ];
    for annotation in annotations {
        let safe = safe_identifier(&annotation.var_name);
        match annotation.kind {
            OotKind::State => {
                if annotation.enum_state.is_some() {
                    declarations.push(format!(
                        "    OotPrev_{safe} : {} := {};",
                        annotation.st_type,
                        state_initializer(annotation)
                    ));
                    declarations.push(format!("    OotStatePrev_{safe} : UINT := UINT#0;"));
                    declarations.push(format!("    OotStateNew_{safe} : UINT := UINT#0;"));
                } else {
                    declarations.push(format!(
                        "    OotPrev_{safe} : UINT := {};",
                        state_initializer(annotation)
                    ));
                }
            }
            OotKind::Alarm | OotKind::Message => {
                declarations.push(format!("    OotPrev_{safe} : BOOL := FALSE;"))
            }
            OotKind::Value => {}
        }
    }
    declarations
}

fn hidden_statements(annotations: &[Annotation]) -> Vec<String> {
    let mut statements = vec![
        "(* Generated OpenOT attribute instrumentation. *)".to_string(),
        format!("{PRODUCER_NAME}(Execute := FALSE, ResetScanRecords := TRUE);"),
        format!("{PRODUCER_NAME}(Execute := FALSE, ResetScanRecords := FALSE);"),
    ];

    for annotation in annotations {
        match annotation.kind {
            OotKind::Value => statements.extend(value_statements(annotation)),
            OotKind::State => statements.extend(state_statements(annotation)),
            OotKind::Alarm => statements.extend(alarm_statements(annotation)),
            OotKind::Message => statements.extend(message_statements(annotation)),
        }
    }
    statements
}

fn value_statements(annotation: &Annotation) -> Vec<String> {
    let op = if annotation.is_real() { 6 } else { 7 };
    let value_arg = if annotation.is_real() {
        format!("ValueReal := {}", annotation.var_name)
    } else {
        format!("ValueInt := {}", annotation.var_name)
    };
    vec![
        format!(
            "{PRODUCER_NAME}(Execute := TRUE, Op := UINT#{op}, SourceId := UDINT#{}, {}, ValueId := UDINT#{}, {value_arg}, DeadbandReal := {}, HasQuality := FALSE, Quality := UINT#0);",
            annotation.source_id,
            source_time_args(),
            annotation.id,
            real_literal(annotation.deadband.as_deref().unwrap_or("0.0"))
        ),
        format!("{PRODUCER_NAME}(Execute := FALSE);"),
    ]
}

fn state_statements(annotation: &Annotation) -> Vec<String> {
    let safe = safe_identifier(&annotation.var_name);
    if let Some(enum_state) = &annotation.enum_state {
        let mut statements = vec![format!("IF {} <> OotPrev_{safe} THEN", annotation.var_name)];
        statements.extend(enum_state_value_statements(
            &format!("OotPrev_{safe}"),
            &format!("OotStatePrev_{safe}"),
            enum_state,
            "    ",
        ));
        statements.extend(enum_state_value_statements(
            &annotation.var_name,
            &format!("OotStateNew_{safe}"),
            enum_state,
            "    ",
        ));
        statements.push(format!(
            "    {PRODUCER_NAME}(Execute := TRUE, Op := UINT#8, SourceId := UDINT#{}, {}, StateMachineId := UDINT#{}, Category := UINT#{}, PreviousState := OotStatePrev_{safe}, NewState := OotStateNew_{safe});",
            annotation.source_id, source_time_args(), annotation.id, annotation.category
        ));
        statements.push(format!("    {PRODUCER_NAME}(Execute := FALSE);"));
        statements.push(format!("    OotPrev_{safe} := {};", annotation.var_name));
        statements.push("END_IF;".to_string());
        return statements;
    }
    vec![
        format!("IF {} <> OotPrev_{safe} THEN", annotation.var_name),
        format!(
            "    {PRODUCER_NAME}(Execute := TRUE, Op := UINT#8, SourceId := UDINT#{}, {}, StateMachineId := UDINT#{}, Category := UINT#{}, PreviousState := OotPrev_{safe}, NewState := {});",
            annotation.source_id, source_time_args(), annotation.id, annotation.category, annotation.var_name
        ),
        format!("    {PRODUCER_NAME}(Execute := FALSE);"),
        format!("    OotPrev_{safe} := {};", annotation.var_name),
        "END_IF;".to_string(),
    ]
}

fn enum_state_value_statements(
    expression: &str,
    target: &str,
    enum_state: &EnumStateInfo,
    indent: &str,
) -> Vec<String> {
    let mut statements = Vec::new();
    for (idx, variant) in enum_state.variants.iter().enumerate() {
        let keyword = if idx == 0 { "IF" } else { "ELSIF" };
        statements.push(format!(
            "{indent}{keyword} {expression} = {} THEN",
            enum_state.literal(&variant.name)
        ));
        statements.push(format!("{indent}    {target} := UINT#{};", variant.value));
    }
    statements.push(format!("{indent}END_IF;"));
    statements
}

fn alarm_statements(annotation: &Annotation) -> Vec<String> {
    let safe = safe_identifier(&annotation.var_name);
    vec![
        format!("IF {} <> OotPrev_{safe} THEN", annotation.var_name),
        format!(
            "    {PRODUCER_NAME}(Execute := TRUE, Op := UINT#9, SourceId := UDINT#{}, {}, ConditionId := UDINT#{}, ConditionClass := UINT#{}, Severity := UINT#{}, ConditionActive := {});",
            annotation.source_id,
            source_time_args(),
            annotation.id,
            annotation.condition_class,
            annotation.severity,
            annotation.var_name
        ),
        format!("    {PRODUCER_NAME}(Execute := FALSE);"),
        format!("    OotPrev_{safe} := {};", annotation.var_name),
        "END_IF;".to_string(),
    ]
}

fn message_statements(annotation: &Annotation) -> Vec<String> {
    let safe = safe_identifier(&annotation.var_name);
    vec![
        format!("IF {} AND (NOT OotPrev_{safe}) THEN", annotation.var_name),
        format!(
            "    {PRODUCER_NAME}(Execute := TRUE, Op := UINT#0, SourceId := UDINT#{}, {}, Checkpoint := FALSE, AccumulateScanRecords := TRUE);",
            annotation.source_id,
            source_time_args()
        ),
        format!("    {PRODUCER_NAME}(Execute := FALSE);"),
        "END_IF;".to_string(),
        format!("OotPrev_{safe} := {};", annotation.var_name),
    ]
}

fn source_time_args() -> String {
    format!("UseSourceTimeInput := {USE_SOURCE_TIME_NAME}, SourceTimeInput := {SOURCE_TIME_NAME}")
}

fn id_or_default(attrs: &BTreeMap<String, String>, default: u32) -> u32 {
    attrs
        .get("id")
        .or_else(|| attrs.get("valueid"))
        .or_else(|| attrs.get("machineid"))
        .or_else(|| attrs.get("statemachineid"))
        .or_else(|| attrs.get("conditionid"))
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

fn state_initializer(annotation: &Annotation) -> String {
    if let Some(enum_state) = &annotation.enum_state {
        return annotation
            .initializer
            .clone()
            .unwrap_or_else(|| enum_state.default_literal());
    }
    annotation
        .initializer
        .as_deref()
        .filter(|init| init.to_ascii_uppercase().starts_with("UINT#"))
        .unwrap_or("UINT#0")
        .to_string()
}

fn real_literal(value: &str) -> String {
    if value.to_ascii_uppercase().starts_with("REAL#") {
        value.to_string()
    } else {
        format!("REAL#{value}")
    }
}

fn safe_identifier(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn state_transition_event_type() -> Value {
    json!({
        "id": 1,
        "name": "StateTransition",
        "profile": "Core",
        "slots": [
            slot(0x0001, 0x05, 1, 1, 1),
            slot(0x0002, 0x03, 1, 1, 2),
            slot(0x0003, 0x03, 1, 1, 3),
            slot(0x0004, 0x03, 1, 1, 4)
        ]
    })
}

fn value_changed_event_type() -> Value {
    json!({
        "id": 2,
        "name": "ValueChanged",
        "profile": "Core",
        "slots": [
            slot(0x000D, 0x05, 1, 1, 1),
            slot(0x000F, 0x09, 0, 1, 2),
            slot(0x0010, 0x09, 1, 1, 3),
            slot(0x0011, 0x03, 0, 1, 4)
        ]
    })
}

fn message_event_type() -> Value {
    json!({
        "id": 3,
        "name": "Message",
        "profile": "Core",
        "slots": []
    })
}

fn condition_event_type(id: u32, name: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "profile": "Full",
        "slots": [
            slot(0x0005, 0x05, 1, 1, 1),
            slot(0x0006, 0x03, 1, 1, 2),
            slot(0x0008, 0x03, 1, 1, 3)
        ]
    })
}

fn source_high_water_event_type() -> Value {
    json!({
        "id": 0x80000108u32,
        "name": "SourceHighWater",
        "profile": "Full",
        "slots": [
            slot(0x0038, 0x07, 1, 1, 1)
        ]
    })
}

fn slot(key: u16, ty: u8, min: u16, max: u16, order: u16) -> Value {
    json!({
        "key": key,
        "type": ty,
        "minOccurs": min,
        "maxOccurs": max,
        "orderClass": order
    })
}

#[derive(Debug, Clone)]
struct Annotation {
    kind: OotKind,
    var_name: String,
    st_type: String,
    initializer: Option<String>,
    id: u32,
    source_id: u32,
    deadband: Option<String>,
    unit: Option<String>,
    category: u16,
    model: Option<String>,
    condition_class: u16,
    severity: u16,
    message: Option<String>,
    enum_state: Option<EnumStateInfo>,
}

impl Annotation {
    fn is_real(&self) -> bool {
        self.st_type.eq_ignore_ascii_case("REAL")
    }

    fn tlv_type(&self) -> u8 {
        if self.is_real() {
            0x09
        } else {
            0x06
        }
    }
}

#[derive(Debug, Clone, Default)]
struct AuthoringModel {
    files: Vec<FileAuthoringModel>,
    annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Default)]
struct FileAuthoringModel {
    programs: Vec<ProgramInstrumentation>,
}

#[derive(Debug, Clone)]
struct ProgramInstrumentation {
    start: usize,
    end: usize,
    annotations: Vec<Annotation>,
}

#[derive(Debug, Default)]
struct AnnotationCounters {
    values: u32,
    states: u32,
    alarms: u32,
    messages: u32,
}

#[derive(Debug, Clone)]
struct AnnotationDraft {
    kind: OotKind,
    var_name: String,
    st_type: String,
    initializer: Option<String>,
    source_id: u32,
    enum_state: Option<EnumStateInfo>,
}

#[derive(Debug, Clone)]
struct NameInfo {
    text: String,
    range: TextRange,
}

#[derive(Debug, Clone)]
struct EnumStateInfo {
    type_name: String,
    enum_set_name: String,
    variants: Vec<EnumVariantInfo>,
}

impl EnumStateInfo {
    fn literal(&self, variant: &str) -> String {
        format!("{}#{variant}", self.type_name)
    }

    fn default_literal(&self) -> String {
        self.variants.first().map_or_else(
            || format!("{}#0", self.type_name),
            |variant| self.literal(&variant.name),
        )
    }

    fn definition_json(&self) -> Value {
        json!({
            "name": self.enum_set_name,
            "members": self
                .variants
                .iter()
                .map(|variant| json!({
                    "value": variant.value,
                    "label": variant.name
                }))
                .collect::<Vec<_>>()
        })
    }
}

#[derive(Debug, Clone)]
struct EnumVariantInfo {
    name: String,
    value: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruments_simple_program_attributes() {
        let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Level : REAL {attribute 'oot' := 'value', 'deadband' := '0.5'};\nEND_VAR\nLevel := REAL#1.0;\nEND_PROGRAM\n",
        );

        let instrumented = instrument_source_files(&[source]);
        assert!(instrumented[0]
            .text
            .contains("OotProducer : OPENOT_Producer"));
        assert!(instrumented[0]
            .text
            .contains("OotUseSourceTimeInput : BOOL := FALSE;"));
        assert!(instrumented[0]
            .text
            .contains("OotSourceTime : ULINT := ULINT#0;"));
        assert!(instrumented[0].text.contains("Op := UINT#6"));
        assert!(instrumented[0]
            .text
            .contains("UseSourceTimeInput := OotUseSourceTimeInput"));
        assert!(instrumented[0]
            .text
            .contains("SourceTimeInput := OotSourceTime"));
        assert!(instrumented[0].text.contains("ValueReal := Level"));
    }

    #[test]
    fn generated_definition_contains_hash() {
        let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Level : REAL {attribute 'oot' := 'value', 'unit' := 'L'};\nEND_VAR\nEND_PROGRAM\n",
        );
        let definition = definition_json_from_sources(&[source]).expect("definition");
        let hash = definition["header"]["contentHash"].as_str().unwrap();
        assert_eq!(hash.len(), 64);
        assert_eq!(definition["values"][0]["name"], "Level");
    }

    #[test]
    fn enum_state_lowering_uses_hir_enum_values() {
        let source = SourceFile::with_path(
            "main.st",
            "TYPE Phase : (Idle := 0, Fill := 10, Done := 20) END_TYPE\nPROGRAM Main\nVAR\n    ActiveStep : Phase {attribute 'oot' := 'state', 'category' := 'procedural'};\nEND_VAR\nActiveStep := Fill;\nEND_PROGRAM\n",
        );

        let instrumented = instrument_source_files(&[source]);
        let text = &instrumented[0].text;
        assert!(
            text.contains("OotPrev_ActiveStep : Phase := Phase#Idle;"),
            "{text}"
        );
        assert!(text.contains("OotStatePrev_ActiveStep : UINT := UINT#0;"));
        assert!(text.contains("IF ActiveStep = Phase#Fill THEN"));
        assert!(text.contains("OotStateNew_ActiveStep := UINT#10;"));
        assert!(text.contains("PreviousState := OotStatePrev_ActiveStep"));
        assert!(text.contains("NewState := OotStateNew_ActiveStep"));
    }

    #[test]
    fn enum_state_definition_includes_enum_set() {
        let source = SourceFile::with_path(
            "main.st",
            "TYPE Phase : (Idle := 0, Fill := 10, Done := 20) END_TYPE\nPROGRAM Main\nVAR\n    ActiveStep : Phase {attribute 'oot' := 'state', 'model' := 'ISA-88'};\nEND_VAR\nEND_PROGRAM\n",
        );

        let definition = definition_json_from_sources(&[source]).expect("definition");
        assert_eq!(definition["stateMachines"][0]["enumSet"], "Phase");
        assert_eq!(definition["enumSets"][0]["name"], "Phase");
        assert_eq!(definition["enumSets"][0]["members"][1]["label"], "Fill");
        assert_eq!(definition["enumSets"][0]["members"][1]["value"], 10);
    }
}
