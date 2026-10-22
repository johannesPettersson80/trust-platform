//! The members of each standard function block POU at their places (checklist P1-4 and P1-5;
//! specification 34 PERF-05). Preparation finds, for every POU that is a standard function block,
//! where each member its native code reads or writes lies in an instance's bytes, and the position
//! among the POU's own declarations of each formal a call binds, by the name a scan would look it up
//! by. A scan then reaches a persistent instance's member at its place.

use super::layouts::holds_presence;
use super::*;
use crate::bytecode::StorageOwner;
use crate::stdlib::fbs;
use crate::stdlib::fbs::members::{
    formal_positions, member_places, DeclaredPlace, FormalPositions, MemberPlaces,
};

/// The places of one standard function block POU's members in an instance's bytes, `None` when
/// they do not fit a `MemberPlace` and the members are found by their names, and the positions of
/// its formals by formal index among its own declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinMembers {
    pub(crate) pou: u32,
    pub(crate) places: Option<MemberPlaces>,
    pub(crate) formals: FormalPositions,
}

crate::memory::figure::holds_no_heap!(BuiltinMembers);

impl PreparedModule {
    /// The member places of every POU that is a standard function block, in POU order.
    pub(crate) fn builtin_member_tables(&self) -> Vec<BuiltinMembers> {
        let strings = self.strings();
        let mut tables: Vec<BuiltinMembers> = self
            .vm
            .pou_by_id
            .iter()
            .filter(|(pou, _)| {
                self.vm
                    .pou_name(**pou)
                    .and_then(fbs::builtin_kind)
                    .is_some()
            })
            .map(|(pou, _)| {
                let declared = self
                    .declarations(StorageOwner::Instance, Some(*pou))
                    .iter()
                    .map(|&id| {
                        let name = self
                            .layout
                            .entries
                            .get(id as usize)
                            .and_then(|entry| strings.get(entry.name_idx as usize));
                        (name, self.declared_place(id))
                    });
                let formals = self
                    .vm
                    .pou_params(*pou)
                    .unwrap_or(&[])
                    .iter()
                    .map(|param| param.name.as_str());
                BuiltinMembers {
                    pou: *pou,
                    places: member_places(declared.clone()),
                    formals: formal_positions(declared.map(|(name, _)| name), formals),
                }
            })
            .collect();
        tables.sort_unstable_by_key(|table| table.pou);
        // The table holds exactly the standard blocks' entries (specification 34 MEM-14).
        tables.shrink_to_fit();
        tables
    }

    /// The member places of `pou` when it is a standard function block whose places fit
    /// (checklist P1-4).
    pub(crate) fn builtin_places(&self, pou: u32) -> Option<&MemberPlaces> {
        self.builtin_table(pou)
            .and_then(|table| table.places.as_ref())
    }

    /// The positions of `pou`'s formals by formal index when it is a standard function block
    /// (checklist P1-5).
    pub(crate) fn builtin_formals(&self, pou: u32) -> Option<&FormalPositions> {
        self.builtin_table(pou).map(|table| &table.formals)
    }

    fn builtin_table(&self, pou: u32) -> Option<&BuiltinMembers> {
        self.builtin_members
            .binary_search_by_key(&pou, |table| table.pou)
            .ok()
            .map(|index| &self.builtin_members[index])
    }

    /// Where storage declaration `id` lies among its instance's own bytes, as the store finds a
    /// persistent instance's variable: its offset, its type's bytes, its type, whether it keeps a
    /// presence byte (specification 34 MEM-12) and its leaf's primitive when its type is a scalar.
    /// `None` where the store finds no place.
    fn declared_place(&self, id: u32) -> Option<DeclaredPlace> {
        let entry = self.layout.entries.get(id as usize)?;
        let type_id = entry.type_id?;
        let layout = self.type_layout(type_id)?;
        Some(DeclaredPlace {
            offset: self.declaration_offset(id)?,
            bytes: layout.shape.bytes,
            type_id,
            presence: holds_presence(entry.role),
            scalar: layout.scalar,
        })
    }
}
