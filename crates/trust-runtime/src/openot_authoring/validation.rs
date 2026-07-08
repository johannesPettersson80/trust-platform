use std::collections::BTreeMap;
use std::path::Path;

use crate::harness::SourceFile;
use trust_hir::db::SemanticDatabase;
use trust_hir::openot_authoring as hir_openot;
use trust_hir::{Project, SourceKey};
use trust_syntax::parser;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::model::{collect_program_annotations, text_size_to_usize};
use super::types::*;
use super::DEFAULT_SOURCE_ID;

pub(super) fn validate_authoring_sources(sources: &[SourceFile]) -> Vec<String> {
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
    let model = collect_authoring_model(sources);
    errors.extend(validate_source_ownership(&model.annotations));
    errors
}

pub(super) fn collect_authoring_model(sources: &[SourceFile]) -> AuthoringModel {
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

    let annotated_program_count = count_openot_programs(sources);
    let use_multi_program_defaults = annotated_program_count > 1;
    let mut next_default_source_id = DEFAULT_SOURCE_ID;
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
            if !program_has_openot_attribute(&program) {
                continue;
            }
            let default_source_id = if use_multi_program_defaults {
                let id = next_default_source_id;
                next_default_source_id = next_default_source_id.saturating_add(1);
                id
            } else {
                DEFAULT_SOURCE_ID
            };
            let annotations = collect_program_annotations(
                &program,
                source.path.as_deref(),
                file_ids[idx],
                analysis.declaration_catalog.as_ref(),
                analysis.symbols.as_ref(),
                &mut counters,
                default_source_id,
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

fn count_openot_programs(sources: &[SourceFile]) -> usize {
    sources
        .iter()
        .map(|source| {
            let parse = parser::parse(&source.text);
            if !parse.ok() {
                return 0;
            }
            parse
                .syntax()
                .descendants()
                .filter(|node| node.kind() == SyntaxKind::Program)
                .filter(program_has_openot_attribute)
                .count()
        })
        .sum()
}

fn program_has_openot_attribute(program: &SyntaxNode) -> bool {
    program
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .any(|var_decl| {
            hir_openot::parse_attribute_map_from_node(&var_decl)
                .to_btree_map()
                .contains_key("oot")
        })
}

fn validate_source_ownership(annotations: &[Annotation]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut by_id = BTreeMap::<u32, SourceDescriptor>::new();
    let mut reported = std::collections::BTreeSet::<u32>::new();
    for annotation in annotations {
        if let Some(existing) = by_id.get(&annotation.source_id) {
            if existing != &annotation.source && reported.insert(annotation.source_id) {
                errors.push(format!(
                    "OpenOT sourceid {} is used by both '{}' and '{}'; use distinct sourceid values for distinct PROGRAM sources",
                    annotation.source_id, existing.name, annotation.source.name
                ));
            }
        } else {
            by_id.insert(annotation.source_id, annotation.source.clone());
        }
    }
    errors
}
