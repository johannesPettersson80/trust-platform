//! Source/context association around supplied artifacts, never replacement bodies.
use super::{build, CompileError, SourceFile};
use crate::{
    bytecode::{
        BytecodeMetadata, BytecodeModule, BytecodeVersion, Section, SectionData, SectionId,
    },
    Runtime,
};
use smol_str::SmolStr;
use std::sync::Arc;

pub(super) fn build_runtime(
    supplied: &BytecodeModule,
    resource_name: Option<&str>,
    sources: &[SourceFile],
    labels: bool,
    extras: &[SmolStr],
    limits: trust_runtime_core::vm::PreparationLimits,
) -> Result<Runtime, CompileError> {
    let error = |error: crate::bytecode::BytecodeError| CompileError::new(error.to_string());
    supplied.validate().map_err(error)?;
    let (mut module, metadata) = if supplied.version == BytecodeVersion::LEGACY {
        if sources.is_empty() {
            return Err(CompileError::new(
                "STBC 1.1 requires matching source construction context",
            ));
        }
        let input = build::lower_application_with_abi(
            sources,
            labels,
            supplied.storage_abi().map_err(error)?,
        )?
        .prepare_legacy_import_context(extras)?;
        let (module, projection) =
            crate::bytecode::build_legacy_with_type_projection(supplied, &input).map_err(error)?;
        let mut metadata = input.metadata_snapshot(&module)?;
        metadata.artifact_types = projection;
        // Construction context establishes declarations/defaults, not the
        // provenance of supplied ordinary bodies. Certify source association
        // separately using the retained explicit legacy producer, outside RUN.
        let certified =
            match build::build_bytecode_module_from_source_files(sources, labels, extras) {
                Ok(expected) if equivalent_source_artifact(&expected, supplied) => {
                    metadata
                        .associate_artifact_source_paths(&expected, supplied)
                        .map_err(|err| CompileError::new(err.to_string()))?;
                    true
                }
                _ => false,
            };
        if !certified {
            metadata.clear_source_association();
        }
        (module, Some(Arc::new(metadata)))
    } else if matches!(
        supplied.version,
        BytecodeVersion::SOURCE_FREE | BytecodeVersion::SOURCE_FREE_V2
    ) {
        let metadata = if sources.is_empty() {
            None
        } else {
            let lowered = build::lower_application_with_abi(
                sources,
                labels,
                supplied.storage_abi().map_err(error)?,
            )?;
            let contextual_legacy =
                supplied.flags & crate::bytecode::HEADER_FLAG_LEGACY_TEMPORAL32 != 0;
            let input = if contextual_legacy {
                lowered.prepare_legacy_import_context(extras)?
            } else {
                lowered.prepare_authoring(extras)?
            };
            let repeated_legacy_type_context =
                contextual_legacy && input.has_repeated_program_types();
            if repeated_legacy_type_context {
                // This source producer cannot certify the supplied archive.
                // Derive all tooling types from admitted bytecode instead of
                // attaching an unverified context registry with matching names.
                None
            } else {
                let mut projected = crate::bytecode::build_module_with_type_projection_for_version(
                    &input,
                    supplied.version,
                )
                .map_err(error)?;
                select_resource(&mut projected.module, resource_name)?;
                // A contextual 1.1 import deliberately retains the legacy ordinary
                // bodies. Recreate that same validated route for source identity;
                // current source-free emission need not have identical instructions.
                // Failure to reproduce the old producer means no source proof, not
                // permission to substitute freshly compiled bodies in the artifact.
                if !equivalent_source_artifact(&projected.module, supplied)
                    && contextual_legacy
                    && let Ok(legacy) =
                        build::build_bytecode_module_from_source_files(sources, labels, extras)
                    && let Ok((mut expected, artifact_types)) =
                        crate::bytecode::build_legacy_with_type_projection(&legacy, &input)
                    && select_resource(&mut expected, resource_name).is_ok()
                    && equivalent_source_artifact(&expected, supplied)
                {
                    projected.module = expected;
                    projected.artifact_types = artifact_types;
                    // The contextual legacy route has no positional source-free
                    // I/O label proof; admitted I/O bindings remain authoritative.
                    projected.io_display_names.clear();
                }
                if equivalent_source_artifact(&projected.module, supplied) {
                    let mut metadata = input.metadata_snapshot(supplied)?;
                    metadata.artifact_types = projected.artifact_types;
                    metadata.io_display_names = projected.io_display_names;
                    metadata
                        .associate_artifact_source_paths(&projected.module, supplied)
                        .map_err(|err| CompileError::new(err.to_string()))?;
                    Some(Arc::new(metadata))
                } else {
                    None
                }
            }
        };
        (supplied.clone(), metadata)
    } else {
        return Err(CompileError::new("unsupported executable bytecode version"));
    };
    let original_resource_unchanged = match resource_name {
        Some(name) => supplied.metadata().map_err(error)?.resource(name).is_some(),
        None => true,
    };
    let resource = select_resource(&mut module, resource_name)?;
    let mut runtime = Runtime::from_module_with_metadata(module, metadata, resource, limits)
        .map_err(|err| CompileError::new(err.to_string()))?;
    if supplied.version == BytecodeVersion::LEGACY && original_resource_unchanged {
        runtime
            .retain_legacy_import_artifact(supplied)
            .map_err(|err| CompileError::new(err.to_string()))?;
    }
    Ok(runtime)
}

