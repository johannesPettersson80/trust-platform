//! What the evaluation of initializers holds at once (specification 34 MEM-09; checklist C1-21).
//!
//! An initializer evaluates, inside its own evaluation, each default recipe that the construction
//! of its result meets and each initializer its body names with `DEFAULT_VALUE`, and so on through
//! them. Its body builds raw arrays and structures in containers that the engine keeps for the
//! next evaluation. The engine reserves its initializer scratch for the deepest nesting and its
//! containers for the most and the largest raw values, so that neither construction nor a
//! call-time initializer grows them, and the memory figure counts them.

use super::*;
use crate::bytecode::opcodes::{ARRAY_NEW, DEFAULT_VALUE, STRUCT_NEW};
use crate::bytecode::{TypeData, BYTECODE_MAX_TYPE_NESTING};

/// The deepest chain of nested evaluations that the walk follows: deeper chains are refused by the
/// engine's depth check before they could need more.
const NESTING_LIMIT: usize = 2 * BYTECODE_MAX_TYPE_NESTING + 2;

/// The raw containers that the initializer bodies of a module build: how many of each kind one
/// evaluation holds at once, and the room of each (C1-21).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RawShellBounds {
    /// Raw arrays held at once.
    pub(crate) arrays: usize,
    /// The most elements of a raw array.
    pub(crate) elements: usize,
    /// Raw structures held at once.
    pub(crate) structures: usize,
    /// The most fields of a raw structure: those of the largest structure or union type.
    pub(crate) fields: usize,
}

/// The `ARRAY_NEW`, `STRUCT_NEW` and `DEFAULT_VALUE` instructions of one body.
#[derive(Default)]
struct BodyOps {
    arrays: usize,
    elements: usize,
    structures: usize,
    defaults: Vec<u32>,
}

impl PreparedModule {
    /// The instructions of initializer `id`'s body that build raw values or evaluate another
    /// initializer.
    fn body_ops(&self, id: u32) -> BodyOps {
        let mut ops = BodyOps::default();
        let Some(entry) = self.initializer(id as usize) else {
            return ops;
        };
        let start = entry.code_offset as usize;
        let Some(code) = start
            .checked_add(entry.code_length as usize)
            .and_then(|end| self.vm.code.get(start..end))
        else {
            return ops;
        };
        let mut pc = 0usize;
        while let Some(&opcode) = code.get(pc) {
            let Some(width) = crate::vm::opcode_operand_len(opcode) else {
                break;
            };
            let operand = code
                .get(pc + 1..pc + 1 + width)
                .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                .map(u32::from_le_bytes);
            match (opcode, operand) {
                (ARRAY_NEW, Some(count)) => {
                    ops.arrays += 1;
                    ops.elements = ops.elements.max(count as usize);
                }
                (STRUCT_NEW, _) => ops.structures += 1,
                (DEFAULT_VALUE, Some(nested)) => ops.defaults.push(nested),
                _ => {}
            }
            pc += 1 + width;
        }
        ops
    }

