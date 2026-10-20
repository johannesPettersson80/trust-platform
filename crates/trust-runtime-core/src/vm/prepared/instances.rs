//! What a program's function-block and class instances take (specification 34 MEM-09, MEM-12;
//! checklist C1-11c, C1-21): the instances a call creates on the activation stack, and the
//! bookkeeping of every instance the program holds at once, both sized at preparation so that
//! construction reserves them once, no scan grows them and the memory figure counts them.

use super::*;
use crate::bytecode::{
    InitializationOnce, StorageOwner, TypeData, TypeKind, BYTECODE_MAX_TYPE_NESTING,
};

/// What the instances that a value of one type holds take: one instance for each level of each
/// inheritance chain, with that level's own variables.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct InstanceRoom {
    /// The bytes of their own variables on the activation stack.
    pub(crate) bytes: u32,
    /// Their number.
    pub(crate) count: u32,
    /// Their reference leaves.
    pub(crate) leaves: u32,
    /// The initialized-declaration marks they hold at most: one for each own variable of each.
    pub(crate) marks: u32,
    /// The once marks they hold at most: one for each instance action of each that runs once per
    /// instance.
    pub(crate) once: u32,
}

impl InstanceRoom {
    fn plus(self, other: Self) -> Option<Self> {
        Some(Self {
            bytes: self.bytes.checked_add(other.bytes)?,
            count: self.count.checked_add(other.count)?,
            leaves: self.leaves.checked_add(other.leaves)?,
            marks: self.marks.checked_add(other.marks)?,
            once: self.once.checked_add(other.once)?,
        })
    }

    fn times(self, factor: u32) -> Option<Self> {
        Some(Self {
            bytes: self.bytes.checked_mul(factor)?,
            count: self.count.checked_mul(factor)?,
            leaves: self.leaves.checked_mul(factor)?,
            marks: self.marks.checked_mul(factor)?,
            once: self.once.checked_mul(factor)?,
        })
    }

    /// The larger of each part of `self` and `other`.
    fn widest(self, other: Self) -> Self {
        Self {
            bytes: self.bytes.max(other.bytes),
            count: self.count.max(other.count),
            leaves: self.leaves.max(other.leaves),
            marks: self.marks.max(other.marks),
            once: self.once.max(other.once),
        }
    }
}

/// The bookkeeping construction reserves for the instances a program holds at once: its persistent
/// instances and those the deepest chain of calls creates on the activation stack.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct InstanceBookkeeping {
    /// The instances: each has a template, a lifetime, an initialized flag and a visited slot.
    pub(crate) instances: usize,
    /// The initialized-declaration marks: the globals' and each instance's own variables'.
    pub(crate) marks: usize,
    /// The once marks: one for each action that runs once per program, and each instance's.
    pub(crate) once: usize,
    /// The construction roots that construction claims at most: each root with a template, whose
    /// instance a program, an inheritance parent or a member claims once.
    pub(crate) claimed_roots: usize,
}

impl PreparedModule {
    /// What the instances a frame of `pou` creates for its function-block and class variables
    /// take; they lie on the activation stack above the frame (C1-11c, C1-19c4).
    pub(crate) fn call_instance_room(&self, pou: u32) -> Option<InstanceRoom> {
        let mut room = InstanceRoom::default();
        for declaration in &self.layout.entries {
            if declaration.owner != StorageOwner::Frame
                || declaration.owner_pou_id != Some(pou)
                || !declaration.owns_storage()
            {
                continue;
            }
            let Some(type_id) = declaration.type_id else {
                continue;
            };
            room = room.plus(self.instance_tree(type_id, 0)?)?;
        }
        Some(room)
    }

    /// The marks one instance of `pou`, one level of its chain, holds at most: one initialized
    /// mark for each of its own variables, and one once mark for each of its instance actions that
    /// runs once per instance.
    fn level_marks(&self, pou: u32) -> Option<(u32, u32)> {
        let marks =
            u32::try_from(self.declarations(StorageOwner::Instance, Some(pou)).len()).ok()?;
        let once = self
            .instance_actions(pou)
            .iter()
            .filter(|&&id| {
                self.initializers
                    .entries
                    .get(id as usize)
                    .is_some_and(|entry| entry.once == InitializationOnce::Instance)
            })
            .count();
        Some((marks, u32::try_from(once).ok()?))
    }

