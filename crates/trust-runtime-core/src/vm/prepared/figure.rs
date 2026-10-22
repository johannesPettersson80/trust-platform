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
        count.of(&vm.compact);
        part("module: instructions", &mut count);
        count.of(&self.stdlib);
        count.of(&self.method_owners);
        count.of(&self.recipes);
        part("method owners and recipes", &mut count);
        count.of(&self.io_bindings);
        count.of(&self.io_leaves);
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
        count.of(&self.builtin_members);
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

#[cfg(test)]
mod exact_tables {
    extern crate std;

    use super::PreparedModule;
    use crate::vm::module::VmNativeSymbolSpec;
    use crate::vm::PreparationLimits;
    use std::format;

    /// Specification 34 MEM-14: each table of a prepared program holds exactly its entries, with no
    /// room for entries it does not hold, which the memory figure would count as the program's heap.
    /// Worked by hand: each table below is built once with the entries preparation found, so its
    /// capacity equals its length, in the corpus programs that have standard blocks (`blocks`),
    /// native calls with arguments (`strings`, `calls`), bindings (`io`) and compact bodies. Catches
    /// a table that grows by pushing and keeps the room of its last doubling: on 22 October 2026
    /// v1p's block members held room for four entries and used two, and its type table, which
    /// preparation extends by a timer's duration slot, room for 18 and used 10.
    #[test]
    fn the_prepared_tables_keep_no_room_they_do_not_use() {
        let mut filled = [0usize; 6];
        for name in ["blocks", "calls", "io", "strings", "scalar"] {
            let path = format!(
                "{}/../trust-runtime/tests/fixtures/bounded-c/{name}/program.stbc",
                env!("CARGO_MANIFEST_DIR")
            );
            let bytes = std::fs::read(&path).expect("the corpus program reads");
            let p = PreparedModule::from_bytes(&bytes, PreparationLimits::default())
                .expect("the corpus program prepares");
            let vm = &p.vm;
            let tables = [
                ("compact code", vm.compact.len(), vm.compact.capacity()),
                ("types", vm.types.entries.len(), vm.types.entries.capacity()),
                (
                    "I/O bindings",
                    p.io_bindings.len(),
                    p.io_bindings.capacity(),
                ),
                (
                    "block members",
                    p.builtin_members.len(),
                    p.builtin_members.capacity(),
                ),
                ("call plans", p.call_plans.len(), p.call_plans.capacity()),
            ];
            for (at, (table, len, capacity)) in tables.into_iter().enumerate() {
                assert_eq!(capacity, len, "{name}: {table}");
                filled[at] += len;
            }
            for spec in vm.native_symbol_specs.iter() {
                if let VmNativeSymbolSpec::Parsed { arg_specs, .. } = spec {
                    assert_eq!(
                        arg_specs.capacity(),
                        arg_specs.len(),
                        "{name}: a native call's arguments"
                    );
                    filled[5] += arg_specs.len();
                }
            }
        }
        assert!(
            filled.iter().all(|&entries| entries > 0),
            "every table has entries in some program: {filled:?}"
        );
    }
}

#[cfg(test)]
mod record_probe {
    extern crate std;

    use super::PreparedModule;
    use crate::vm::PreparationLimits;
    use core::mem::size_of_val;
    use std::vec::Vec;

