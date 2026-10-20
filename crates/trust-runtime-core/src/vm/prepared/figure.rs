//! The prepared program's part of the memory figure (specification 34 MEM-12, MEM-14; checklist
//! C1-21): the bytes of heap its tables hold, part by part, counted from the tables themselves.

use super::PreparedModule;
use crate::memory::figure::HeapCount;

/// One part of a prepared program and the bytes of heap it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramPart {
    /// What the part holds.
    pub name: &'static str,
    /// Its bytes of heap.
    pub bytes: usize,
}

/// The number of parts of a prepared program ([`PreparedModule::program_parts`]).
pub const PROGRAM_PARTS: usize = 22;

impl PreparedModule {
    /// The bytes of heap this prepared program holds, part by part (specification 34 MEM-14): each
    /// table's capacity times its entry's size, with what its entries hold, and the module's own
    /// record, which a runtime keeps on the heap beside its tables (a chip in its program's memory,
    /// a computer behind a shared pointer), first. An allocation that several parts share, such as a
    /// long name's string, is counted once, in the last part that holds it, which is where freeing the
    /// program part by part gives it back. The tables are in the order in which `drop_in_parts`
    /// frees them; a part that the build has no tables for is 0.
    #[must_use]
    pub fn program_parts(&self) -> [ProgramPart; PROGRAM_PARTS] {
        let vm = &self.vm;
        let mut count = HeapCount::new();
        let mut parts = [ProgramPart { name: "", bytes: 0 }; PROGRAM_PARTS];
        let mut at = PROGRAM_PARTS;
        let mut part = |name: &'static str, count: &mut HeapCount| {
            at -= 1;
            parts[at] = ProgramPart {
                name,
                bytes: count.take(),
            };
        };
        // From the last part freed to the first.
        #[cfg(feature = "register-ir")]
        count.of(&self.register_plans);
        part("register plans", &mut count);
        #[cfg(feature = "hir")]
        {
            count.of(&vm.function_ids);
            count.of(&vm.debug_map);
        }
        part("module: source names and debug map", &mut count);
        count.of(&vm.native_symbol_specs);
        part("module: native symbols", &mut count);
        count.of(&vm.program_ids);
        count.of(&vm.function_block_ids);
        count.of(&vm.class_ids);
        count.of(&vm.parent_pou_ids);
        count.of(&vm.interface_type_ids_by_pou);
        count.of(&vm.method_table_by_owner);
        part("module: name maps", &mut count);
        count.of(&vm.pou_by_id);
        count.of(&vm.pou_params);
        count.of(&vm.pou_has_return_slot);
        part("module: POU entries and parameters", &mut count);
        count.of(&vm.consts);
        part("module: constants", &mut count);
        count.of(&vm.refs);
        count.of(&vm.ref_types);
        part("module: references", &mut count);
        count.of(&vm.types);
        count.of(&vm.physical_layouts);
        part("module: types", &mut count);
        count.of(&vm.strings);
        part("module: strings", &mut count);
        count.of(&vm.code);
        part("module: instructions", &mut count);
        count.of(&self.stdlib);
        count.of(&self.method_owners);
        count.of(&self.recipes);
        part("method owners and recipes", &mut count);
        count.of(&self.io_bindings);
        part("I/O bindings", &mut count);
        count.of(&self.resources);
        part("resources", &mut count);
        count.of(&self.access);
        part("access bindings", &mut count);
        count.of(&self.initializers);
        part("initializers", &mut count);
        count.of(&self.roots);
        part("construction roots", &mut count);
        count.of(&self.layout);
        part("storage layout", &mut count);
        count.of(&self.layouts);
        part("type and region layouts", &mut count);
        count.of(&self.indexes);
        part("prepared indexes", &mut count);
        count.of(&self.call_plans);
        part("call plans", &mut count);
        count.of(&self.statements);
        part("statement index (debugger)", &mut count);
        count.add(core::mem::size_of::<Self>());
        part("module record", &mut count);
        debug_assert_eq!(at, 0, "every part is named once");
        parts
    }

    /// The bytes of heap this prepared program holds: the sum of its
    /// [`program_parts`](Self::program_parts).
    #[must_use]
    pub fn program_bytes(&self) -> usize {
        self.program_parts()
            .iter()
            .fold(0usize, |sum, part| sum.saturating_add(part.bytes))
    }
}
