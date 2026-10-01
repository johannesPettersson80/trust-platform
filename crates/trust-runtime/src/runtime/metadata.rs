//! Runtime metadata snapshots.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::debug::SourceLocation;
use crate::memory::{AccessMap, FrameId, LocalFrame, VariableStorage};
use crate::program_model::{ClassDef, FunctionBlockDef, FunctionDef, InterfaceDef, Param};
use crate::stdlib::StandardLibrary;
use crate::task::{ProgramDef, TaskConfig};
use crate::value::DateTimeProfile;
use trust_hir::types::TypeRegistry;

#[derive(Debug, Clone)]
pub(crate) struct ControlBindingLeaf {
    pub(crate) constant: bool,
    pub(crate) external: bool,
    pub(crate) in_out: bool,
    pub(crate) address: Option<crate::io::IoAddress>,
}

/// Snapshot of runtime metadata needed by external tooling.
#[derive(Debug, Clone)]
pub struct RuntimeMetadata {
    pub(super) profile: DateTimeProfile,
    pub(super) registry: TypeRegistry,
    pub(super) stdlib: StandardLibrary,
    pub(super) access: AccessMap,
    pub(super) functions: IndexMap<SmolStr, FunctionDef>,
    pub(super) function_blocks: IndexMap<SmolStr, FunctionBlockDef>,
    pub(super) classes: IndexMap<SmolStr, ClassDef>,
    pub(super) interfaces: IndexMap<SmolStr, InterfaceDef>,
    pub(super) programs: IndexMap<SmolStr, ProgramDef>,
    pub(super) tasks: Vec<TaskConfig>,
    pub(super) task_thread_ids: IndexMap<SmolStr, u32>,
    pub(super) background_thread_id: Option<u32>,
    pub(super) statement_index: IndexMap<u32, Vec<SourceLocation>>,
}

impl RuntimeMetadata {
    pub(crate) fn control_program_revision(&self) -> String {
        use sha2::{Digest, Sha256};
        format!(
            "sha256:{:x}",
            Sha256::digest(
                format!(
                    "{:?}",
                    (
                        &self.programs,
                        &self.function_blocks,
                        &self.functions,
                        &self.classes,
                        &self.interfaces,
                        &self.registry,
                        &self.tasks
                    )
                )
                .as_bytes()
            )
        )
    }

    pub(crate) fn control_parameter_writable(&self, symbol: &str) -> bool {
        self.control_binding_leaf(symbol).is_some_and(|leaf| {
            !leaf.constant && !leaf.external && !leaf.in_out && leaf.address.is_none()
        })
    }

    pub(crate) fn control_binding_leaf(&self, symbol: &str) -> Option<ControlBindingLeaf> {
        let mut path = symbol.split('.');
        let root = path.next()?;
        let program = self
            .programs
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(root))
            .map(|(_, program)| program)?;
        let mut vars = program.vars.as_slice();
        let mut params: &[Param] = &[];
        let mut type_id = None;
        let mut constant = false;
        let mut external = false;
        let mut in_out = false;
        let mut address = None;
        let mut rest = path.peekable();
        while let Some(name) = rest.next() {
            let declaration =
                if let Some(var) = vars.iter().find(|var| var.name.eq_ignore_ascii_case(name)) {
                    (
                        var.type_id,
                        var.constant,
                        var.external,
                        var.in_out,
                        var.address.clone(),
                    )
                } else if let Some(param) = params
                    .iter()
                    .find(|param| param.name.eq_ignore_ascii_case(name))
                {
                    (
                        param.type_id,
                        false,
                        false,
                        param.direction == trust_hir::symbols::ParamDirection::InOut,
                        param.address.clone(),
                    )
                } else {
                    let current = type_id
                        .and_then(|id| self.registry.get(id))
                        .and_then(|ty| match ty {
                            trust_hir::types::Type::Alias { target, .. } => {
                                self.registry.get(*target)
                            }
                            _ => Some(ty),
                        })?;
                    let trust_hir::types::Type::Struct { fields, .. } = current else {
                        return None;
                    };
                    let field = fields
                        .iter()
                        .find(|field| field.name.eq_ignore_ascii_case(name))?;
                    (field.type_id, false, false, false, None)
                };
            type_id = Some(declaration.0);
            constant |= declaration.1;
            external |= declaration.2;
            in_out |= declaration.3;
            if address.is_none() {
                address = declaration.4;
            }
            if rest.peek().is_none() {
                return Some(ControlBindingLeaf {
                    constant,
                    external,
                    in_out,
                    address,
                });
            }
            let next = match self.registry.get(type_id?) {
                Some(trust_hir::types::Type::Alias { target, .. }) => *target,
                Some(_) => type_id?,
                None => return None,
            };
            type_id = Some(next);
            let type_name = self.registry.type_name(next)?;
            let fb = self
                .function_blocks
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&type_name))
                .map(|(_, fb)| fb);
            if let Some(fb) = fb {
                vars = &fb.vars;
                params = &fb.params;
            } else {
                vars = &[];
                params = &[];
            }
        }
        None
    }

    /// Access the type registry snapshot.
    #[must_use]
    pub fn registry(&self) -> &TypeRegistry {
        &self.registry
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
        self.task_thread_ids.get(name).copied()
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

    use crate::program_model::{FunctionBlockDef, VarDef};
    use crate::runtime::Runtime;
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

    #[test]
    fn nested_binding_rejects_constant_function_block_owner() {
        let mut runtime = Runtime::new();
        let parameters = runtime.registry_mut().register_struct(
            "PID_PARAMETERS",
            vec![trust_hir::types::StructField {
                name: "ProportionalGain".into(),
                type_id: TypeId::LREAL,
                address: None,
                default_initializer: None,
            }],
        );
        let controller_type = runtime.registry_mut().register(
            "Controller",
            trust_hir::types::Type::FunctionBlock {
                name: "Controller".into(),
            },
        );
        runtime.register_function_block(FunctionBlockDef {
            name: "Controller".into(),
            base: None,
            interfaces: Vec::new(),
            params: Vec::new(),
            vars: vec![VarDef {
                name: "ParametersValue".into(),
                type_id: parameters,
                initializer: None,
                retain: crate::RetainPolicy::Unspecified,
                static_storage: false,
                external: false,
                in_out: false,
                constant: false,
                address: None,
            }],
            temps: Vec::new(),
            using: Vec::new(),
            methods: Vec::new(),
            body: Vec::new(),
        });
        runtime
            .register_program(ProgramDef {
                name: "Main".into(),
                vars: vec![VarDef {
                    name: "Controller".into(),
                    type_id: controller_type,
                    initializer: None,
                    retain: crate::RetainPolicy::Unspecified,
                    static_storage: false,
                    external: false,
                    in_out: false,
                    constant: true,
                    address: None,
                }],
                temps: Vec::new(),
                using: Vec::new(),
                body: Vec::new(),
            })
            .unwrap();

        assert!(!runtime
            .metadata_snapshot()
            .control_parameter_writable("Main.Controller.ParametersValue.ProportionalGain"));
    }
}
