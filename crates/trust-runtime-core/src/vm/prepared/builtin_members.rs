//! The members of each standard function block POU at their places (checklist P1-4 and P1-5;
//! specification 34 PERF-05). Preparation finds, for every POU that is a standard function block,
//! the position among the POU's own declarations of each member its native code reads or writes
//! and of each formal a call binds, by the name a scan would look it up by. A scan then reaches a
//! persistent instance's member by its position.

use super::*;
use crate::bytecode::StorageOwner;
use crate::stdlib::fbs;
use crate::stdlib::fbs::members::{
    formal_positions, member_ordinals, FormalPositions, MemberOrdinals,
};

/// The positions of one standard function block POU's members, and of its formals by formal index,
/// among its own declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinMembers {
    pub(crate) pou: u32,
    pub(crate) ordinals: MemberOrdinals,
    pub(crate) formals: FormalPositions,
}

crate::memory::figure::holds_no_heap!(BuiltinMembers);

impl PreparedModule {
    /// The member positions of every POU that is a standard function block, in POU order.
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
                        self.layout
                            .entries
                            .get(id as usize)
                            .and_then(|entry| strings.get(entry.name_idx as usize))
                    });
                let formals = self
                    .vm
                    .pou_params(*pou)
                    .unwrap_or(&[])
                    .iter()
                    .map(|param| param.name.as_str());
                BuiltinMembers {
                    pou: *pou,
                    ordinals: member_ordinals(declared.clone()),
                    formals: formal_positions(declared, formals),
                }
            })
            .collect();
        tables.sort_unstable_by_key(|table| table.pou);
        tables
    }

    /// The member positions of `pou` when it is a standard function block (checklist P1-4).
    pub(crate) fn builtin_members(&self, pou: u32) -> Option<&MemberOrdinals> {
        self.builtin_table(pou).map(|table| &table.ordinals)
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
}
