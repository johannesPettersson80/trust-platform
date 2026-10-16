//! Assemble hosted tooling metadata from admitted types, POUs and resource records.
//!
//! HIR identities never enter the engine. Source text, USING lists and executable
//! AST bodies absent from the artifact are deliberately not reconstructed.
use super::{
    AccessMap, DateTimeProfile, GlobalDeclarationInfo, IndexMap, InterfaceDef, RetainedBinding,
    RetainedLabel, RuntimeMetadata, SmolStr, StandardLibrary, TaskConfig,
};
use crate::{error::RuntimeError, program_model::MethodDef};
use trust_runtime_core::{
    bytecode::{StorageOwner, StorageRole, TypeData},
    vm::PreparedModule,
};
mod pous;
mod types;
use pous::PouDeclarations;
use types::{primitive, Projection};

impl RuntimeMetadata {
    pub(in crate::runtime) fn bind_retained_declarations(
        &mut self,
        prepared: &PreparedModule,
    ) -> Result<(), RuntimeError> {
        let mut bindings = IndexMap::new();
        let name = |id: u32| {
            prepared
                .strings()
                .get(id as usize)
                .cloned()
                .ok_or(RuntimeError::InvalidExecutionState)
        };
        for declaration in prepared.storage_declarations() {
            if declaration.owner == StorageOwner::Global
                && declaration.is_retained()
                && declaration.role != StorageRole::ProgramRoot
                && declaration.owns_storage()
            {
                let name = name(declaration.name_idx)?;
                insert_retained_binding(
                    &mut bindings,
                    RetainedLabel(name.clone()),
                    RetainedBinding { root: None, name },
                )?;
            }
        }
        for root in prepared.construction_roots() {
            let declaration = prepared
                .storage_declarations()
                .get(root.declaration_idx as usize)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            if root.parent_root_idx.is_some() || declaration.role != StorageRole::ProgramRoot {
                continue;
            }
            let program = name(declaration.name_idx)?;
            let template = root
                .template_pou_id
                .ok_or(RuntimeError::InvalidExecutionState)?;
            for member in prepared.instance_declarations(template) {
                if !member.is_retained() || !member.owns_storage() {
                    continue;
                }
                let member = name(member.name_idx)?;
                insert_retained_binding(
                    &mut bindings,
                    RetainedLabel(format!("@program/{program}/{member}").into()),
                    RetainedBinding {
                        root: Some(program.clone()),
                        name: member,
                    },
                )?;
            }
        }
        self.retained_bindings = bindings;
        Ok(())
    }

    pub(in crate::runtime) fn bind_global_declarations(
        &mut self,
        prepared: &PreparedModule,
    ) -> Result<(), RuntimeError> {
        self.global_declarations.clear();
        for declaration in prepared.storage_declarations() {
            if declaration.owner != StorageOwner::Global
                || declaration.role != StorageRole::Variable
            {
                continue;
            }
            let name = prepared
                .strings()
                .get(declaration.name_idx as usize)
                .cloned()
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let type_id = declaration
                .type_id
                .and_then(|id| self.artifact_type(id))
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let retain = match declaration.retain {
                0 => crate::RetainPolicy::Unspecified,
                1 => crate::RetainPolicy::Retain,
                2 => crate::RetainPolicy::NonRetain,
                3 => crate::RetainPolicy::Persistent,
                _ => return Err(RuntimeError::InvalidExecutionState),
            };
            self.global_declarations
                .insert(name, GlobalDeclarationInfo { type_id, retain });
        }
        Ok(())
    }

    pub(in crate::runtime) fn bind_known_artifact_types(
        &mut self,
        prepared: &PreparedModule,
    ) -> Result<(), RuntimeError> {
        self.artifact_types
            .resize(prepared.type_table().entries.len(), None);
        for (index, entry) in prepared.type_table().entries.iter().enumerate() {
            if self.artifact_types[index].is_some() {
                continue;
            }
            let named = entry
                .name_idx
                .and_then(|id| prepared.strings().get(id as usize))
                .and_then(|name| self.registry.lookup(name));
            if let Some(id) = named {
                self.artifact_types[index] = Some(id);
                continue;
            }
            self.artifact_types[index] = match entry.data {
                TypeData::Primitive {
                    prim_id: 24,
                    max_length,
                } if max_length != 0 => Some(
                    self.registry
                        .register_string_with_length(u32::from(max_length)),
                ),
                TypeData::Primitive {
                    prim_id: 25,
                    max_length,
                } if max_length != 0 => Some(
                    self.registry
                        .register_wstring_with_length(u32::from(max_length)),
                ),
                TypeData::Primitive {
                    prim_id,
                    max_length: 0,
                } => Some(primitive(prim_id)?),
                _ => None,
            };
        }
        Ok(())
    }

