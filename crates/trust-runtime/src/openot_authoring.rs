//! OpenOT attribute authoring support.
//!
//! This module implements the first compiler-side lowering target for
//! declaration-adjacent `{attribute 'oot' := ...}` pragmas. The user source
//! remains pure ST; the compile session instruments hidden `OPENOT_Producer`
//! calls before bytecode is built.

use crate::harness::SourceFile;
use open_ot_definition::model::canonical_event_types;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
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
const DEFAULT_STATE_CATEGORY: u16 = hir_openot::STATE_CATEGORY_PROCESS;
const ST_PRODUCER_MAX_RECORD_SIZE: u16 = 256;
const SAMPLING_MODE_DEFAULT: u16 = 0;
const SAMPLING_MODE_PERIODIC: u16 = 1;
const SAMPLING_MODE_HYSTERESIS: u16 = 2;
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
    if !validate_authoring_sources(sources).is_empty() {
        return sources.to_vec();
    }

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

    let mut sources_by_id = BTreeMap::<u32, SourceDescriptor>::new();
    let mut values = Vec::new();
    let mut states = Vec::new();
    let mut conditions = Vec::new();
    let mut messages = Vec::new();
    let mut units_by_symbol = BTreeMap::<String, u16>::new();
    let mut enum_sets = BTreeMap::<String, Value>::new();

    for annotation in &annotations {
        sources_by_id
            .entry(annotation.source_id)
            .or_insert_with(|| annotation.source.clone());
        match annotation.kind {
            OotKind::Value => {
                let unit = annotation.unit.as_ref().map(|symbol| {
                    let unit_id = canonical_unit_id(symbol)
                        .unwrap_or_else(|| panic!("unit '{symbol}' should have been validated"));
                    units_by_symbol.entry(symbol.clone()).or_insert(unit_id);
                    unit_id
                });
                values.push(json!({
                    "valueId": annotation.id,
                    "name": annotation.var_name,
                    "dataType": annotation.tlv_type(),
                    "semanticRole": annotation.semantic_role,
                    "unit": unit,
                    "deadband": annotation.deadband.as_ref().map(|decimal| json!({
                        "decimal": decimal,
                        "scaled": null
                    })),
                    "samplingPolicy": annotation.sampling_policy_json()
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
                    "causeOperands": annotation
                        .cause_operand
                        .as_ref()
                        .map(|operand| vec![json!({
                            "operandId": operand.operand_id,
                            "name": operand.name
                        })])
                        .unwrap_or_default()
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
                    "argTypes": annotation
                        .message_args
                        .iter()
                        .map(MessageArgInfo::tlv_type)
                        .collect::<Vec<_>>()
                }));
            }
            OotKind::Condition => {}
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

    let sources_json = sources_by_id
        .iter()
        .map(|(source_id, source)| {
            json!({
                "sourceId": source_id,
                "name": source.name,
                "path": source.path,
                "hierarchy": source.hierarchy,
                "dynamic": false
            })
        })
        .collect::<Vec<_>>();

    let event_types = canonical_event_types()
        .into_iter()
        .map(|event_type| {
            serde_json::to_value(event_type).expect("canonical event type serializes")
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
                "maxRecordSize": ST_PRODUCER_MAX_RECORD_SIZE,
                "maxSlots": 16,
                "overflowPolicy": "overwrite-oldest"
            },
            "epochStrategy": "retain",
            "contentHash": ""
        },
        "eventTypes": event_types,
        "sources": sources_json,
        "stateMachines": states,
        "conditions": conditions,
        "messageTemplates": messages,
        "values": values,
        "units": units,
        "enumSets": enum_sets.into_values().collect::<Vec<_>>(),
        "recipeDefinitions": [],
        "batchDefinitions": [],
        "materialDefinitions": [],
        "operatorDefinitions": [],
        "eSignatureMeanings": [
            { "meaning": 0, "label": "Authored" },
            { "meaning": 1, "label": "Reviewed" },
            { "meaning": 2, "label": "Approved" },
            { "meaning": 3, "label": "Verified" },
            { "meaning": 4, "label": "Performed" },
            { "meaning": 5, "label": "Witnessed" }
        ],
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

fn canonical_unit_id(symbol: &str) -> Option<u16> {
    match symbol.to_ascii_lowercase().as_str() {
        "1" => Some(1),
        "l" => Some(2),
        "degc" => Some(3),
        "bar" => Some(4),
        "rpm" => Some(5),
        "s" => Some(6),
        "ms" => Some(7),
        "kg" => Some(8),
        "m" => Some(9),
        "%" => Some(10),
        _ => None,
    }
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
    let mut project = Project::new();
    let file_ids = sources
        .iter()
        .enumerate()
        .map(|(idx, source)| {
            let key = match source.path.as_deref() {
                Some(path) => SourceKey::from_path(Path::new(path)),
                None => SourceKey::from_virtual(format!("openot_authoring_validation_{idx}")),
            };
            project.set_source_text(key, source.text.clone())
        })
        .collect::<Vec<_>>();
    for (idx, file_id) in file_ids.iter().copied().enumerate() {
        let source = &sources[idx];
        let parse = parser::parse(&source.text);
        let analysis = project.database().analyze(file_id);
        for diagnostic in hir_openot::collect_openot_semantic_diagnostics(
            &parse.syntax(),
            analysis.symbols.as_ref(),
            analysis.declaration_catalog.as_ref(),
            file_id,
        ) {
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
                source.path.as_deref(),
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
    source_path: Option<&str>,
    file_id: FileId,
    catalog: &DeclarationCatalog,
    symbols: &trust_hir::symbols::SymbolTable,
    counters: &mut AnnotationCounters,
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
                    },
                    attrs.clone(),
                ));
            }
        }
    }

    let mut annotations = Vec::new();
    let mut index = BTreeMap::<String, AnnotationIndexEntry>::new();
    for (draft, attrs) in &pending {
        if draft.kind == OotKind::Condition {
            continue;
        }
        let annotation = annotation_from_parts(draft.clone(), attrs, counters);
        index.insert(
            annotation.var_name.to_ascii_lowercase(),
            AnnotationIndexEntry {
                kind: annotation.kind,
                id: annotation.id,
                source_id: annotation.source_id,
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
            }
        }
        OotKind::Condition => {
            unreachable!("condition annotations require a resolved parent alarm index")
        }
    }
}