    /// Prints the bytes of each field of a prepared program's own record, the part of the figure
    /// that every program holds, and of its module's, for the program that `TRUST_FIGURE_PROBE`
    /// names (MEM-14): `TRUST_FIGURE_PROBE=<program.stbc> <test binary> record_probe --ignored
    /// --nocapture`. The sizes are this host's.
    #[test]
    #[ignore = "a measurement probe"]
    fn record_probe() {
        let path = std::env::var("TRUST_FIGURE_PROBE").expect("TRUST_FIGURE_PROBE names a program");
        let bytes = std::fs::read(path).expect("the program reads");
        let p = PreparedModule::from_bytes(&bytes, PreparationLimits::default())
            .expect("the program prepares");
        let mut fields: Vec<(&str, usize)> = Vec::from([
            ("vm", size_of_val(&p.vm)),
            ("layout", size_of_val(&p.layout)),
            ("layouts", size_of_val(&p.layouts)),
            ("roots", size_of_val(&p.roots)),
            ("initializers", size_of_val(&p.initializers)),
            ("access", size_of_val(&p.access)),
            ("resources", size_of_val(&p.resources)),
            ("io_bindings", size_of_val(&p.io_bindings)),
            ("io_leaves", size_of_val(&p.io_leaves)),
            ("input_leaves", size_of_val(&p.input_leaves)),
            ("builtin_members", size_of_val(&p.builtin_members)),
            ("limits", size_of_val(&p.limits)),
            ("operand_bounds", size_of_val(&p.operand_bounds)),
            ("call_depth", size_of_val(&p.call_depth)),
            ("preparation_usage", size_of_val(&p.preparation_usage)),
            ("indexes", size_of_val(&p.indexes)),
            ("call_plans", size_of_val(&p.call_plans)),
            ("stdlib", size_of_val(&p.stdlib)),
            ("method_owners", size_of_val(&p.method_owners)),
            ("recipes", size_of_val(&p.recipes)),
            ("statements", size_of_val(&p.statements)),
            ("memory_figure", size_of_val(&p.memory_figure)),
        ]);
        #[cfg(feature = "register-ir")]
        fields.push(("register_plans", size_of_val(&p.register_plans)));
        let vm = &p.vm;
        fields.extend([
            ("vm.code", size_of_val(&vm.code)),
            ("vm.strings", size_of_val(&vm.strings)),
            ("vm.types", size_of_val(&vm.types)),
            ("vm.physical_layouts", size_of_val(&vm.physical_layouts)),
            ("vm.refs", size_of_val(&vm.refs)),
            ("vm.consts", size_of_val(&vm.consts)),
            ("vm.pou_by_id", size_of_val(&vm.pou_by_id)),
            ("vm.program_ids", size_of_val(&vm.program_ids)),
            ("vm.function_block_ids", size_of_val(&vm.function_block_ids)),
            ("vm.class_ids", size_of_val(&vm.class_ids)),
            ("vm.parent_pou_ids", size_of_val(&vm.parent_pou_ids)),
            (
                "vm.interface_type_ids_by_pou",
                size_of_val(&vm.interface_type_ids_by_pou),
            ),
            (
                "vm.native_symbol_specs",
                size_of_val(&vm.native_symbol_specs),
            ),
            ("vm.pou_params", size_of_val(&vm.pou_params)),
            (
                "vm.pou_has_return_slot",
                size_of_val(&vm.pou_has_return_slot),
            ),
            (
                "vm.method_table_by_owner",
                size_of_val(&vm.method_table_by_owner),
            ),
            ("vm.ref_types", size_of_val(&vm.ref_types)),
            ("vm.compact", size_of_val(&vm.compact)),
        ]);
        std::println!("record probe: the record {} B", size_of_val(&p));
        for (name, size) in fields {
            std::println!("record probe:   {name}: {size} B");
        }
    }

