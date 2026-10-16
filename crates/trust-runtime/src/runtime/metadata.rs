//! Runtime metadata snapshots.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::debug::SourceLocation;
use crate::memory::{AccessMap, FrameId, LocalFrame, VariableStorage};
use crate::program_model::{ClassDef, FunctionBlockDef, FunctionDef, InterfaceDef};
use crate::stdlib::StandardLibrary;
use crate::task::{ProgramDef, TaskConfig};
use crate::value::DateTimeProfile;
use trust_hir::types::TypeRegistry;

mod prepared;

mod retained;
use retained::{RetainedBinding, RetainedLabel};

#[derive(Debug, Clone)]
pub(crate) struct GlobalDeclarationInfo {
    pub(crate) type_id: trust_hir::TypeId,
    pub(crate) retain: crate::RetainPolicy,
}

/// Snapshot of runtime metadata needed by external tooling.
#[derive(Debug, Clone)]
pub struct RuntimeMetadata {
    pub(super) storage_abi: crate::value::StorageAbi,
    pub(super) profile: DateTimeProfile,
    pub(super) registry: TypeRegistry,
    pub(crate) artifact_types: Vec<Option<trust_hir::TypeId>>,
    pub(crate) io_display_names: Vec<Option<SmolStr>>,
    pub(super) retained_bindings: IndexMap<RetainedLabel, RetainedBinding>,
    pub(super) global_declarations: IndexMap<SmolStr, GlobalDeclarationInfo>,
    pub(super) stdlib: StandardLibrary,
    pub(super) access: AccessMap,
    pub(super) functions: IndexMap<SmolStr, FunctionDef>,
    pub(super) function_blocks: IndexMap<SmolStr, FunctionBlockDef>,
    pub(super) classes: IndexMap<SmolStr, ClassDef>,
    pub(super) interfaces: IndexMap<SmolStr, InterfaceDef>,
    pub(super) programs: IndexMap<SmolStr, ProgramDef>,
    pub(super) program_templates: IndexMap<SmolStr, ProgramDef>,
    pub(super) tasks: Vec<TaskConfig>,
    pub(super) task_thread_ids: IndexMap<SmolStr, u32>,
    pub(super) background_thread_id: Option<u32>,
    pub(super) statement_index: IndexMap<u32, Vec<SourceLocation>>,
    pub(super) source_text_index: IndexMap<u32, String>,
    pub(super) source_label_index: std::collections::HashMap<SmolStr, u32>,
}

impl RuntimeMetadata {
    /// Storage widths certified by the installed artifact, independent of source defaults.
    pub fn storage_abi(&self) -> crate::value::StorageAbi {
        self.storage_abi
    }

    /// Access the type registry snapshot.
    #[must_use]
    pub fn registry(&self) -> &TypeRegistry {
        &self.registry
    }

    /// Resolve an artifact type to its distinct, immutable tooling identity.
    /// TYPE_TABLE indexes are never cast to HIR type ids.
    #[must_use]
    pub fn artifact_type(&self, index: u32) -> Option<trust_hir::TypeId> {
        self.artifact_types.get(index as usize).copied().flatten()
    }

    /// Keep verified declarations and USING context while refusing uncertified
    /// source locations, breakpoint attachment and source-aware artifact export.
    pub(crate) fn clear_source_association(&mut self) {
        self.statement_index.clear();
        self.source_text_index.clear();
        self.source_label_index.clear();
    }

    /// Associate path-only artifact relabels after the authoring boundary proved
    /// identical semantic tables, instruction bytes and indexed source positions.
    pub(crate) fn associate_artifact_source_paths(
        &mut self,
        expected: &crate::bytecode::BytecodeModule,
        supplied: &crate::bytecode::BytecodeModule,
    ) -> Result<(), crate::error::RuntimeError> {
        use crate::bytecode::{SectionData, SectionId};
        let (Some(SectionData::DebugStringTable(a)), Some(SectionData::DebugStringTable(b))) = (
            expected.section(SectionId::DebugStringTable),
            supplied.section(SectionId::DebugStringTable),
        ) else {
            return Ok(());
        };
        // Resolve every source identity before replacing aliases. A bundle may
        // legitimately permute unique path labels; old aliases must not make
        // an otherwise identical indexed source map appear ambiguous.
        let mut installed_labels = std::collections::HashMap::new();
        for (original, installed) in a.entries.iter().zip(&b.entries) {
            let id = self
                .source_file_id(original)
                .ok_or(crate::error::RuntimeError::InvalidExecutionState)?;
            if installed_labels
                .insert(installed.clone(), id)
                .is_some_and(|prior| prior != id)
            {
                return Err(crate::error::RuntimeError::InvalidConfig(
                    "ambiguous artifact source labels".into(),
                ));
            }
        }
        self.source_label_index.extend(installed_labels);
        Ok(())
    }