fn condition_annotation_from_parts(
    draft: AnnotationDraft,
    parent: &AnnotationIndexEntry,
) -> Annotation {
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
            OotKind::Alarm | OotKind::Message | OotKind::Condition => {
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
        if annotation.kind == OotKind::Condition {
            continue;
        }
        match annotation.kind {
            OotKind::Value => statements.extend(value_statements(annotation)),
            OotKind::State => statements.extend(state_statements(annotation)),
            OotKind::Alarm => statements.extend(alarm_statements(annotation)),
            OotKind::Message => statements.extend(message_statements(annotation)),
            OotKind::Condition => {}
        }
    }
    for annotation in annotations
        .iter()
        .filter(|annotation| annotation.kind == OotKind::Condition)
    {
        statements.extend(condition_lifecycle_statements(annotation));
    }
    statements
}

fn value_statements(annotation: &Annotation) -> Vec<String> {
    if annotation.is_real() {
        return vec![
            format!(
                "{PRODUCER_NAME}(Execute := TRUE, Op := UINT#6, SourceId := UDINT#{}, {}, ValueId := UDINT#{}, ValueReal := {}, DeadbandReal := {}, {}, SuppressPrevious := {}, HasQuality := {}, Quality := UINT#{});",
                annotation.source_id,
                source_time_args(),
                annotation.id,
                annotation.var_name,
                real_literal(annotation.deadband.as_deref().unwrap_or("0.0")),
                annotation.sampling_args(),
                bool_literal(annotation.suppress_previous),
                bool_literal(annotation.quality.is_some()),
                annotation.quality.unwrap_or(0)
            ),
            format!("{PRODUCER_NAME}(Execute := FALSE);"),
        ];
    }

    if annotation.is_dint() {
        return vec![
            format!(
                "{PRODUCER_NAME}(Execute := TRUE, Op := UINT#7, SourceId := UDINT#{}, {}, ValueId := UDINT#{}, ValueInt := {}, DeadbandReal := REAL#0.0, {}, SuppressPrevious := {}, HasQuality := {}, Quality := UINT#{});",
                annotation.source_id,
                source_time_args(),
                annotation.id,
                annotation.var_name,
                annotation.sampling_args(),
                bool_literal(annotation.suppress_previous),
                bool_literal(annotation.quality.is_some()),
                annotation.quality.unwrap_or(0)
            ),
            format!("{PRODUCER_NAME}(Execute := FALSE);"),
        ];
    }

    if annotation.is_string() {
        return vec![
            format!(
                "{PRODUCER_NAME}(Execute := TRUE, Op := UINT#11, SourceId := UDINT#{}, {}, ValueId := UDINT#{}, ValueString := {}, {}, SuppressPrevious := {}, HasQuality := {}, Quality := UINT#{});",
                annotation.source_id,
                source_time_args(),
                annotation.id,
                annotation.var_name,
                annotation.sampling_args(),
                bool_literal(annotation.suppress_previous),
                bool_literal(annotation.quality.is_some()),
                annotation.quality.unwrap_or(0)
            ),
            format!("{PRODUCER_NAME}(Execute := FALSE);"),
        ];
    }

    vec![
        format!(
            "{PRODUCER_NAME}(Execute := TRUE, Op := UINT#10, SourceId := UDINT#{}, {}, ValueId := UDINT#{}, ValueTypeTag := BYTE#16#{:02X}, ValuePayloadLength := UINT#{}, ValueBits := {}, {}, SuppressPrevious := {}, HasQuality := {}, Quality := UINT#{});",
            annotation.source_id,
            source_time_args(),
            annotation.id,
            annotation.tlv_type(),
            annotation.payload_len(),
            annotation.value_bits_expr(),
            annotation.sampling_args(),
            bool_literal(annotation.suppress_previous),
            bool_literal(annotation.quality.is_some()),
            annotation.quality.unwrap_or(0)
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
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#9".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        format!("ConditionId := UDINT#{}", annotation.id),
        format!("ConditionClass := UINT#{}", annotation.condition_class),
        format!("Severity := UINT#{}", annotation.severity),
        format!("ConditionActive := {}", annotation.var_name),
    ];
    if let Some(cause_operand) = &annotation.cause_operand {
        call_args.push("HasCauseOperand := TRUE".to_string());
        call_args.push(format!(
            "CauseOperand := UDINT#{}",
            cause_operand.operand_id
        ));
    }
    vec![
        format!("IF {} <> OotPrev_{safe} THEN", annotation.var_name),
        format!("    {PRODUCER_NAME}({});", call_args.join(", ")),
        format!("    {PRODUCER_NAME}(Execute := FALSE);"),
        format!("    OotPrev_{safe} := {};", annotation.var_name),
        "END_IF;".to_string(),
    ]
}

fn message_statements(annotation: &Annotation) -> Vec<String> {
    let safe = safe_identifier(&annotation.var_name);
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#0".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        "Checkpoint := FALSE".to_string(),
        "AccumulateScanRecords := TRUE".to_string(),
        format!("MessageTemplateId := UDINT#{}", annotation.id),
    ];
    if let Some(severity) = annotation.message_severity {
        call_args.push("MessageHasSeverity := TRUE".to_string());
        call_args.push(format!("MessageSeverity := UINT#{severity}"));
    }
    call_args.push(format!(
        "MessageArgCount := UINT#{}",
        annotation.message_args.len()
    ));
    for (idx, arg) in annotation.message_args.iter().enumerate() {
        let arg_number = idx + 1;
        call_args.push(format!(
            "MessageArg{arg_number}TypeTag := BYTE#16#{:02X}",
            arg.tlv_type()
        ));
        if arg.is_string() {
            call_args.push(format!("MessageArg{arg_number}String := {}", arg.name));
        } else {
            call_args.push(format!(
                "MessageArg{arg_number}PayloadLength := UINT#{}",
                arg.payload_len()
            ));
            call_args.push(format!(
                "MessageArg{arg_number}Bits := {}",
                arg.value_bits_expr()
            ));
        }
    }
    vec![
        format!("IF {} AND (NOT OotPrev_{safe}) THEN", annotation.var_name),
        format!("    {PRODUCER_NAME}({});", call_args.join(", ")),
        format!("    {PRODUCER_NAME}(Execute := FALSE);"),
        "END_IF;".to_string(),
        format!("OotPrev_{safe} := {};", annotation.var_name),
    ]
}

