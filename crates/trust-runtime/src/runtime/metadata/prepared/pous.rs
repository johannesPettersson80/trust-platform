//! Read-only POU declarations for hosted debugger and protocol metadata.
use super::types::Projection;
use crate::{
    error::RuntimeError,
    program_model::{
        ClassDef, FunctionBlockBase, FunctionBlockDef, FunctionDef, MethodDef, Param, VarDef,
    },
    task::ProgramDef,
};
use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::{symbols::ParamDirection, TypeId};
use trust_runtime_core::bytecode::{PouKind, StorageDeclaration, StorageOwner, StorageRole};

impl Projection<'_> {
    fn return_type(&self, pou: u32) -> Result<Option<TypeId>, RuntimeError> {
        self.prepared
            .frame_declarations(pou)
            .find(|entry| entry.role == StorageRole::Return)
            .map(|entry| self.ty(entry.type_id.ok_or(RuntimeError::InvalidExecutionState)?))
            .transpose()
    }

    fn params(&self, pou: u32) -> Result<Vec<Param>, RuntimeError> {
        self.prepared
            .pou_parameters(pou)
            .unwrap_or(&[])
            .iter()
            .map(|param| {
                Ok(Param {
                    name: param.name.clone(),
                    type_id: self.ty(param.type_id)?,
                    direction: match param.direction {
                        0 => ParamDirection::In,
                        1 => ParamDirection::Out,
                        2 => ParamDirection::InOut,
                        _ => return Err(RuntimeError::InvalidExecutionState),
                    },
                    address: None,
                    default: param
                        .default_const_idx
                        .map(|id| {
                            self.prepared
                                .constant(id)
                                .cloned()
                                .map(crate::program_model::Expr::Literal)
                                .ok_or(RuntimeError::InvalidExecutionState)
                        })
                        .transpose()?,
                })
            })
            .collect()
    }

    fn variables(
        &self,
        pou: u32,
        owner: StorageOwner,
        statics: bool,
    ) -> Result<Vec<VarDef>, RuntimeError> {
        self.declarations
            .get(&(pou, owner as u8, statics))
            .into_iter()
            .flatten()
            .map(|entry| self.variable(entry))
            .collect()
    }

    fn variable(&self, entry: &StorageDeclaration) -> Result<VarDef, RuntimeError> {
        Ok(VarDef {
            name: self.name(entry.source_name_idx.unwrap_or(entry.name_idx))?,
            type_id: self.ty(entry.type_id.ok_or(RuntimeError::InvalidExecutionState)?)?,
            initializer: None,
            retain: match entry.retain {
                0 => crate::RetainPolicy::Unspecified,
                1 => crate::RetainPolicy::Retain,
                2 => crate::RetainPolicy::NonRetain,
                3 => crate::RetainPolicy::Persistent,
                _ => return Err(RuntimeError::InvalidExecutionState),
            },
            static_storage: entry.role == StorageRole::Static,
            external: entry.role == StorageRole::External,
            in_out: entry.flags & 8 != 0,
            constant: entry.is_constant(),
            address: None,
        })
    }

    fn interfaces(&self, pou: u32) -> Result<Vec<SmolStr>, RuntimeError> {
        self.prepared
            .pou_interfaces(pou)
            .iter()
            .map(|id| {
                self.registry
                    .type_name(self.ty(*id)?)
                    .ok_or(RuntimeError::InvalidExecutionState)
            })
            .collect()
    }

    fn methods(&self, owner: u32) -> Result<Vec<MethodDef>, RuntimeError> {
        self.methods
            .get(&owner)
            .into_iter()
            .flatten()
            .map(|id| {
                let id = *id;
                Ok(MethodDef {
                    name: self
                        .prepared
                        .pou_name(id)
                        .ok_or(RuntimeError::InvalidExecutionState)?
                        .into(),
                    return_type: self.return_type(id)?,
                    params: self.params(id)?,
                    locals: self.variables(id, StorageOwner::Frame, false)?,
                    static_locals: self.variables(id, StorageOwner::Instance, true)?,
                    using: Vec::new(),
                    body: Vec::new(),
                })
            })
            .collect()
    }
}

