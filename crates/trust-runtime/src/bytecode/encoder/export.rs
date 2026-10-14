//! Optional debugger presentation over an unchanged installed artifact.
use super::{BytecodeError, BytecodeModule, SectionData, SectionId};

pub(super) fn export_with_sources(
    runtime: &crate::Runtime,
    sources: &[&str],
    paths: Option<&[&str]>,
) -> Result<BytecodeModule, BytecodeError> {
    let metadata = runtime.tooling_metadata();
    let mut module = runtime.deployment_artifact()?.clone();
    if !matches!(module.section(SectionId::DebugMap), Some(SectionData::DebugMap(map)) if !map.entries.is_empty())
    {
        return Err(BytecodeError::InvalidSection(
            "installed debug positions unavailable".into(),
        ));
    }
    if !matches!(module.section(SectionId::DebugStringTable), Some(SectionData::DebugStringTable(strings)) if !strings.entries.is_empty())
    {
        return Err(BytecodeError::InvalidSection(
            "debug source identity unavailable".into(),
        ));
    }
    // Relabel only file strings already tied to admitted instruction positions.
    // Generating a new map from current PLC values or an unrelated source would
    // lose initializer semantics or attach misleading debugger positions.
    if let Some(SectionData::DebugStringTable(strings)) =
        module.section_mut(SectionId::DebugStringTable)
    {
        for label in &mut strings.entries {
            let id = metadata.source_file_id(label).ok_or_else(|| {
                BytecodeError::InvalidSection("debug source identity unavailable".into())
            })?;
            let source = sources
                .get(id as usize)
                .ok_or_else(|| BytecodeError::InvalidSection("debug source missing".into()))?;
            let installed = metadata.source_text(id).ok_or_else(|| {
                BytecodeError::InvalidSection("installed source identity unavailable".into())
            })?;
            if installed != *source {
                return Err(BytecodeError::InvalidSection(
                    "debug source differs from installed artifact".into(),
                ));
            }
            *label = match paths {
                Some(paths) => paths
                    .get(id as usize)
                    .ok_or_else(|| BytecodeError::InvalidSection("debug path missing".into()))?
                    .to_string(),
                None => format!("file_{id}"),
            }
            .into();
        }
    }
    module.validate()?;
    Ok(module)
}
