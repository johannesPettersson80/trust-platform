//! Where a prepared module's heap goes, part by part (specification 34 MEM-12; checklist C1): a
//! measuring test drops the module one part at a time and reads its allocator between the parts.

use super::PreparedModule;
use crate::vm::module::VmModule;

impl PreparedModule {
    /// Drop this module one part at a time, in a fixed order, calling `after` with the name of each
    /// part once it is freed, so a counting allocator can attribute every byte the module held.
    pub fn drop_in_parts(self, mut after: impl FnMut(&'static str)) {
        let Self {
            vm,
            #[cfg(feature = "register-ir")]
            register_plans,
            layout,
            layouts,
            roots,
            initializers,
            access,
            resources,
            io_bindings,
            indexes,
            call_plans,
            stdlib,
            method_owners,
            recipes,
            statements,
            ..
        } = self;
        after("start");
        drop(statements);
        after("statement index (debugger)");
        drop(call_plans);
        after("call plans");
        drop(indexes);
        after("prepared indexes");
        drop(layouts);
        after("type and region layouts");
        drop(layout);
        after("storage layout");
        drop(roots);
        after("construction roots");
        drop(initializers);
        after("initializers");
        drop(access);
        after("access bindings");
        drop(resources);
        after("resources");
        drop(io_bindings);
        after("I/O bindings");
        drop(stdlib);
        drop(method_owners);
        drop(recipes);
        after("method owners and recipes");
        drop_module(vm, &mut after);
        #[cfg(feature = "register-ir")]
        drop(register_plans);
        after("register plans");
    }
}

/// Drop the decoded module one part at a time, as [`PreparedModule::drop_in_parts`] does.
fn drop_module(vm: VmModule, after: &mut impl FnMut(&'static str)) {
    let VmModule {
        code,
        strings,
        types,
        physical_layouts,
        refs,
        consts,
        pou_by_id,
        program_ids,
        function_block_ids,
        class_ids,
        parent_pou_ids,
        interface_type_ids_by_pou,
        native_symbol_specs,
        pou_params,
        pou_has_return_slot,
        method_table_by_owner,
        ref_types,
        #[cfg(feature = "hir")]
        function_ids,
        #[cfg(feature = "hir")]
        debug_map,
        ..
    } = vm;
    drop(code);
    after("module: instructions");
    drop(strings);
    after("module: strings");
    drop(types);
    drop(physical_layouts);
    after("module: types");
    drop(refs);
    drop(ref_types);
    after("module: references");
    drop(consts);
    after("module: constants");
    drop(pou_by_id);
    drop(pou_params);
    drop(pou_has_return_slot);
    after("module: POU entries and parameters");
    drop(program_ids);
    drop(function_block_ids);
    drop(class_ids);
    drop(parent_pou_ids);
    drop(interface_type_ids_by_pou);
    drop(method_table_by_owner);
    after("module: name maps");
    drop(native_symbol_specs);
    after("module: native symbols");
    #[cfg(feature = "hir")]
    {
        drop(function_ids);
        drop(debug_map);
    }
    after("module: source names and debug map");
}