/// Source locations are certified only when every semantic byte/record matches.
/// File labels may change when a bundle moves; index counts and source positions
/// remain exact, and the metadata records the explicit corresponding labels.
///
/// LINE_TABLE is optional (specification 12 §6.13): an artifact written before it existed is
/// the same program, so it is compared only when both sides carry one. Every other section
/// must match, as before.
fn equivalent_source_artifact(expected: &BytecodeModule, supplied: &BytecodeModule) -> bool {
    let line_tables_agree = match (expected.line_table(), supplied.line_table()) {
        (Some(expected), Some(supplied)) => expected == supplied,
        _ => true,
    };
    let expected_sections = sections_but_line_table(&expected.sections);
    let supplied_sections = sections_but_line_table(&supplied.sections);
    line_tables_agree
        && expected.version == supplied.version
        && expected.flags == supplied.flags
        && expected_sections.len() == supplied_sections.len()
        && expected_sections
            .iter()
            .zip(&supplied_sections)
            .all(|(a, b)| {
                a.id == b.id
                    && a.flags == b.flags
                    && match (&a.data, &b.data) {
                        (SectionData::DebugStringTable(a), SectionData::DebugStringTable(b)) => {
                            a.entries.len() == b.entries.len()
                        }
                        _ => a.data == b.data,
                    }
            })
}

/// The sections that every source-identity comparison requires, LINE_TABLE left out.
fn sections_but_line_table(sections: &[Section]) -> Vec<&Section> {
    sections
        .iter()
        .filter(|section| !matches!(section.data, SectionData::LineTable(_)))
        .collect()
}

fn select_resource(
    module: &mut BytecodeModule,
    requested: Option<&str>,
) -> Result<usize, CompileError> {
    let Some(name) = requested else { return Ok(0) };
    let metadata = module
        .metadata()
        .map_err(|err| CompileError::new(err.to_string()))?;
    if let Some(index) = configured_resource(&metadata, name)? {
        return Ok(index);
    }
    let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable) else {
        return Err(CompileError::new("missing string table"));
    };
    let index = u32::try_from(strings.entries.len())
        .map_err(|_| CompileError::new("string table overflow"))?;
    strings.entries.push(name.into());
    let Some(SectionData::ResourceMeta(resources)) = module.section_mut(SectionId::ResourceMeta)
    else {
        return Err(CompileError::new("missing resource metadata"));
    };
    resources.resources[0].name_idx = index;
    Ok(0)
}

/// The launch rule for the configured resource (spec 11 §6.7.1): `Some(index)` for the resource
/// whose name equals `name` case-insensitively, `None` when the module declares only the legacy
/// placeholder `RESOURCE`, which launch then renames to `name`. Any other name is refused.
pub fn configured_resource(
    metadata: &BytecodeMetadata,
    name: &str,
) -> Result<Option<usize>, CompileError> {
    if let Some(index) = metadata
        .resources
        .iter()
        .position(|resource| resource.name.eq_ignore_ascii_case(name))
    {
        return Ok(Some(index));
    }
    if metadata.resources.len() == 1 && metadata.resources[0].name.eq_ignore_ascii_case("RESOURCE")
    {
        return Ok(None);
    }
    Err(CompileError::new(format!(
        "resource '{name}' not found in supplied bytecode"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::CompileSession;

    #[test]
    fn artifact_without_line_table_keeps_its_source_association_and_statement_index_path() {
        // Specification 12 §6.13: LINE_TABLE is optional, so a 2.0/3.0 artifact written before it
        // existed is the same program. Its source association survives, and its breakpoints still
        // resolve through the statement index, as specification 13 "Source Mapping" describes.
        let source = "PROGRAM Main\nVAR\nx : INT;\nEND_VAR\nx := 1;\nEND_PROGRAM\n";
        let session =
            CompileSession::from_sources(vec![SourceFile::with_path("src/main.st", source)]);
        let module = session
            .build_bytecode_module()
            .expect("the compiler builds the module");
        assert!(
            module.line_table().is_some(),
            "the compiler writes LINE_TABLE for source-built modules"
        );
        let mut before_line_table = module.clone();
        before_line_table
            .sections
            .retain(|section| !matches!(section.data, SectionData::LineTable(_)));
        assert!(before_line_table.line_table().is_none());

        let stripped = session
            .build_runtime_from_bytecode(&before_line_table, None)
            .expect("the artifact without LINE_TABLE installs");
        let stripped_metadata = stripped.metadata_snapshot();
        let file_id = stripped_metadata
            .source_file_id("src/main.st")
            .expect("the source file has an id");
        assert!(
            stripped_metadata.line_table().is_none(),
            "without LINE_TABLE the installation resolves through its statement index"
        );
        assert!(
            stripped.statement_locations(file_id).is_some(),
            "the source association survives the missing LINE_TABLE"
        );
        // Hand-derived from the source: `x := 1;` is 0-based line 4 and starts at column 0.
        let expected = Some((4, 0));
        let resolved = stripped
            .resolve_breakpoint_position(source, file_id, 4, 0)
            .map(|(_, line, column)| (line, column));
        assert_eq!(resolved, expected, "statement-index answer");

        let built = session
            .build_runtime()
            .expect("the same sources build with LINE_TABLE");
        assert!(
            built.metadata_snapshot().line_table().is_some(),
            "with LINE_TABLE the installation resolves through the table"
        );
        let resolved = built
            .resolve_breakpoint_position(source, file_id, 4, 0)
            .map(|(_, line, column)| (line, column));
        assert_eq!(resolved, expected, "table answer");
    }
}
