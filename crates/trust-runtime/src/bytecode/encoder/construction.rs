//! Structural storage bindings and executable initialization plans for source-free authoring.
//!
//! These records contain declarations and references, never snapshots of live Values.

use indexmap::IndexMap;
use smol_str::SmolStr;
use std::collections::HashMap;
use trust_hir::TypeId;

use super::{BytecodeEncoder, BytecodeError, CodegenContext};
use crate::bytecode::{ConstructionRoots, InitializerIndex, StorageLayout};
use crate::memory::InstanceId;
use crate::program_model::Expr;
use crate::value::ValueRef;

mod access;
mod actions;
mod bodies;
mod configuration;
mod declarations;
mod fb_overrides;
mod frames;
mod globals;
mod initializers;
mod io;
mod metadata;
mod recipes;
mod roots;
mod templates;
use super::util::normalize_name;

#[derive(Default)]
pub(super) struct ConstructionModel {
    pub(super) bodies: InitializerBodies,
    pub(super) recipes: RecipeModel,
    pub(super) templates: TemplateModel,
    pub(super) layout: StorageLayout,
    pub(super) io: crate::io::IoInterface,
    pub(super) direct_image_extents: [u32; 3],
    pub(super) tasks: Vec<crate::task::TaskConfig>,
    pub(super) access: crate::bytecode::AccessBindings,
    pub(super) aliases: HashMap<SmolStr, access::ResolvedBinding>,
    pub(super) wildcards: Vec<crate::harness::WildcardRequirement>,
    pub(super) roots: ConstructionRoots,
    pub(super) initializers: InitializerIndex,
    pub(super) bindings: StructuralBindings,
    pub(super) plans: Vec<DeclarationPlan>,
    pub(super) contexts: HashMap<u32, CodegenContext>,
    pub(super) declaration_types: Vec<Option<TypeId>>,
}

#[derive(Clone)]
pub(super) struct PendingInitializerBody {
    pub(super) id: u32,
    pub(super) type_id: TypeId,
    pub(super) expression: Option<Expr>,
    pub(super) coerce_opcode: u8,
}

#[derive(Clone)]
pub(super) struct DeclarationPlan {
    pub(super) declaration: u32,
    pub(super) type_id: TypeId,
    pub(super) expression: Option<Expr>,
}

/// Case-normalized logical storage identities allocated from declaration order.
#[derive(Default)]
pub(super) struct StructuralBindings {
    configuration_name: Option<SmolStr>,
    resource_name: Option<SmolStr>,
    globals: IndexMap<SmolStr, ValueRef>,
    instances: Vec<InstanceBindings>,
    pub(super) types: HashMap<ValueRef, TypeId>,
    pub(super) values_to_instances: HashMap<ValueRef, InstanceId>,
}

struct InstanceBindings {
    parent: Option<InstanceId>,
    fields: IndexMap<SmolStr, ValueRef>,
}

impl StructuralBindings {
    pub(super) fn global(&self, name: &SmolStr) -> Option<&ValueRef> {
        self.globals.get(&normalize_name(name))
    }

    pub(super) fn instance(&self, mut owner: InstanceId, name: &SmolStr) -> Option<&ValueRef> {
        let key = normalize_name(name);
        // Parents are always earlier records, so malformed/cyclic ancestry cannot
        // enter this graph through the allocation API.
        loop {
            let instance = self.instances.get(owner.0 as usize)?;
            if let Some(reference) = instance.fields.get(&key) {
                return Some(reference);
            }
            owner = instance.parent?;
        }
    }

    pub(super) fn allocate_global(
        &mut self,
        name: &SmolStr,
        type_id: Option<TypeId>,
    ) -> Result<ValueRef, BytecodeError> {
        let key = normalize_name(name);
        if self.globals.contains_key(&key) {
            return Err(BytecodeError::InvalidSection(
                format!("duplicate global binding '{name}'").into(),
            ));
        }
        let reference = ValueRef {
            location: crate::memory::MemoryLocation::Global,
            offset: self.globals.len(),
            path: Vec::new(),
        };
        self.globals.insert(key, reference.clone());
        if let Some(type_id) = type_id {
            self.types.insert(reference.clone(), type_id);
        }
        Ok(reference)
    }

    pub(super) fn allocate_instance(
        &mut self,
        parent: Option<InstanceId>,
    ) -> Result<InstanceId, BytecodeError> {
        if parent.is_some_and(|id| id.0 as usize >= self.instances.len()) {
            return Err(BytecodeError::InvalidSection(
                "instance parent must already exist".into(),
            ));
        }
        let id = u32::try_from(self.instances.len())
            .map_err(|_| BytecodeError::InvalidSection("instance identity overflow".into()))?;
        self.instances.push(InstanceBindings {
            parent,
            fields: IndexMap::new(),
        });
        Ok(InstanceId(id))
    }

    pub(super) fn allocate_field(
        &mut self,
        owner: InstanceId,
        name: &SmolStr,
        type_id: TypeId,
    ) -> Result<ValueRef, BytecodeError> {
        let instance = self
            .instances
            .get_mut(owner.0 as usize)
            .ok_or_else(|| BytecodeError::InvalidSection("unknown instance owner".into()))?;
        let key = normalize_name(name);
        if instance.fields.contains_key(&key) {
            return Err(BytecodeError::InvalidSection(
                format!("duplicate instance field '{name}'").into(),
            ));
        }
        let reference = ValueRef {
            location: crate::memory::MemoryLocation::Instance(owner),
            offset: instance.fields.len(),
            path: Vec::new(),
        };
        instance.fields.insert(key, reference.clone());
        self.types.insert(reference.clone(), type_id);
        Ok(reference)
    }
}

fn invalid(message: &str) -> BytecodeError {
    BytecodeError::InvalidSection(message.into())
}

#[derive(Default)]
pub(super) struct TemplateModel {
    pub(super) reachable_stdlib: std::collections::HashSet<SmolStr>,
    pub(super) template_fields: HashMap<u32, Vec<u32>>,
    pub(super) template_parents: HashMap<u32, u32>,
}

#[derive(Default)]
pub(super) struct RecipeModel {
    pub(super) default_recipes: HashMap<TypeId, u32>,
    pub(super) recipe_types: Vec<(u32, TypeId)>,
    pub(super) recipe_contexts: HashMap<[u32; 11], u32>,
    pub(super) typed_recipes: HashMap<(InitializerId, TypeId, Option<MemberId>), InitializerId>,
}

#[derive(Default)]
pub(super) struct InitializerBodies {
    pub(super) initializer_contexts: HashMap<u32, CodegenContext>,
    pub(super) result_frames: HashMap<crate::memory::FrameId, u32>,
    pub(super) pending_bodies: Vec<PendingInitializerBody>,
}

/// Index into INITIALIZERS; distinct from TYPE_TABLE and storage declaration ids.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct InitializerId(pub(super) u32);

/// Zero-based member of a STRUCT or UNION, never a storage slot.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct MemberId(pub(super) u32);