pub(super) struct PouDeclarations {
    pub(super) functions: IndexMap<SmolStr, FunctionDef>,
    pub(super) function_blocks: IndexMap<SmolStr, FunctionBlockDef>,
    pub(super) classes: IndexMap<SmolStr, ClassDef>,
    pub(super) program_templates: IndexMap<SmolStr, ProgramDef>,
}

impl Projection<'_> {
    pub(super) fn pous(&self) -> Result<PouDeclarations, RuntimeError> {
        let prepared = self.prepared;
        let mut functions = IndexMap::new();
        let mut function_blocks = IndexMap::new();
        let mut classes = IndexMap::new();
        let mut program_templates = IndexMap::new();
        let mut pous = prepared.pous().collect::<Vec<_>>();
        pous.sort_unstable_by_key(|(id, _)| *id);
        for (id, pou) in &pous {
            let id = *id;
            let name = pou.name.clone();
            let key = SmolStr::new(name.to_ascii_uppercase());
            match prepared
                .pou_kind(id)
                .ok_or(RuntimeError::InvalidExecutionState)?
            {
                PouKind::Program => {
                    program_templates.insert(
                        key,
                        ProgramDef {
                            name,
                            vars: self.variables(id, StorageOwner::Instance, false)?,
                            temps: self.variables(id, StorageOwner::Frame, false)?,
                            using: Vec::new(),
                            body: Vec::new(),
                            methods: self.methods(id)?,
                        },
                    );
                }
                PouKind::Function => {
                    functions.insert(
                        key,
                        FunctionDef {
                            name,
                            return_type: self
                                .return_type(id)?
                                .ok_or(RuntimeError::InvalidExecutionState)?,
                            params: self.params(id)?,
                            locals: self.variables(id, StorageOwner::Frame, false)?,
                            static_locals: self.variables(id, StorageOwner::Global, true)?,
                            using: Vec::new(),
                            body: Vec::new(),
                        },
                    );
                }
                PouKind::FunctionBlock => {
                    let base = prepared
                        .parent_pou(id)
                        .map(|parent| {
                            let name = prepared
                                .pou_name(parent)
                                .ok_or(RuntimeError::InvalidExecutionState)?
                                .into();
                            match prepared.pou_kind(parent) {
                                Some(PouKind::FunctionBlock) => {
                                    Ok(FunctionBlockBase::FunctionBlock(name))
                                }
                                Some(PouKind::Class) => Ok(FunctionBlockBase::Class(name)),
                                _ => Err(RuntimeError::InvalidExecutionState),
                            }
                        })
                        .transpose()?;
                    function_blocks.insert(
                        key,
                        FunctionBlockDef {
                            name,
                            base,
                            interfaces: self.interfaces(id)?,
                            params: self.params(id)?,
                            vars: self.variables(id, StorageOwner::Instance, false)?,
                            temps: self.variables(id, StorageOwner::Frame, false)?,
                            using: Vec::new(),
                            methods: self.methods(id)?,
                            body: Vec::new(),
                        },
                    );
                }
                PouKind::Class => {
                    let base = prepared
                        .parent_pou(id)
                        .map(|parent| {
                            prepared
                                .pou_name(parent)
                                .map(SmolStr::new)
                                .ok_or(RuntimeError::InvalidExecutionState)
                        })
                        .transpose()?;
                    classes.insert(
                        key,
                        ClassDef {
                            name,
                            base,
                            interfaces: self.interfaces(id)?,
                            vars: self.variables(id, StorageOwner::Instance, false)?,
                            using: Vec::new(),
                            methods: self.methods(id)?,
                        },
                    );
                }
                PouKind::Method => {}
            }
        }
        Ok(PouDeclarations {
            functions,
            function_blocks,
            classes,
            program_templates,
        })
    }
}