    pub(crate) fn source_file_id(&self, label: &str) -> Option<u32> {
        self.source_label_index
            .get(label)
            .copied()
            .or_else(|| label.strip_prefix("file_")?.parse().ok())
    }

    pub(crate) fn source_text(&self, file_id: u32) -> Option<&str> {
        self.source_text_index.get(&file_id).map(String::as_str)
    }

    /// Access the standard library snapshot.
    #[must_use]
    pub fn stdlib(&self) -> &StandardLibrary {
        &self.stdlib
    }

    /// Access interface definitions.
    #[must_use]
    pub fn interfaces(&self) -> &IndexMap<SmolStr, InterfaceDef> {
        &self.interfaces
    }

    /// Access the VAR_ACCESS binding map snapshot.
    #[must_use]
    pub fn access_map(&self) -> &AccessMap {
        &self.access
    }

    /// Access configured tasks.
    #[must_use]
    pub fn tasks(&self) -> &[TaskConfig] {
        &self.tasks
    }

    /// Access program definitions.
    #[must_use]
    pub fn programs(&self) -> &IndexMap<SmolStr, ProgramDef> {
        &self.programs
    }

    /// Resolve a stable thread id for a task name.
    #[must_use]
    pub fn task_thread_id(&self, name: &SmolStr) -> Option<u32> {
        self.tasks
            .iter()
            .any(|task| task.name == *name)
            .then(|| self.task_thread_ids.get(name).copied())
            .flatten()
    }

    /// Access the background thread id, if assigned.
    #[must_use]
    pub fn background_thread_id(&self) -> Option<u32> {
        self.background_thread_id
    }

    /// Determine whether any programs run outside configured tasks.
    #[must_use]
    pub fn has_background_programs(&self) -> bool {
        self.programs.keys().any(|name| {
            !self.tasks.iter().any(|task| {
                task.programs
                    .iter()
                    .any(|program| program.eq_ignore_ascii_case(name.as_str()))
            })
        })
    }

    /// Resolve USING directives for the given frame id.
    #[must_use]
    pub fn using_for_frame(
        &self,
        storage: &VariableStorage,
        frame_id: FrameId,
    ) -> Option<Vec<SmolStr>> {
        let frame = storage.frames().iter().find(|frame| frame.id == frame_id)?;
        resolve_using_for_frame(
            frame,
            storage,
            &self.functions,
            &self.function_blocks,
            &self.classes,
            &self.programs,
        )
        .or_else(|| {
            self.program_templates
                .values()
                .find(|program| program.name.eq_ignore_ascii_case(frame.owner.as_str()))
                .map(|program| program.using.as_slice())
        })
        .map(|using| using.to_vec())
    }

    /// Return statement locations for a file id, if known.
    #[must_use]
    pub fn statement_locations(&self, file_id: u32) -> Option<&[SourceLocation]> {
        self.statement_index.get(&file_id).map(Vec::as_slice)
    }

    /// Resolve a line/column breakpoint to the nearest statement boundary.
    #[must_use]
    pub fn resolve_breakpoint_location(
        &self,
        source: &str,
        file_id: u32,
        line: u32,
        column: u32,
    ) -> Option<SourceLocation> {
        let locations = self.statement_index.get(&file_id)?;
        crate::debug::resolve_breakpoint_location(source, file_id, locations, line, column)
    }

    /// Resolve an artifact source position against this installation's immutable source snapshot.
    #[must_use]
    pub(crate) fn resolve_vm_debug_location(
        &self,
        file_label: &str,
        line_1based: u32,
        column_1based: u32,
    ) -> Option<SourceLocation> {
        let file_id = self
            .source_label_index
            .get(file_label)
            .copied()
            .or_else(|| file_label.strip_prefix("file_")?.parse::<u32>().ok())?;
        let source = self.source_text_index.get(&file_id)?;
        self.resolve_breakpoint_location(
            source,
            file_id,
            line_1based.saturating_sub(1),
            column_1based.saturating_sub(1),
        )
    }

    /// Resolve a breakpoint to a statement boundary and return adjusted line/column.
    #[must_use]
    pub fn resolve_breakpoint_position(
        &self,
        source: &str,
        file_id: u32,
        line: u32,
        column: u32,
    ) -> Option<(SourceLocation, u32, u32)> {
        let location = self.resolve_breakpoint_location(source, file_id, line, column)?;
        let (resolved_line, resolved_col) = crate::debug::location_to_line_col(source, &location);
        Some((location, resolved_line, resolved_col))
    }