    /// What the instances on the activation stack that a value of `type_id` holds take: a function
    /// block or a class its instance, one for each level of its inheritance chain with that level's
    /// own variables, and the instances those variables hold; an array each element's; a structure
    /// each member's.
    fn instance_tree(&self, type_id: u32, depth: usize) -> Option<InstanceRoom> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return None;
        }
        let mut current = type_id;
        let entry = loop {
            let entry = self.vm.types.entries.get(current as usize)?;
            match &entry.data {
                TypeData::Alias { target_type_id } => current = *target_type_id,
                _ => break entry,
            }
        };
        match &entry.data {
            TypeData::Pou { pou_id }
                if matches!(entry.kind, TypeKind::FunctionBlock | TypeKind::Class) =>
            {
                let mut total = InstanceRoom::default();
                let mut level = Some(*pou_id);
                for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
                    let Some(pou) = level else {
                        break;
                    };
                    let (marks, once) = self.level_marks(pou)?;
                    total = total.plus(InstanceRoom {
                        bytes: self
                            .instance_template(pou)
                            .map_or(0, |template| template.range.bytes),
                        count: 1,
                        leaves: u32::try_from(self.instance_reference_leaves(pou).len()).ok()?,
                        marks,
                        once,
                    })?;
                    for &id in self.declarations(StorageOwner::Instance, Some(pou)) {
                        let member = self.layout.entries.get(id as usize)?;
                        let Some(member_type) = member.type_id.filter(|_| member.owns_storage())
                        else {
                            continue;
                        };
                        total = total.plus(self.instance_tree(member_type, depth + 1)?)?;
                    }
                    level = self.vm.parent_pou_ids.get(&pou).copied();
                }
                Some(total)
            }
            TypeData::Array { elem_type_id, dims } => {
                let room = self.instance_tree(*elem_type_id, depth + 1)?;
                // An array of values that hold no instance holds none, whatever its bounds (an
                // `ARRAY[*]` parameter has none of its own).
                if room == InstanceRoom::default() {
                    return Some(room);
                }
                let elements = dims.iter().try_fold(1u32, |elements, (lower, upper)| {
                    let length = u32::try_from(upper.checked_sub(*lower)?.checked_add(1)?).ok()?;
                    elements.checked_mul(length)
                })?;
                room.times(elements)
            }
            TypeData::Struct { fields } | TypeData::Union { fields } => fields
                .iter()
                .try_fold(InstanceRoom::default(), |total, field| {
                    total.plus(self.instance_tree(field.type_id, depth + 1)?)
                }),
            _ => Some(InstanceRoom::default()),
        }
    }

    /// The most instances one initializer's result holds, with their inheritance levels and the
    /// instances their variables hold: the instances that promoting the result to its owner walks
    /// (C1-21).
    pub(crate) fn largest_result_instances(&self) -> Result<usize, RuntimeError> {
        let mut largest = 0u32;
        for entry in &self.initializers.entries {
            if let Some(type_id) = self.vm.ref_type(entry.result_ref_idx) {
                let room = self
                    .instance_tree(type_id, 0)
                    .ok_or(RuntimeError::Overflow)?;
                largest = largest.max(room.count);
            }
        }
        usize::try_from(largest).map_err(|_| RuntimeError::Overflow)
    }

    /// The bookkeeping of every instance the program holds at once (C1-21): each construction root
    /// with a template holds one persistent instance, with its marks, and each reserved call level
    /// holds at most the instances that the frame of any POU creates, with theirs. The globals have
    /// their marks, and each action that runs once per program its once mark.
    pub(crate) fn instance_bookkeeping(&self) -> Result<InstanceBookkeeping, RuntimeError> {
        let overflow = || RuntimeError::Overflow;
        let mut persistent = InstanceRoom::default();
        let mut claimed_roots = 0usize;
        for root in &self.roots.entries {
            let Some(pou) = root.template_pou_id else {
                continue;
            };
            let (marks, once) = self.level_marks(pou).ok_or_else(overflow)?;
            persistent = persistent
                .plus(InstanceRoom {
                    count: 1,
                    marks,
                    once,
                    ..InstanceRoom::default()
                })
                .ok_or_else(overflow)?;
            claimed_roots += 1;
        }
        let mut largest = InstanceRoom::default();
        for pou in self.frame_pous() {
            largest = largest.widest(self.call_instance_room(pou).ok_or_else(overflow)?);
        }
        let levels = u32::try_from(self.reserved_call_levels()).map_err(|_| overflow())?;
        let held = largest
            .times(levels)
            .and_then(|calls| calls.plus(persistent))
            .ok_or_else(overflow)?;
        let globals = self.declarations(StorageOwner::Global, None).len();
        let program_once = self
            .initializers
            .entries
            .iter()
            .filter(|entry| entry.once == InitializationOnce::Module)
            .count();
        Ok(InstanceBookkeeping {
            instances: held.count as usize,
            marks: (held.marks as usize)
                .checked_add(globals)
                .ok_or_else(overflow)?,
            once: (held.once as usize)
                .checked_add(program_once)
                .ok_or_else(overflow)?,
            claimed_roots,
        })
    }
}