    /// The default recipe of `type_id`, or of `member` of it, in any context.
    fn recipes_of(&self, type_id: u32, member: Option<u32>) -> impl Iterator<Item = u32> + '_ {
        self.recipes
            .iter()
            .filter(move |&&(_, recipe_type, recipe_member, _)| {
                recipe_type == type_id && recipe_member == member
            })
            .map(|&(_, _, _, id)| id)
    }

    /// The evaluations that initializer `id` holds at once: itself and the deepest chain nested in
    /// it, through the recipes of its result's construction and the initializers its body names.
    fn initializer_depth(&self, id: u32, guard: usize, memo: &mut [Option<usize>]) -> usize {
        if guard > NESTING_LIMIT {
            return 0;
        }
        if let Some(Some(depth)) = memo.get(id as usize) {
            return *depth;
        }
        let Some(entry) = self.initializer(id as usize) else {
            return 0;
        };
        let mut nested = self
            .vm
            .ref_type(entry.result_ref_idx)
            .map_or(0, |type_id| self.type_depth(type_id, guard + 1, memo));
        for default in self.body_ops(id).defaults {
            nested = nested.max(self.initializer_depth(default, guard + 1, memo));
        }
        let depth = nested + 1;
        if let Some(slot) = memo.get_mut(id as usize) {
            *slot = Some(depth);
        }
        depth
    }

    /// The deepest chain of evaluations that constructing a value of `type_id` holds: its own default
    /// recipe's, else the deepest of its members' recipes or members' types, or its elements'.
    fn type_depth(&self, type_id: u32, guard: usize, memo: &mut [Option<usize>]) -> usize {
        if guard > NESTING_LIMIT {
            return 0;
        }
        let mut current = type_id;
        let entry = loop {
            let own = self
                .recipes_of(current, None)
                .collect::<Vec<_>>()
                .into_iter()
                .map(|recipe| self.initializer_depth(recipe, guard + 1, memo))
                .max();
            if let Some(own) = own {
                return own;
            }
            let Some(entry) = self.vm.types.entries.get(current as usize) else {
                return 0;
            };
            match &entry.data {
                TypeData::Alias { target_type_id } => current = *target_type_id,
                _ => break entry,
            }
        };
        match &entry.data {
            TypeData::Struct { fields } | TypeData::Union { fields } => {
                let mut deepest = 0;
                for (index, field) in fields.iter().enumerate() {
                    let member = u32::try_from(index).ok();
                    let recipes = self
                        .recipes_of(current, member)
                        .chain(self.recipes_of(type_id, member))
                        .collect::<Vec<_>>();
                    let depth = if recipes.is_empty() {
                        self.type_depth(field.type_id, guard + 1, memo)
                    } else {
                        recipes
                            .into_iter()
                            .map(|recipe| self.initializer_depth(recipe, guard + 1, memo))
                            .max()
                            .unwrap_or(0)
                    };
                    deepest = deepest.max(depth);
                }
                deepest
            }
            TypeData::Array { elem_type_id, .. } => self.type_depth(*elem_type_id, guard + 1, memo),
            _ => 0,
        }
    }

    /// The most initializer evaluations that one call level holds at once: an initializer and the
    /// deepest chain nested in it (at least one).
    pub(crate) fn initializer_nesting(&self) -> usize {
        let mut memo = alloc::vec![None; self.initializer_count()];
        (0..self.initializer_count())
            .filter_map(|id| u32::try_from(id).ok())
            .map(|id| self.initializer_depth(id, 0, &mut memo))
            .max()
            .unwrap_or(0)
            .max(1)
    }

    /// The raw containers one evaluation holds at once and the room of each: each nested
    /// evaluation holds at most as many of each kind as the body that builds the most, and the
    /// engine keeps at most `kept` of each.
    pub(crate) fn raw_shell_bounds(&self, kept: usize) -> RawShellBounds {
        let mut bounds = RawShellBounds::default();
        let mut arrays = 0usize;
        let mut structures = 0usize;
        for id in 0..self.initializer_count() {
            let Ok(id) = u32::try_from(id) else {
                break;
            };
            let ops = self.body_ops(id);
            arrays = arrays.max(ops.arrays);
            structures = structures.max(ops.structures);
            bounds.elements = bounds.elements.max(ops.elements);
        }
        let nesting = self.initializer_nesting();
        bounds.arrays = arrays.saturating_mul(nesting).min(kept);
        bounds.structures = structures.saturating_mul(nesting).min(kept);
        if bounds.structures != 0 {
            bounds.fields = self
                .vm
                .types
                .entries
                .iter()
                .filter_map(|entry| match &entry.data {
                    TypeData::Struct { fields } | TypeData::Union { fields } => Some(fields.len()),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
        }
        if bounds.arrays == 0 {
            bounds.elements = 0;
        }
        bounds
    }
}
