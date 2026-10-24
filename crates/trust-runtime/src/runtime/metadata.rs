//! Runtime metadata snapshots.

#![allow(missing_docs)]

use std::collections::BTreeMap;

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::bytecode::{BytecodeModule, LineStatement, LineTable, SectionData, SectionId};
use crate::debug::SourceLocation;
use crate::memory::{AccessMap, FrameId, LocalFrame, VariableStorage};
use crate::program_model::{ClassDef, FunctionBlockDef, FunctionDef, InterfaceDef};
use crate::stdlib::StandardLibrary;
use crate::task::{ProgramDef, TaskConfig};
use crate::value::DateTimeProfile;
use trust_hir::types::TypeRegistry;
use trust_runtime_core::debug::LineResolver;

mod prepared;

mod retained;
use retained::{RetainedBinding, RetainedLabel};

#[derive(Debug, Clone)]
pub(crate) struct GlobalDeclarationInfo {
    pub(crate) type_id: trust_hir::TypeId,
    pub(crate) retain: crate::RetainPolicy,
}

/// The installed module's LINE_TABLE, bound to this installation's source text (specification 12
/// §6.13; specification 13, "Source Mapping").
///
/// The table decides every statement start, breakpoint and stop position. The text only projects
/// the table's positions to the byte offsets that hooks and adapters carry, using the same
/// coordinates as the statement index it replaces, so it decides no answer. Built once at install:
/// each lookup afterwards is an index read, never a scan.
#[derive(Debug, Clone)]
pub(crate) struct LineIndex {
    table: LineTable,
    /// DEBUG_STRING_TABLE: `labels[path_idx]` names a file entry.
    labels: Vec<SmolStr>,
    /// The installation's file id of each file entry, when it has one.
    file_ids: Vec<Option<u32>>,
    /// The byte span of each table statement, when its file's text is installed.
    spans: Vec<Option<SourceLocation>>,
    /// The innermost statement that starts at `(pou_id, start_code_offset)`: the last such entry,
    /// because the table lists an outer statement first (specification 12 §6.13).
    by_start: BTreeMap<(u32, u32), usize>,
    /// The statements that start at `(file entry, start line, start column)`, 1-based, in table order.
    by_position: BTreeMap<(u32, u32, u32), Vec<usize>>,
    /// The spans of each file id's statements, in table order: the breakpoint-location listing.
    listing: BTreeMap<u32, Vec<SourceLocation>>,
}

impl LineIndex {
    /// Bind the module's LINE_TABLE to the installation. `None` when the module has no LINE_TABLE, so
    /// the installation keeps resolving through its statement index (specification 12 §10).
    fn bind(module: &BytecodeModule, metadata: &RuntimeMetadata) -> Option<Self> {
        let table = module.line_table()?.clone();
        let Some(SectionData::DebugStringTable(strings)) =
            module.section(SectionId::DebugStringTable)
        else {
            return None;
        };
        let labels = strings.entries.clone();
        let file_ids: Vec<Option<u32>> = table
            .files
            .iter()
            .map(|file| {
                labels
                    .get(file.path_idx as usize)
                    .and_then(|label| metadata.source_file_id(label))
            })
            .collect();
        let mut coordinates: BTreeMap<u32, LineStarts> = BTreeMap::new();
        for file_id in file_ids.iter().flatten() {
            if let Some(starts) = metadata.source_text(*file_id).and_then(LineStarts::of) {
                coordinates.insert(*file_id, starts);
            }
        }
        let spans: Vec<Option<SourceLocation>> = table
            .statements
            .iter()
            .map(|statement| {
                let file_id = file_ids
                    .get(statement.file_index as usize)
                    .copied()
                    .flatten()?;
                span(statement, file_id, coordinates.get(&file_id)?)
            })
            .collect();
        let mut by_start = BTreeMap::new();
        let mut by_position: BTreeMap<(u32, u32, u32), Vec<usize>> = BTreeMap::new();
        for (index, statement) in table.statements.iter().enumerate() {
            by_start.insert((statement.pou_id, statement.start_code_offset), index);
            by_position
                .entry((
                    statement.file_index,
                    statement.start_line,
                    statement.start_column,
                ))
                .or_default()
                .push(index);
        }
        let mut listing: BTreeMap<u32, Vec<SourceLocation>> = BTreeMap::new();
        for location in spans.iter().flatten() {
            listing.entry(location.file_id).or_default().push(*location);
        }
        Some(Self {
            table,
            labels,
            file_ids,
            spans,
            by_start,
            by_position,
            listing,
        })
    }

    /// The byte span of the statement that starts at `(pou_id, code_offset)`, when the table has
    /// one there and its file's text is installed.
    #[must_use]
    pub(crate) fn start_location(&self, pou_id: u32, code_offset: u32) -> Option<SourceLocation> {
        let index = *self.by_start.get(&(pou_id, code_offset))?;
        self.spans.get(index).copied().flatten()
    }