    /// Access the profile snapshot.
    #[must_use]
    pub fn profile(&self) -> DateTimeProfile {
        self.profile
    }

    /// Access function definitions.
    #[must_use]
    pub fn functions(&self) -> &IndexMap<SmolStr, FunctionDef> {
        &self.functions
    }

    /// Access function block definitions.
    #[must_use]
    pub fn function_blocks(&self) -> &IndexMap<SmolStr, FunctionBlockDef> {
        &self.function_blocks
    }

    /// Access class definitions.
    #[must_use]
    pub fn classes(&self) -> &IndexMap<SmolStr, ClassDef> {
        &self.classes
    }
}

pub(super) fn resolve_using_for_frame<'a>(
    frame: &LocalFrame,
    storage: &VariableStorage,
    functions: &'a IndexMap<SmolStr, FunctionDef>,
    function_blocks: &'a IndexMap<SmolStr, FunctionBlockDef>,
    classes: &'a IndexMap<SmolStr, ClassDef>,
    programs: &'a IndexMap<SmolStr, ProgramDef>,
) -> Option<&'a [SmolStr]> {
    if let Some(instance_id) = frame.instance_id {
        let instance = storage.get_instance(instance_id)?;
        let type_key = SmolStr::new(instance.type_name.to_ascii_uppercase());

        if let Some(fb) = function_blocks.get(&type_key) {
            if let Some(method) = fb
                .methods
                .iter()
                .find(|method| method.name.eq_ignore_ascii_case(frame.owner.as_ref()))
            {
                if method.using.is_empty() {
                    return Some(fb.using.as_slice());
                }
                return Some(method.using.as_slice());
            }
            if fb.name.eq_ignore_ascii_case(frame.owner.as_ref()) {
                return Some(fb.using.as_slice());
            }
        }

        if let Some(class_def) = classes.get(&type_key) {
            if let Some(method) = class_def
                .methods
                .iter()
                .find(|method| method.name.eq_ignore_ascii_case(frame.owner.as_ref()))
            {
                if method.using.is_empty() {
                    return Some(class_def.using.as_slice());
                }
                return Some(method.using.as_slice());
            }
            if class_def.name.eq_ignore_ascii_case(frame.owner.as_ref()) {
                return Some(class_def.using.as_slice());
            }
        }
    }

    let key = SmolStr::new(frame.owner.to_ascii_uppercase());
    if let Some(func) = functions.get(&key) {
        return Some(func.using.as_slice());
    }
    if let Some(program) = programs
        .values()
        .find(|program| program.name.eq_ignore_ascii_case(frame.owner.as_ref()))
    {
        return Some(program.using.as_slice());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::runtime::LegacyRuntimeFixture as Runtime;
    use trust_hir::TypeId;

    #[test]
    fn metadata_using_resolution_is_case_insensitive_for_functions_and_programs() {
        let mut runtime = Runtime::new();
        runtime.register_function(FunctionDef {
            name: "Compute".into(),
            return_type: TypeId::DINT,
            params: Vec::new(),
            locals: Vec::new(),
            static_locals: Vec::new(),
            using: vec!["FunctionLib".into()],
            body: Vec::new(),
        });
        runtime
            .register_program(ProgramDef {
                name: "Main".into(),
                vars: Vec::new(),
                temps: Vec::new(),
                using: vec!["ProgramLib".into()],
                body: Vec::new(),
            })
            .unwrap();

        let function_frame = runtime.storage_mut().push_frame("cOmPuTe");
        assert_eq!(
            runtime.using_for_frame(function_frame),
            Some(vec![SmolStr::new("FunctionLib")])
        );
        runtime.storage_mut().pop_frame();

        let program_frame = runtime.storage_mut().push_frame("mAiN");
        assert_eq!(
            runtime.using_for_frame(program_frame),
            Some(vec![SmolStr::new("ProgramLib")])
        );
    }

    #[test]
    fn metadata_background_classification_is_case_insensitive() {
        let mut runtime = Runtime::new();
        runtime
            .register_program(ProgramDef {
                name: "Main".into(),
                vars: Vec::new(),
                temps: Vec::new(),
                using: Vec::new(),
                body: Vec::new(),
            })
            .unwrap();
        runtime.register_task(TaskConfig {
            name: "Fast".into(),
            interval: crate::value::Duration::from_millis(10),
            single: None,
            priority: 1,
            programs: vec!["mAiN".into()],
            fb_instances: Vec::new(),
        });

        let snapshot = runtime.metadata_snapshot();
        assert!(!snapshot.has_background_programs());
    }
}
