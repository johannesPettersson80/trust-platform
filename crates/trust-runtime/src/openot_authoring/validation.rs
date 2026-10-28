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

/// Prepare validation and instrumentation from one semantic project. Sources
/// without OpenOT attributes need no OpenOT semantic analysis at all.
pub(super) fn prepare_authoring_sources(sources: &[SourceFile]) -> (AuthoringModel, Vec<String>) {
    let _trace = trust_hir::timing::CompilerStage::new("openot.prepare", sources.len());
    let parses: Vec<_> = sources
        .iter()
        .map(|source| parser::parse(&source.text))
        .collect();
    let attributed: Vec<_> = parses
        .iter()
        .map(|parse| hir_openot::parse_attribute_map_from_node(&parse.syntax()).contains_oot())
        .collect();
    trust_hir::timing::event(
        "openot.attributed_files",
        attributed.iter().filter(|present| **present).count(),
    );
    if !attributed.iter().any(|present| *present) {
        trust_hir::timing::event("openot.skipped", 1);
        return (
            AuthoringModel {
                files: vec![FileAuthoringModel::default(); sources.len()],
                annotations: Vec::new(),
            },
            Vec::new(),
        );
    }
    let mut errors = Vec::new();
    for (idx, parse) in parses.iter().enumerate() {
        for diagnostic in hir_openot::collect_openot_attribute_diagnostics(&parse.syntax()) {
            let label = sources[idx]
                .path
                .as_deref()
                .map_or_else(|| format!("source {}", idx + 1), ToString::to_string);
            errors.push(format!("{label}: {}", diagnostic.message));
        }
    }
    // Keep every source available for cross-file types and functions, but analyze
    // only files which contain attributes. Ordinary ST checking stays in the compiler.
    let mut project = Project::new();
    let file_ids: Vec<_> = sources
        .iter()
        .enumerate()
        .map(|(idx, source)| {
            let key = match source.path.as_deref() {
                Some(path) => SourceKey::from_path(Path::new(path)),
                None => SourceKey::from_virtual(format!("openot_authoring_{idx}")),
            };
            project.set_source_text(key, source.text.clone())
        })
        .collect();

    let annotated_program_count = count_openot_programs(sources);
    let use_multi_program_defaults = annotated_program_count > 1;
    let mut next_default_source_id = DEFAULT_SOURCE_ID;
    let mut counters = AnnotationCounters::default();
    let mut files = Vec::with_capacity(sources.len());
    let mut all_annotations = Vec::new();

    for (idx, source) in sources.iter().enumerate() {
        let parse = &parses[idx];
        if !attributed[idx] {
            files.push(FileAuthoringModel::default());
            continue;
        }
        let syntax = parse.syntax();
        let analysis = project.database().analyze(file_ids[idx]);
        for diagnostic in hir_openot::collect_openot_semantic_diagnostics(
            &syntax,
            analysis.symbols.as_ref(),
            analysis.declaration_catalog.as_ref(),
            file_ids[idx],
        ) {
            let label = source
                .path
                .as_deref()
                .map_or_else(|| format!("source {}", idx + 1), ToString::to_string);
            errors.push(format!("{label}: {}", diagnostic.message));
        }
        if !parse.ok() {
            files.push(FileAuthoringModel::default());
            continue;
        }
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

    errors.extend(validate_source_ownership(&all_annotations));
    (
        AuthoringModel {
            files,
            annotations: all_annotations,
        },
        errors,
    )
}

#[cfg(test)]
pub(super) fn validate_authoring_sources(sources: &[SourceFile]) -> Vec<String> {
    prepare_authoring_sources(sources).1
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
