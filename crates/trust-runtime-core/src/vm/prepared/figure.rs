//! The prepared program's part of the memory figure (specification 34 MEM-12, MEM-14; checklist
//! C1-21): the bytes of heap its tables hold, part by part, counted from the tables themselves.

use super::PreparedModule;
use crate::error::RuntimeError;
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

/// The memory figure of a prepared program (specification 34 MEM-12, MEM-14): the bytes of heap
/// that the program needs on the device that prepared it, part by part, which preparation computes
/// from the program and the device's own sizes. A scan allocates nothing, so the figure is what the
/// constructed program holds while it runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryFigure {
    /// The prepared program, its layout tables among them (MEM-14).
    pub program: usize,
    /// The persistent block: the globals, the programs and their instances, with its tables.
    pub persistent_block: usize,
    /// The activation stack for the deepest call chain, with what each call level takes.
    pub activation_stack: usize,
    /// The operand stack, its temporaries and buffers, and the register tier's register files.
    pub operand_stack: usize,
    /// The engine's own state: its records, the process images, the tasks, the bookkeeping of the
    /// instances, the input snapshot and the output journal.
    pub engine: usize,
    /// The capture of the retained values, which a runtime reserves when it keeps them.
    pub retain_capture: usize,
}

impl MemoryFigure {
    /// The figure of `prepared`, finished but for this figure.
    pub(super) fn of(prepared: &PreparedModule) -> Result<Self, RuntimeError> {
        let engine = crate::vm::engine::engine_parts(prepared)?;
        Ok(Self {
            program: prepared.program_bytes(),
            persistent_block: engine.persistent_block,
            activation_stack: engine.activation_stack,
            operand_stack: engine.operand_stack,
            engine: engine.engine,
            retain_capture: engine.retain_capture,
        })
    }

    /// The bytes of the whole figure, the capture of the retained values included when the runtime
    /// keeps them (`retains`).
    #[must_use]
    pub fn total(&self, retains: bool) -> usize {
        [
            self.program,
            self.persistent_block,
            self.activation_stack,
            self.operand_stack,
            self.engine,
            if retains { self.retain_capture } else { 0 },
        ]
        .iter()
        .fold(0usize, |sum, bytes| sum.saturating_add(*bytes))
    }
}

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

    /// The memory figure of this program, which preparation computed (specification 34 MEM-12).
    #[must_use]
    pub fn memory_figure(&self) -> MemoryFigure {
        self.memory_figure
    }

    /// The most bytes one capture of this program's retained values takes (specification 34
    /// MEM-09): a runtime that keeps the retained values reserves its capture for this many bytes,
    /// the staged payload and the one a save leaves the next capture, so that no capture grows it.
    /// The memory figure's `retain_capture` is twice this.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Overflow`] when the bytes overflow.
    pub fn retained_payload_bytes(&self) -> Result<usize, RuntimeError> {
        crate::vm::engine::retained_payload_bound(self).ok_or(RuntimeError::Overflow)
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