    /// Reconstruct read-only tooling declarations from admitted STBC metadata.
    ///
    /// No source text, USING clauses or executable AST bodies are available here.
    /// Interface records carry names/slots only: their source-level signatures and
    /// inheritance cannot be recovered and are not inferred from implementors.
    /// Task instance references and engineering aliases are populated by the host
    /// after instantiation through the core's checked persistent-reference resolver.
    pub(crate) fn from_prepared(
        prepared: &PreparedModule,
        resource_index: usize,
    ) -> Result<Self, RuntimeError> {
        let projection = Projection::new(prepared)?;
        let resource = prepared
            .resources()
            .get(resource_index)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let PouDeclarations {
            functions,
            function_blocks,
            classes,
            program_templates,
        } = projection.pous()?;
        let mut interfaces = IndexMap::new();
        for (id, entry) in prepared.type_table().entries.iter().enumerate() {
            if let TypeData::Interface { methods } = &entry.data {
                let name = projection
                    .registry
                    .type_name(projection.wire_types[id])
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let methods = methods
                    .iter()
                    .map(|method| {
                        Ok(MethodDef {
                            name: projection.name(method.name_idx)?,
                            return_type: None,
                            params: Vec::new(),
                            locals: Vec::new(),
                            static_locals: Vec::new(),
                            using: Vec::new(),
                            body: Vec::new(),
                        })
                    })
                    .collect::<Result<_, RuntimeError>>()?;
                interfaces.insert(
                    name.to_ascii_uppercase().into(),
                    InterfaceDef {
                        name,
                        base: None,
                        using: Vec::new(),
                        methods,
                    },
                );
            }
        }
        let mut programs = IndexMap::new();
        for declaration in prepared.storage_declarations() {
            if declaration.owner != StorageOwner::Global
                || declaration.role != StorageRole::ProgramRoot
            {
                continue;
            }
            let owner = declaration
                .owner_pou_id
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let template = prepared
                .pou_name(owner)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let mut definition = program_templates
                .get(&SmolStr::new(template.to_ascii_uppercase()))
                .cloned()
                .ok_or(RuntimeError::InvalidExecutionState)?;
            definition.name = projection.name(declaration.name_idx)?;
            programs.insert(definition.name.clone(), definition);
        }
        let mut tasks = Vec::new();
        let mut task_thread_ids = IndexMap::new();
        let mut next_thread = 1u32;
        for task in &resource.tasks {
            let name = projection.name(task.name_idx)?;
            if !task_thread_ids.contains_key(&name) {
                task_thread_ids.insert(name.clone(), next_thread);
                next_thread = next_thread.checked_add(1).ok_or(RuntimeError::Overflow)?;
            }
            tasks.push(TaskConfig {
                name,
                interval: crate::value::Duration::from_nanos(task.interval_nanos),
                priority: task.priority,
                single: task
                    .single_name_idx
                    .map(|id| projection.name(id))
                    .transpose()?,
                programs: task
                    .program_name_idx
                    .iter()
                    .map(|id| projection.name(*id))
                    .collect::<Result<_, _>>()?,
                // Artifact owners are remapped against the actual installed state by the constructor.
                fb_instances: Vec::new(),
            });
        }
        let background = programs.keys().any(|name| {
            !tasks.iter().any(|task| {
                task.programs
                    .iter()
                    .any(|program| program.eq_ignore_ascii_case(name))
            })
        });
        Ok(Self {
            storage_abi: prepared.storage_abi(),
            profile: DateTimeProfile::default(),
            registry: projection.registry,
            artifact_types: projection.wire_types.into_iter().map(Some).collect(),
            retained_bindings: IndexMap::new(),
            global_declarations: IndexMap::new(),
            io_display_names: prepared
                .io_bindings()
                .iter()
                .map(|binding| {
                    prepared
                        .persistent_reference_name(binding.ref_idx)
                        .map(Some)
                })
                .collect::<Result<_, _>>()?,
            stdlib: StandardLibrary::new(),
            access: AccessMap::default(),
            functions,
            function_blocks,
            classes,
            interfaces,
            programs,
            program_templates,
            tasks,
            task_thread_ids,
            background_thread_id: background.then_some(next_thread),
            statement_index: IndexMap::new(),
            source_text_index: IndexMap::new(),
            source_label_index: Default::default(),
        })
    }

    /// Whether original source text is available for source-level debugger resolution.
    #[must_use]
    pub fn has_source_text(&self) -> bool {
        !self.source_text_index.is_empty()
    }
}