    /// Resolve a 1-based `(line, column)` request on `file_id` to the statement the table selects
    /// (specification 13, "Breakpoint evaluation, source resolution, and hook contract"). Returns
    /// its byte span and its 1-based start.
    #[must_use]
    fn resolve_breakpoint(
        &self,
        file_id: u32,
        line: u32,
        column: u32,
    ) -> Option<(SourceLocation, u32, u32)> {
        let file_index = self.file_ids.iter().position(|id| *id == Some(file_id))?;
        let path_idx = self.table.files.get(file_index)?.path_idx;
        let label = self.labels.get(path_idx as usize)?;
        let resolved = LineResolver::new(&self.table, &self.labels).resolve_breakpoint(
            label.as_str(),
            line,
            column,
        )?;
        let index = self
            .by_position
            .get(&(
                u32::try_from(file_index).ok()?,
                resolved.line,
                resolved.column,
            ))?
            .iter()
            .copied()
            .find(|index| {
                self.table.statements.get(*index).is_some_and(|statement| {
                    statement.pou_id == resolved.pou_id
                        && statement.start_code_offset == resolved.code_offset
                        && statement.end_line == resolved.end_line
                        && statement.end_column == resolved.end_column
                })
            })?;
        let location = self.spans.get(index).copied().flatten()?;
        Some((location, resolved.line, resolved.column))
    }
}

/// The line starts of one source text and its byte length, counted as the runtime counts byte
/// offsets: offset zero, then the byte after every LF (specification 13, "Runtime source
/// coordinates").
#[derive(Debug, Clone)]
struct LineStarts {
    starts: Vec<u32>,
    len: u32,
}

impl LineStarts {
    fn of(text: &str) -> Option<Self> {
        let len = u32::try_from(text.len()).ok()?;
        let mut starts = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(u32::try_from(index + 1).ok()?);
            }
        }
        Some(Self { starts, len })
    }

    /// The byte offset of a 1-based `(line, column)`, when it lies inside the text.
    fn offset(&self, line: u32, column: u32) -> Option<u32> {
        let line_start = *self
            .starts
            .get(usize::try_from(line.checked_sub(1)?).ok()?)?;
        let offset = line_start.checked_add(column.checked_sub(1)?)?;
        (offset <= self.len).then_some(offset)
    }
}

/// The byte span of a table statement, from its first token to the end of its source span with
/// trailing trivia (specification 12 §6.13).
fn span(statement: &LineStatement, file_id: u32, starts: &LineStarts) -> Option<SourceLocation> {
    let start = starts.offset(statement.start_line, statement.start_column)?;
    let end = starts.offset(statement.end_line, statement.end_column)?;
    Some(SourceLocation::new(file_id, start, end))
}

/// Snapshot of runtime metadata needed by external tooling. A compiled program has none of the
/// bytecode engine's program model, so its metadata is the empty default (specification 44 §8.8).
#[derive(Debug, Clone, Default)]
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
    /// The installed module's LINE_TABLE, when the installed module carries one.
    pub(super) line_table: Option<LineIndex>,
}

impl RuntimeMetadata {
    /// Storage widths certified by the installed artifact, independent of source defaults.
    pub fn storage_abi(&self) -> crate::value::StorageAbi {
        self.storage_abi
    }

    /// Bind the installed module's LINE_TABLE, replacing any earlier binding. A module without a
    /// LINE_TABLE leaves the installation on its statement index.
    pub(crate) fn bind_line_table(&mut self, module: &BytecodeModule) {
        self.line_table = LineIndex::bind(module, self);
    }

    /// The bound LINE_TABLE, when the installed module carries one.
    #[must_use]
    pub(crate) fn line_table(&self) -> Option<&LineIndex> {
        self.line_table.as_ref()
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
        self.line_table = None;
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

    /// The statement list the source text provides for a file id: the statement index that the
    /// source-based resolver answered from before a LINE_TABLE did (specification 13, "Breakpoint
    /// evaluation, source resolution, and hook contract"). The reference a line table is compared
    /// with; the installation's own answers read the table once one is bound.
    #[must_use]
    pub fn source_statement_locations(&self, file_id: u32) -> Option<&[SourceLocation]> {
        self.statement_index.get(&file_id).map(Vec::as_slice)
    }

    /// Return statement locations for a file id, if known. With a LINE_TABLE these are the table's
    /// statements (specification 13, "Breakpoint Locations"); without one, the statement index.
    #[must_use]
    pub fn statement_locations(&self, file_id: u32) -> Option<&[SourceLocation]> {
        match &self.line_table {
            Some(table) => table.listing.get(&file_id).map(Vec::as_slice),
            None => self.statement_index.get(&file_id).map(Vec::as_slice),
        }
    }

    /// Resolve a line/column breakpoint to the nearest statement boundary. `line` and `column` are
    /// 0-based, as the control layer sends them; with a LINE_TABLE the table decides the statement.
    #[must_use]
    pub fn resolve_breakpoint_location(
        &self,
        source: &str,
        file_id: u32,
        line: u32,
        column: u32,
    ) -> Option<SourceLocation> {
        if let Some(table) = &self.line_table {
            // The table is 1-based (specification 12 §6.13); the boundary converts once.
            let (location, _, _) =
                table.resolve_breakpoint(file_id, line.checked_add(1)?, column.checked_add(1)?)?;
            return Some(location);
        }
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

    /// Resolve a breakpoint to a statement boundary and return its 0-based line and column.
    #[must_use]
    pub fn resolve_breakpoint_position(
        &self,
        source: &str,
        file_id: u32,
        line: u32,
        column: u32,
    ) -> Option<(SourceLocation, u32, u32)> {
        if let Some(table) = &self.line_table {
            // The table's 1-based start answers the reply; the boundary converts it to 0-based.
            let (location, resolved_line, resolved_column) =
                table.resolve_breakpoint(file_id, line.checked_add(1)?, column.checked_add(1)?)?;
            return Some((
                location,
                resolved_line.checked_sub(1)?,
                resolved_column.checked_sub(1)?,
            ));
        }
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
