//! Source/context association around supplied artifacts, never replacement bodies.
use super::{build, CompileError, SourceFile};
use crate::{
    bytecode::{BytecodeMetadata, BytecodeModule, BytecodeVersion, SectionData, SectionId},
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
    let mut runtime = Runtime::from_module_with_metadata(module, metadata, resource)
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
fn equivalent_source_artifact(expected: &BytecodeModule, supplied: &BytecodeModule) -> bool {
    expected.version == supplied.version
        && expected.flags == supplied.flags
        && expected.sections.len() == supplied.sections.len()
        && expected
            .sections
            .iter()
            .zip(&supplied.sections)
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
