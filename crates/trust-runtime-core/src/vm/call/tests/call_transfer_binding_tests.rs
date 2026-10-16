use alloc::{vec, vec::Vec};
use smol_str::SmolStr;

use super::super::super::module::{PouMap, VmModule, VmParamMeta, VmPouEntry};
use super::super::super::{VmFrame, VmTrap};
use super::super::bindings::{
    bind_vm_call_arguments, bind_vm_call_arguments_owned, VmNativeArg, VmNativeArgValue,
};
use super::TestContext;
use crate::bytecode::{BytecodeVersion, PouKind, TypeTable};
use crate::value::{StorageAbi, Value};

const FUNCTION_ID: u32 = 7;

#[cfg(feature = "std")]
type TestMap<K, V> = std::collections::HashMap<K, V>;
#[cfg(not(feature = "std"))]
type TestMap<K, V> = alloc::collections::BTreeMap<K, V>;

fn function_module(params: Vec<VmParamMeta>, local_ref_count: u32) -> VmModule {
    let pou = VmPouEntry {
        kind: PouKind::Function,
        name: SmolStr::new("callee"),
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count,
        primary_instance_owner: None,
    };
    let mut pou_by_id = PouMap::default();
    pou_by_id.insert(FUNCTION_ID, pou);
    let mut pou_params = PouMap::default();
    pou_params.insert(FUNCTION_ID, params);
    VmModule {
        version: BytecodeVersion::LEGACY,
        storage_abi: StorageAbi::LegacyTemporal32,
        code: Vec::new(),
        strings: vec![SmolStr::new("callee")],
        types: TypeTable::default(),
        physical_layouts: Vec::new(),
        refs: Vec::new(),
        consts: Vec::new(),
        pou_by_id,
        program_ids: TestMap::new(),
        #[cfg(feature = "hir")]
        function_ids: TestMap::new(),
        function_block_ids: TestMap::new(),
        class_ids: TestMap::new(),
        parent_pou_ids: Default::default(),
        interface_type_ids_by_pou: PouMap::default(),
        native_symbol_specs: Vec::new(),
        pou_params,
        pou_has_return_slot: Default::default(),
        method_table_by_owner: PouMap::default(),
        ref_types: Vec::new(),
        #[cfg(feature = "hir")]
        debug_map: crate::vm::debug_map::VmDebugMap::default(),
        instruction_budget: crate::vm::VM_MAX_EXECUTED_INSTRUCTIONS,
    }
}

fn input(name: &str) -> VmParamMeta {
    VmParamMeta {
        name: SmolStr::new(name),
        type_id: 0,
        direction: 0,
        default_const_idx: None,
    }
}

fn caller(locals: Vec<Value>) -> VmFrame {
    VmFrame {
        activation: None,
        pou_id: Some(1),
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: locals.len() as u32,
        locals,
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    }
}

fn unknown_named_argument() -> Vec<VmNativeArg> {
    vec![VmNativeArg {
        name: Some(SmolStr::new("Z")),
        value: VmNativeArgValue::Expr(Value::DInt(1)),
    }]
}

fn assert_unknown_name_rejected(error: VmTrap) {
    let VmTrap::InvalidNativeCall(message) = error else {
        panic!("an unknown named argument must be an InvalidNativeCall");
    };
    assert_eq!(message.as_str(), "unexpected named argument 'Z'");
}

/// Spec 12 §7.7: an unknown named argument rejects the call before any move, copy or callee run.
#[test]
fn call_transfer_unknown_named_argument_is_rejected_before_any_move_or_copy() {
    let module = function_module(vec![input("A")], 1);
    let mut context = TestContext::new();
    let frame = caller(vec![Value::DInt(9)]);

    let mut owned = unknown_named_argument();
    let error =
        bind_vm_call_arguments_owned(&mut context, &module, &frame, FUNCTION_ID, &mut owned)
            .unwrap_err();
    assert_unknown_name_rejected(error);
    assert!(matches!(
        &owned[0].value,
        VmNativeArgValue::Expr(Value::DInt(1))
    ));
    assert_eq!(context.clone_admissions.get(), 0);
    assert_eq!(frame.locals, vec![Value::DInt(9)]);
}

/// Spec 12 §7.7: the borrowed binder rejects an unknown named argument the same way.
#[test]
fn call_transfer_unknown_named_argument_is_rejected_by_the_borrowed_binder() {
    let module = function_module(vec![input("A")], 1);
    let mut context = TestContext::new();
    let frame = caller(vec![Value::DInt(9)]);

    let borrowed = unknown_named_argument();
    let error =
        bind_vm_call_arguments(&mut context, &module, &frame, FUNCTION_ID, &borrowed).unwrap_err();
    assert_unknown_name_rejected(error);
    assert!(matches!(
        &borrowed[0].value,
        VmNativeArgValue::Expr(Value::DInt(1))
    ));
    assert_eq!(context.clone_admissions.get(), 0);
    assert_eq!(frame.locals, vec![Value::DInt(9)]);
}