fn condition_lifecycle_statements(annotation: &Annotation) -> Vec<String> {
    let safe = safe_identifier(&annotation.var_name);
    let mut call_args = vec![
        "Execute := TRUE".to_string(),
        "Op := UINT#12".to_string(),
        format!("SourceId := UDINT#{}", annotation.source_id),
        source_time_args(),
        format!("ConditionId := UDINT#{}", annotation.id),
        format!(
            "ConditionLifecycleEventTypeId := UDINT#16#{:04X}",
            annotation
                .condition_event_id
                .expect("condition event id should be resolved")
        ),
    ];
    call_args.push(format!(
        "LifecycleHasAckBy := {}",
        bool_literal(annotation.condition_ack_by.is_some())
    ));
    call_args.push(format!(
        "LifecycleAckBy := {}",
        annotation.condition_ack_by.as_deref().unwrap_or("''")
    ));
    call_args.push(format!(
        "LifecycleHasShelveSecs := {}",
        bool_literal(annotation.condition_shelve_secs.is_some())
    ));
    call_args.push(format!(
        "LifecycleShelveSecs := {}",
        annotation
            .condition_shelve_secs
            .as_deref()
            .unwrap_or("UDINT#0")
    ));
    call_args.push(format!(
        "LifecycleHasReason := {}",
        bool_literal(annotation.condition_reason.is_some())
    ));
    call_args.push(format!(
        "LifecycleReason := {}",
        annotation.condition_reason.as_deref().unwrap_or("''")
    ));
    call_args.push(format!(
        "LifecycleHasComment := {}",
        bool_literal(annotation.condition_comment.is_some())
    ));
    call_args.push(format!(
        "LifecycleComment := {}",
        annotation.condition_comment.as_deref().unwrap_or("''")
    ));
    call_args.push(format!(
        "LifecycleHasPreviousPriority := {}",
        bool_literal(annotation.condition_previous_priority.is_some())
    ));
    call_args.push(format!(
        "LifecyclePreviousPriority := {}",
        annotation
            .condition_previous_priority
            .as_deref()
            .unwrap_or("UINT#0")
    ));
    call_args.push(format!(
        "LifecycleHasNewPriority := {}",
        bool_literal(annotation.condition_new_priority.is_some())
    ));
    call_args.push(format!(
        "LifecycleNewPriority := {}",
        annotation
            .condition_new_priority
            .as_deref()
            .unwrap_or("UINT#0")
    ));
    vec![
        format!("IF {} AND (NOT OotPrev_{safe}) THEN", annotation.var_name),
        format!("    {PRODUCER_NAME}({});", call_args.join(", ")),
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

fn bool_literal(value: bool) -> &'static str {
    if value {
        "TRUE"
    } else {
        "FALSE"
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

#[derive(Debug, Clone)]
struct Annotation {
    kind: OotKind,
    var_name: String,
    st_type: String,
    initializer: Option<String>,
    id: u32,
    source_id: u32,
    source: SourceDescriptor,
    deadband: Option<String>,
    sampling: Option<String>,
    interval_ms: Option<u64>,
    unit: Option<String>,
    quality: Option<u16>,
    semantic_role: u16,
    suppress_previous: bool,
    category: u16,
    model: Option<String>,
    condition_class: u16,
    severity: u16,
    message: Option<String>,
    message_severity: Option<u16>,
    message_args: Vec<MessageArgInfo>,
    cause_operand: Option<CauseOperandInfo>,
    enum_state: Option<EnumStateInfo>,
    condition_event_id: Option<u32>,
    condition_ack_by: Option<String>,
    condition_shelve_secs: Option<String>,
    condition_reason: Option<String>,
    condition_comment: Option<String>,
    condition_previous_priority: Option<String>,
    condition_new_priority: Option<String>,
}

impl Annotation {
    fn is_real(&self) -> bool {
        self.st_type.eq_ignore_ascii_case("REAL")
    }

    fn is_dint(&self) -> bool {
        self.st_type.eq_ignore_ascii_case("DINT")
    }

    fn is_string(&self) -> bool {
        let ty = self.normalized_type();
        ty == "STRING" || ty.starts_with("STRING[")
    }

    fn normalized_type(&self) -> String {
        self.st_type.trim().to_ascii_uppercase()
    }

    fn tlv_type(&self) -> u8 {
        tlv_type_for_type(&self.st_type)
    }

    fn payload_len(&self) -> u8 {
        payload_len_for_type(&self.st_type)
    }

    fn value_bits_expr(&self) -> String {
        value_bits_expr_for_type(&self.var_name, &self.st_type)
    }

    fn sampling_policy_json(&self) -> Value {
        match self.sampling.as_deref() {
            Some("on-change") => json!("on-change"),
            Some("deadband") => json!("deadband"),
            Some("periodic") => json!(format!(
                "periodic:{}",
                self.interval_ms
                    .expect("periodic sampling interval should be validated")
            )),
            Some("hysteresis") => json!("hysteresis"),
            _ if self.deadband.is_some() => Value::Null,
            _ => json!("on-change"),
        }
    }

    fn sampling_mode(&self) -> u16 {
        match self.sampling.as_deref() {
            Some("periodic") => SAMPLING_MODE_PERIODIC,
            Some("hysteresis") => SAMPLING_MODE_HYSTERESIS,
            _ => SAMPLING_MODE_DEFAULT,
        }
    }

    fn sampling_interval_ms(&self) -> u64 {
        self.interval_ms.unwrap_or(0)
    }

    fn sampling_args(&self) -> String {
        format!(
            "SamplingMode := UINT#{}, SamplingIntervalMs := ULINT#{}",
            self.sampling_mode(),
            self.sampling_interval_ms()
        )
    }
}

fn normalized_type_name(st_type: &str) -> String {
    st_type.trim().to_ascii_uppercase()
}

fn is_string_type(st_type: &str) -> bool {
    let ty = normalized_type_name(st_type);
    ty == "STRING" || ty.starts_with("STRING[")
}

fn tlv_type_for_type(st_type: &str) -> u8 {
    if is_string_type(st_type) {
        return 0x0C;
    }
    match normalized_type_name(st_type).as_str() {
        "BOOL" => 0x00,
        "SINT" => 0x01,
        "USINT" => 0x02,
        "UINT" => 0x03,
        "INT" => 0x04,
        "UDINT" => 0x05,
        "DINT" => 0x06,
        "ULINT" => 0x07,
        "LINT" => 0x08,
        "REAL" => 0x09,
        "LREAL" => 0x0A,
        _ => 0x06,
    }
}

fn payload_len_for_type(st_type: &str) -> u8 {
    if is_string_type(st_type) {
        return 96;
    }
    match normalized_type_name(st_type).as_str() {
        "BOOL" | "SINT" | "USINT" => 1,
        "INT" | "UINT" => 2,
        "DINT" | "UDINT" | "REAL" => 4,
        "LINT" | "ULINT" | "LREAL" => 8,
        _ => 4,
    }
}

fn value_bits_expr_for_type(var_name: &str, st_type: &str) -> String {
    match normalized_type_name(st_type).as_str() {
        "BOOL" => format!("BOOL_TO_ULINT({var_name})"),
        "SINT" => format!("BYTE_TO_ULINT(SINT_TO_BYTE({var_name}))"),
        "USINT" => format!("USINT_TO_ULINT({var_name})"),
        "INT" => format!("WORD_TO_ULINT(INT_TO_WORD({var_name}))"),
        "UINT" => format!("UINT_TO_ULINT({var_name})"),
        "UDINT" => format!("UDINT_TO_ULINT({var_name})"),
        "DINT" => format!("DWORD_TO_ULINT(DINT_TO_DWORD({var_name}))"),
        "REAL" => format!("DWORD_TO_ULINT(REAL_TO_DWORD({var_name}))"),
        "LINT" => format!("LWORD_TO_ULINT(LINT_TO_LWORD({var_name}))"),
        "ULINT" => var_name.to_string(),
        "LREAL" => format!("LWORD_TO_ULINT(LREAL_TO_LWORD({var_name}))"),
        other => panic!("unsupported OpenOT value type {other}"),
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
    source: SourceDescriptor,
    enum_state: Option<EnumStateInfo>,
    message_args: Vec<MessageArgInfo>,
    cause_operand: Option<CauseOperandInfo>,
    condition_parent: Option<String>,
    condition_event_id: Option<u32>,
    condition_ack_by: Option<String>,
    condition_shelve_secs: Option<String>,
    condition_reason: Option<String>,
    condition_comment: Option<String>,
    condition_previous_priority: Option<String>,
    condition_new_priority: Option<String>,
}

#[derive(Debug, Clone)]
struct AnnotationIndexEntry {
    kind: OotKind,
    id: u32,
    source_id: u32,
}

#[derive(Debug, Clone)]
struct SourceDescriptor {
    name: String,
    path: Vec<String>,
    hierarchy: Vec<String>,
}

#[derive(Debug, Clone)]
struct MessageArgInfo {
    name: String,
    st_type: String,
}

impl MessageArgInfo {
    fn normalized_type(&self) -> String {
        self.st_type.trim().to_ascii_uppercase()
    }

    fn is_string(&self) -> bool {
        let ty = self.normalized_type();
        ty == "STRING" || ty.starts_with("STRING[")
    }

    fn tlv_type(&self) -> u8 {
        tlv_type_for_type(&self.st_type)
    }

    fn payload_len(&self) -> u8 {
        payload_len_for_type(&self.st_type)
    }

    fn value_bits_expr(&self) -> String {
        value_bits_expr_for_type(&self.name, &self.st_type)
    }
}

#[derive(Debug, Clone)]
struct CauseOperandInfo {
    operand_id: u32,
    name: String,
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
    use std::collections::BTreeSet;

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
        assert_eq!(
            definition["header"]["constraints"]["maxRecordSize"],
            ST_PRODUCER_MAX_RECORD_SIZE
        );
        assert_eq!(definition["values"][0]["name"], "Level");
        assert_eq!(definition["sources"][0]["name"], "main.Main");
        assert_eq!(
            definition["sources"][0]["path"],
            serde_json::json!(["main", "Main"])
        );
        assert_eq!(
            definition["sources"][0]["hierarchy"],
            serde_json::json!(["file", "program"])
        );
    }

    #[test]
    fn generated_definition_uses_canonical_unit_ids() {
        let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Level : REAL {attribute 'oot' := 'value', 'unit' := 'L'};\n    Temp : REAL {attribute 'oot' := 'value', 'unit' := 'degC'};\nEND_VAR\nEND_PROGRAM\n",
        );
        let definition = definition_json_from_sources(&[source]).expect("definition");
        assert_eq!(definition["values"][0]["unit"], 2);
        assert_eq!(definition["values"][1]["unit"], 3);
        let units = definition["units"].as_array().expect("units");
        assert!(units
            .iter()
            .any(|unit| unit["unitId"] == 2 && unit["symbol"] == "L"));
        assert!(units
            .iter()
            .any(|unit| unit["unitId"] == 3 && unit["symbol"] == "degC"));
    }

    #[test]
    fn value_quality_semantic_role_and_previous_lowering_are_explicit() {
        let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Setpoint : REAL {attribute 'oot' := 'value', 'quality' := 'uncertain', 'semanticRole' := 'setpoint', 'previous' := 'false'};\nEND_VAR\nSetpoint := REAL#10.0;\nEND_PROGRAM\n",
        );
        let definition =
            definition_json_from_sources(std::slice::from_ref(&source)).expect("definition");
        assert_eq!(definition["values"][0]["semanticRole"], 1);

        let instrumented = instrument_source_files(&[source]);
        let text = &instrumented[0].text;
        assert!(text.contains("SuppressPrevious := TRUE"), "{text}");
        assert!(text.contains("HasQuality := TRUE"), "{text}");
        assert!(text.contains("Quality := UINT#1"), "{text}");
    }

    #[test]
    fn value_sampling_policy_lowering_uses_existing_definition_field() {
        let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Pressure : REAL {attribute 'oot' := 'value', 'sampling' := 'periodic', 'interval' := '250'};\n    Flow : REAL {attribute 'oot' := 'value', 'sampling' := 'hysteresis', 'deadband' := '1.5'};\n    Level : REAL {attribute 'oot' := 'value', 'deadband' := '0.5'};\nEND_VAR\nEND_PROGRAM\n",
        );
        let definition =
            definition_json_from_sources(std::slice::from_ref(&source)).expect("definition");
        assert_eq!(definition["values"][0]["samplingPolicy"], "periodic:250");
        assert_eq!(definition["values"][1]["samplingPolicy"], "hysteresis");
        assert_eq!(definition["values"][2]["samplingPolicy"], Value::Null);

        let instrumented = instrument_source_files(&[source]);
        let text = &instrumented[0].text;
        assert!(text.contains("SamplingMode := UINT#1"), "{text}");
        assert!(text.contains("SamplingIntervalMs := ULINT#250"), "{text}");
        assert!(text.contains("SamplingMode := UINT#2"), "{text}");
    }

    #[test]
    fn condition_lifecycle_lowering_inherits_parent_and_emits_after_alarm_phase() {
        let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    OperatorName : STRING[32] := 'operator-a';\n    ReasonText : STRING[32] := 'maintenance';\n    CommentText : STRING[32] := 'operator comment';\n    ShelveSecs : UDINT := UDINT#300;\n    PreviousPriority : UINT := UINT#600;\n    NewPriority : UINT := UINT#900;\n    AckHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'acknowledge', 'by' := OperatorName};\n    HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'sourceid' := '77', 'conditionid' := '9101'};\n    ConfirmHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'confirm', 'by' := OperatorName};\n    ShelveHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'shelve', 'by' := OperatorName, 'seconds' := ShelveSecs};\n    UnshelveHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'unshelve'};\n    SuppressHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'suppress', 'reason' := ReasonText};\n    UnsuppressHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'unsuppress'};\n    OosHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'out-of-service', 'by' := OperatorName};\n    InServiceHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'in-service'};\n    ResetHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'reset', 'by' := OperatorName};\n    CommentHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'comment', 'comment' := CommentText, 'by' := OperatorName};\n    PriorityHighPh : BOOL {attribute 'oot' := 'condition', 'of' := HighPhAlarm, 'event' := 'priority-changed', 'previous-priority' := PreviousPriority, 'new-priority' := NewPriority, 'by' := OperatorName};\nEND_VAR\nHighPhAlarm := TRUE;\nAckHighPh := TRUE;\nConfirmHighPh := TRUE;\nShelveHighPh := TRUE;\nUnshelveHighPh := TRUE;\nSuppressHighPh := TRUE;\nUnsuppressHighPh := TRUE;\nOosHighPh := TRUE;\nInServiceHighPh := TRUE;\nResetHighPh := TRUE;\nCommentHighPh := TRUE;\nPriorityHighPh := TRUE;\nEND_PROGRAM\n",
        );
        let definition =
            definition_json_from_sources(std::slice::from_ref(&source)).expect("definition");
        assert_eq!(definition["conditions"].as_array().unwrap().len(), 1);
        assert_eq!(definition["conditions"][0]["conditionId"], 9101);

        let instrumented = instrument_source_files(&[source]);
        let text = &instrumented[0].text;
        let alarm_pos = text.find("Op := UINT#9").expect("alarm op");
        let lifecycle_pos = text.find("Op := UINT#12").expect("lifecycle op");
        assert!(
            alarm_pos < lifecycle_pos,
            "alarm updates must emit before lifecycle commands:\n{text}"
        );
        assert!(text.contains("SourceId := UDINT#77"), "{text}");
        assert!(text.contains("ConditionId := UDINT#9101"), "{text}");
        assert!(
            text.contains("ConditionLifecycleEventTypeId := UDINT#16#0202"),
            "{text}"
        );
        assert!(
            text.contains("ConditionLifecycleEventTypeId := UDINT#16#0203"),
            "{text}"
        );
        assert!(
            text.contains("ConditionLifecycleEventTypeId := UDINT#16#0204"),
            "{text}"
        );
        assert!(
            text.contains("ConditionLifecycleEventTypeId := UDINT#16#0205"),
            "{text}"
        );
        assert!(
            text.contains("ConditionLifecycleEventTypeId := UDINT#16#0206"),
            "{text}"
        );
        assert!(
            text.contains("ConditionLifecycleEventTypeId := UDINT#16#0207"),
            "{text}"
        );
        assert!(
            text.contains("ConditionLifecycleEventTypeId := UDINT#16#0208"),
            "{text}"
        );
        assert!(
            text.contains("ConditionLifecycleEventTypeId := UDINT#16#0209"),
            "{text}"
        );
        assert!(
            text.contains("ConditionLifecycleEventTypeId := UDINT#16#020A"),
            "{text}"
        );
        assert!(
            text.contains("ConditionLifecycleEventTypeId := UDINT#16#020B"),
            "{text}"
        );
        assert!(
            text.contains("ConditionLifecycleEventTypeId := UDINT#16#020C"),
            "{text}"
        );
        assert!(text.contains("LifecycleAckBy := OperatorName"), "{text}");
        assert!(text.contains("LifecycleShelveSecs := ShelveSecs"), "{text}");
        assert!(text.contains("LifecycleReason := ReasonText"), "{text}");
        assert!(text.contains("LifecycleComment := CommentText"), "{text}");
        assert!(
            text.contains("LifecyclePreviousPriority := PreviousPriority"),
            "{text}"
        );
        assert!(
            text.contains("LifecycleNewPriority := NewPriority"),
            "{text}"
        );
    }

    #[test]
    fn instruments_fixed_width_value_types_with_generic_value_op() {
        let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Enabled : BOOL {attribute 'oot' := 'value'};\n    Total : ULINT {attribute 'oot' := 'value'};\n    Ratio : LREAL {attribute 'oot' := 'value'};\n    Label : STRING[16] {attribute 'oot' := 'value'};\nEND_VAR\nEnabled := TRUE;\nTotal := ULINT#10;\nRatio := LREAL#1.25;\nLabel := 'ready';\nEND_PROGRAM\n",
        );

        let instrumented = instrument_source_files(&[source]);
        let text = &instrumented[0].text;
        assert!(text.contains("Op := UINT#10"), "{text}");
        assert!(text.contains("ValueTypeTag := BYTE#16#00"), "{text}");
        assert!(text.contains("ValueTypeTag := BYTE#16#07"), "{text}");
        assert!(text.contains("ValueTypeTag := BYTE#16#0A"), "{text}");
        assert!(text.contains("ValuePayloadLength := UINT#8"), "{text}");
        assert!(text.contains("LREAL_TO_LWORD(Ratio)"), "{text}");
        assert!(text.contains("Op := UINT#11"), "{text}");
        assert!(text.contains("ValueString := Label"), "{text}");
    }

    #[test]
    fn generated_definition_rejects_unsupported_value_types() {
        let source = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    WideText : WSTRING {attribute 'oot' := 'value'};\nEND_VAR\nEND_PROGRAM\n",
        );
        let err = definition_json_from_sources(&[source]).expect_err("unsupported values reject");
        assert!(err.contains("OpenOT value logging supports BOOL"));
    }

    #[test]
    fn enum_state_lowering_uses_hir_enum_values() {
        let source = SourceFile::with_path(
            "main.st",
            "TYPE Phase : (Idle := 0, Fill := 10, Done := 20) END_TYPE\nPROGRAM Main\nVAR\n    ActiveStep : Phase {attribute 'oot' := 'state', 'category' := 'process'};\nEND_VAR\nActiveStep := Fill;\nEND_PROGRAM\n",
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
            "TYPE Phase : (Idle := 0, Running := 1, Complete := 2) END_TYPE\nPROGRAM Main\nVAR\n    ActiveStep : Phase {attribute 'oot' := 'state', 'category' := 'procedural', 'model' := 'ISA-88'};\nEND_VAR\nEND_PROGRAM\n",
        );

        let definition = definition_json_from_sources(&[source]).expect("definition");
        assert_eq!(definition["stateMachines"][0]["enumSet"], "Phase");
        assert_eq!(definition["enumSets"][0]["name"], "Phase");
        assert_eq!(definition["enumSets"][0]["members"][1]["label"], "Running");
        assert_eq!(definition["enumSets"][0]["members"][1]["value"], 1);
    }

    #[test]
    fn state_category_defaults_to_process() {
        let source = SourceFile::with_path(
            "main.st",
            "TYPE Phase : (Idle := 0, Fill := 10, Done := 20) END_TYPE\nPROGRAM Main\nVAR\n    ActiveStep : Phase {attribute 'oot' := 'state'};\nEND_VAR\nEND_PROGRAM\n",
        );

        let definition = definition_json_from_sources(&[source]).expect("definition");
        assert_eq!(
            definition["stateMachines"][0]["category"],
            hir_openot::STATE_CATEGORY_PROCESS
        );
        assert_eq!(
            definition["stateMachines"][0]["proceduralModel"],
            Value::Null
        );
    }

    #[test]
    fn generated_event_types_match_openot_reference_canonical_schema() {
        let source = SourceFile::with_path(
            "main.st",
            "TYPE Phase : (Idle := 0, Filling := 1) END_TYPE\nPROGRAM Main\nVAR\n    Step : Phase {attribute 'oot' := 'state', 'category' := 'process'};\n    Level : REAL {attribute 'oot' := 'value'};\n    Alarm : BOOL {attribute 'oot' := 'alarm'};\n    Started : BOOL {attribute 'oot' := 'message', 'template' := 'started'};\nEND_VAR\nEND_PROGRAM\n",
        );

        let definition = definition_json_from_sources(&[source]).expect("definition");
        for event in definition["eventTypes"].as_array().expect("eventTypes") {
            let parsed: open_ot_definition::model::EventTypeDefinition =
                serde_json::from_value(event.clone()).expect("generated event type shape");
            assert_eq!(
                parsed,
                open_ot_definition::model::canonical_event_type(parsed.id)
                    .expect("canonical schema for emitted event")
            );
        }
    }

    #[test]
    fn pinned_ids_are_stable_when_declarations_are_reordered() {
        let first = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Level : REAL {attribute 'oot' := 'value', 'id' := '2201'};\n    BatchCount : DINT {attribute 'oot' := 'value', 'id' := '2202'};\nEND_VAR\nEND_PROGRAM\n",
        );
        let second = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    BatchCount : DINT {attribute 'oot' := 'value', 'id' := '2202'};\n    Level : REAL {attribute 'oot' := 'value', 'id' := '2201'};\nEND_VAR\nEND_PROGRAM\n",
        );

        let first = definition_json_from_sources(&[first]).expect("first definition");
        let second = definition_json_from_sources(&[second]).expect("second definition");
        let first_ids = first["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value["valueId"].as_u64().unwrap())
            .collect::<BTreeSet<_>>();
        let second_ids = second["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value["valueId"].as_u64().unwrap())
            .collect::<BTreeSet<_>>();

        assert_eq!(first_ids, second_ids);
        assert_eq!(first_ids, BTreeSet::from([2201, 2202]));
    }

    #[test]
    fn unpinned_ids_follow_declaration_order() {
        let first = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    Level : REAL {attribute 'oot' := 'value'};\n    BatchCount : DINT {attribute 'oot' := 'value'};\nEND_VAR\nEND_PROGRAM\n",
        );
        let second = SourceFile::with_path(
            "main.st",
            "PROGRAM Main\nVAR\n    BatchCount : DINT {attribute 'oot' := 'value'};\n    Level : REAL {attribute 'oot' := 'value'};\nEND_VAR\nEND_PROGRAM\n",
        );

        let first = definition_json_from_sources(&[first]).expect("first definition");
        let second = definition_json_from_sources(&[second]).expect("second definition");

        assert_eq!(first["values"][0]["name"], "Level");
        assert_eq!(first["values"][0]["valueId"], 2001);
        assert_eq!(second["values"][0]["name"], "BatchCount");
        assert_eq!(second["values"][0]["valueId"], 2001);
    }
}