fn insert_retained_binding(
    bindings: &mut IndexMap<RetainedLabel, RetainedBinding>,
    label: RetainedLabel,
    binding: RetainedBinding,
) -> Result<(), RuntimeError> {
    match bindings.entry(label) {
        indexmap::map::Entry::Vacant(entry) => {
            entry.insert(binding);
            Ok(())
        }
        indexmap::map::Entry::Occupied(entry) => Err(RuntimeError::InvalidConfig(
            format!("ambiguous retained management label '{}'", entry.key().0).into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::BytecodeVersion;
    use crate::harness::CompileSession;
    use trust_hir::{Type, TypeId};
    use trust_runtime_core::vm::PreparationLimits;

    #[test]
    fn saved_artifact_projects_real_roots_functions_and_tasks_without_source() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!("../../../tests/fixtures/portability/f401/gpio.stbc"),
            PreparationLimits::default(),
        )
        .unwrap();
        let metadata = RuntimeMetadata::from_prepared(&prepared, 0).unwrap();
        assert!(!metadata.has_source_text());
        assert!(metadata.statement_locations(0).is_none());
        assert_eq!(
            metadata
                .programs()
                .keys()
                .map(SmolStr::as_str)
                .collect::<Vec<_>>(),
            ["Plant"]
        );
        assert!(metadata.program_templates.contains_key("GPIO"));
        assert!(!metadata.has_background_programs());
        assert_eq!(metadata.background_thread_id(), None);
        assert_eq!(metadata.tasks()[0].name, "Scan");
        assert_eq!(
            metadata.tasks()[0].interval,
            crate::value::Duration::from_millis(10)
        );
        assert_eq!(metadata.functions()["PROBELEAF"].return_type, TypeId::INT);
        let variables = &metadata.programs()["Plant"].vars;
        assert_eq!(
            variables
                .iter()
                .find(|var| var.name == "button")
                .unwrap()
                .type_id,
            TypeId::BOOL
        );
        assert_eq!(
            variables
                .iter()
                .find(|var| var.name == "depth_result")
                .unwrap()
                .type_id,
            TypeId::INT
        );
        assert!(RuntimeMetadata::from_prepared(&prepared, usize::MAX).is_err());
    }

    #[test]
    fn artifact_type_projection_preserves_recursive_refs_arrays_and_string_lengths() {
        let source = "TYPE Node : STRUCT cells : ARRAY[1..2] OF STRING[8]; next : REF_TO Node; END_STRUCT; END_TYPE PROGRAM Main VAR node : Node; END_VAR END_PROGRAM";
        let module = CompileSession::from_source(source)
            .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
            .unwrap();
        let prepared =
            PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
                .unwrap();
        let original = prepared.type_table().clone();
        let metadata = RuntimeMetadata::from_prepared(&prepared, 0).unwrap();
        let node = metadata.registry().lookup("Node").unwrap();
        let Some(Type::Struct { fields, .. }) = metadata.registry().get(node) else {
            panic!("Node struct");
        };
        let next = fields.iter().find(|field| field.name == "next").unwrap();
        assert!(
            matches!(metadata.registry().get(next.type_id), Some(Type::Reference { target }) if *target == node)
        );
        let cells = fields.iter().find(|field| field.name == "cells").unwrap();
        let Some(Type::Array {
            element,
            dimensions,
        }) = metadata.registry().get(cells.type_id)
        else {
            panic!("array");
        };
        assert_eq!(dimensions, &[(1, 2)]);
        assert!(matches!(
            metadata.registry().get(*element),
            Some(Type::String { max_len: Some(8) })
        ));
        let mut scratch = metadata.registry().clone();
        scratch.register_string_with_length(37);
        assert!(metadata.registry().lookup("STRING[37]").is_none());
        assert_eq!(prepared.type_table(), &original);
    }

    #[test]
    fn saved_overlap_projection_keeps_shared_layout_and_relative_field_addresses() {
        let source = r#"
TYPE Overlay : STRUCT OVERLAP
    wide AT %B0 : DWORD;
    low AT %B0 : WORD;
END_STRUCT;
END_TYPE
PROGRAM Main VAR value : Overlay; END_VAR END_PROGRAM
"#;
        let module = CompileSession::from_source(source)
            .build_bytecode_module()
            .unwrap();
        let prepared =
            PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
                .unwrap();
        let metadata = RuntimeMetadata::from_prepared(&prepared, 0).unwrap();
        let id = metadata.registry().lookup("Overlay").unwrap();
        assert!(metadata.registry().is_overlapping_struct_type(id));
        let Some(Type::Struct { fields, .. }) = metadata.registry().get(id) else {
            panic!("saved overlap type")
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].address.as_deref(), Some("%X0.0"));
        assert_eq!(fields[1].address.as_deref(), Some("%X0.0"));
        assert_eq!(fields[0].type_id, TypeId::DWORD);
        assert_eq!(fields[1].type_id, TypeId::WORD);
    }
}