    /// Prints the heap bytes of each table of the prepared program that `TRUST_FIGURE_PROBE`
    /// names, largest first (MEM-14): `TRUST_FIGURE_PROBE=<program.stbc> <test binary>
    /// table_probe --ignored --nocapture`. The sizes are this host's.
    #[test]
    #[ignore = "a measurement probe"]
    fn table_probe() {
        use crate::memory::figure::HeapCount;
        let path = std::env::var("TRUST_FIGURE_PROBE").expect("TRUST_FIGURE_PROBE names a program");
        let bytes = std::fs::read(path).expect("the program reads");
        let p = PreparedModule::from_bytes(&bytes, PreparationLimits::default())
            .expect("the program prepares");
        let heap = |add: &dyn Fn(&mut HeapCount)| {
            let mut count = HeapCount::new();
            add(&mut count);
            count.take()
        };
        let vm = &p.vm;
        let mut tables: Vec<(std::string::String, usize)> = [
            ("vm.code", heap(&|count| count.of(&vm.code))),
            ("vm.compact", heap(&|count| count.of(&vm.compact))),
            ("vm.strings", heap(&|count| count.of(&vm.strings))),
            ("vm.types", heap(&|count| count.of(&vm.types))),
            (
                "vm.physical_layouts",
                heap(&|count| count.of(&vm.physical_layouts)),
            ),
            ("vm.refs", heap(&|count| count.of(&vm.refs))),
            ("vm.ref_types", heap(&|count| count.of(&vm.ref_types))),
            ("vm.consts", heap(&|count| count.of(&vm.consts))),
            ("vm.pou_by_id", heap(&|count| count.of(&vm.pou_by_id))),
            ("vm.pou_params", heap(&|count| count.of(&vm.pou_params))),
            (
                "vm.native_symbol_specs",
                heap(&|count| count.of(&vm.native_symbol_specs)),
            ),
            ("layout", heap(&|count| count.of(&p.layout))),
            ("layouts", heap(&|count| count.of(&p.layouts))),
            ("roots", heap(&|count| count.of(&p.roots))),
            ("initializers", heap(&|count| count.of(&p.initializers))),
            ("access", heap(&|count| count.of(&p.access))),
            ("resources", heap(&|count| count.of(&p.resources))),
            ("io_bindings", heap(&|count| count.of(&p.io_bindings))),
            ("io_leaves", heap(&|count| count.of(&p.io_leaves))),
            (
                "builtin_members",
                heap(&|count| count.of(&p.builtin_members)),
            ),
            ("call_plans", heap(&|count| count.of(&p.call_plans))),
            ("stdlib", heap(&|count| count.of(&p.stdlib))),
            ("statements", heap(&|count| count.of(&p.statements))),
        ]
        .into_iter()
        .map(|(name, bytes)| (std::string::String::from(name), bytes))
        .collect();
        tables.extend(
            p.indexes
                .table_heap()
                .into_iter()
                .map(|(name, bytes)| (std::format!("indexes.{name}"), bytes)),
        );
        tables.sort_by_key(|table| core::cmp::Reverse(table.1));
        for (name, bytes) in tables {
            std::println!("table probe:   {name}: {bytes} B");
        }
        // Length against capacity of the vectors the figure counts at their capacity.
        let fill = [
            ("vm.code", vm.code.len(), vm.code.capacity()),
            ("vm.refs", vm.refs.len(), vm.refs.capacity()),
            ("vm.consts", vm.consts.len(), vm.consts.capacity()),
            ("vm.compact", vm.compact.len(), vm.compact.capacity()),
            ("vm.ref_types", vm.ref_types.len(), vm.ref_types.capacity()),
            (
                "vm.types.entries",
                vm.types.entries.len(),
                vm.types.entries.capacity(),
            ),
            (
                "vm.types.offsets",
                vm.types.offsets.len(),
                vm.types.offsets.capacity(),
            ),
            (
                "layout.entries",
                p.layout.entries.len(),
                p.layout.entries.capacity(),
            ),
            (
                "initializers.entries",
                p.initializers.entries.len(),
                p.initializers.entries.capacity(),
            ),
            ("io_bindings", p.io_bindings.len(), p.io_bindings.capacity()),
            ("io_leaves", p.io_leaves.len(), p.io_leaves.capacity()),
            (
                "builtin_members",
                p.builtin_members.len(),
                p.builtin_members.capacity(),
            ),
            ("call_plans", p.call_plans.len(), p.call_plans.capacity()),
        ];
        for (name, len, capacity) in fill {
            std::println!("table probe: fill {name}: {len} of {capacity}");
        }
        for (pou, code) in &vm.compact {
            std::println!(
                "table probe: compact {pou}: ops {} of {}, slots {} of {}, blocks {} of {}, kinded {}",
                code.ops.len(),
                code.ops.capacity(),
                code.slots.len(),
                code.slots.capacity(),
                code.blocks.len(),
                code.blocks.capacity(),
                code.kinded.is_some()
            );
        }
        // The spare capacity of each table: its heap less that of its exact clone.
        let spare = |name: &str, original: usize, exact: usize| {
            std::println!(
                "table probe: spare {name}: {} of {original} B",
                original - exact
            );
        };
        macro_rules! spare_of {
            ($name:expr, $table:expr) => {{
                let exact = $table.clone();
                spare(
                    $name,
                    heap(&|count| count.of(&$table)),
                    heap(&|count| count.of(&exact)),
                );
            }};
        }
        spare_of!("vm.code", vm.code);
        spare_of!("vm.compact", vm.compact);
        spare_of!("vm.strings", vm.strings);
        spare_of!("vm.types", vm.types);
        spare_of!("vm.refs", vm.refs);
        spare_of!("vm.ref_types", vm.ref_types);
        spare_of!("vm.consts", vm.consts);
        spare_of!("vm.pou_by_id", vm.pou_by_id);
        spare_of!("vm.pou_params", vm.pou_params);
        spare_of!("vm.native_symbol_specs", vm.native_symbol_specs);
        spare_of!("layout", p.layout);
        spare_of!("initializers", p.initializers);
        spare_of!("io_bindings", p.io_bindings);
        spare_of!("builtin_members", p.builtin_members);
        let types = p.layouts.type_layouts().to_vec();
        spare(
            "layouts.types",
            heap(&|count| count.of(p.layouts.type_layouts())),
            heap(&|count| count.of(types.as_slice())),
        );
        std::println!(
            "table probe: sizes VmRef {} Value {} StorageDeclaration {} InitializerEntry {} \
             TypeLayout {} BuiltinMembers {} CompactCode {}",
            core::mem::size_of::<crate::vm::module::VmRef>(),
            core::mem::size_of::<crate::value::Value>(),
            core::mem::size_of::<crate::bytecode::StorageDeclaration>(),
            core::mem::size_of::<crate::bytecode::InitializerEntry>(),
            core::mem::size_of::<crate::memory::layout::TypeLayout>(),
            core::mem::size_of::<super::super::builtin_members::BuiltinMembers>(),
            core::mem::size_of::<crate::vm::compact::CompactCode>(),
        );
    }
}
