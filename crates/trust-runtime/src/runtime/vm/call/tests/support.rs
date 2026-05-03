use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::bytecode::TypeTable;
use crate::error::RuntimeError;
use crate::memory::{FrameId, MemoryLocation};
use crate::program_model::{FunctionBlockDef, Param};
use crate::stdlib::{time, StdParams};
use crate::value::{
    DateTimeValue, DateValue, LDateTimeValue, LTimeOfDayValue, RefPath, RefSegment, StructValue,
    TimeOfDayValue, Value, ValueRef,
};
use crate::Runtime;
use trust_hir::symbols::ParamDirection;
use trust_hir::TypeId;

use super::super::errors::VmTrap;
use super::super::frames::VmFrame;
use super::super::stack::OperandStack;
use super::super::{VmModule, VmNativeArgSpec, VmNativeSymbolSpec, VmParamMeta, VmPouEntry};
use super::{
    preparse_native_symbol_spec, VmFbOutSource, VmWriteTarget, VM_LOCAL_SENTINEL_FRAME_ID,
};

fn manual_vm_function_block_module(params: Vec<VmParamMeta>) -> (VmModule, u32) {
    let pou_id = 1_u32;
    let mut pou_by_id = HashMap::new();
    pou_by_id.insert(
        pou_id,
        VmPouEntry {
            name: SmolStr::new("FB"),
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            primary_instance_owner: None,
        },
    );
    let mut function_block_ids = HashMap::new();
    function_block_ids.insert(SmolStr::new("FB"), pou_id);
    let mut pou_params = HashMap::new();
    pou_params.insert(pou_id, params);
    (
        VmModule {
            code: Vec::new(),
            strings: Vec::new(),
            types: TypeTable::default(),
            refs: Vec::new(),
            consts: Vec::new(),
            pou_by_id,
            program_ids: HashMap::new(),
            function_ids: HashMap::new(),
            function_block_ids,
            class_ids: HashMap::new(),
            native_symbol_specs: Vec::new(),
            pou_params,
            pou_has_return_slot: HashSet::new(),
            method_table_by_owner: HashMap::new(),
            debug_map: super::super::debug_map::VmDebugMap::default(),
            instruction_budget: super::super::DEFAULT_INSTRUCTION_BUDGET,
        },
        pou_id,
    )
}

fn manual_vm_function_module(
    name: &str,
    params: Vec<VmParamMeta>,
    has_return_slot: bool,
) -> (VmModule, u32) {
    let pou_id = 1_u32;
    let mut pou_by_id = HashMap::new();
    pou_by_id.insert(
        pou_id,
        VmPouEntry {
            name: SmolStr::new(name),
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: params.len() as u32 + u32::from(has_return_slot),
            primary_instance_owner: None,
        },
    );
    let mut function_ids = HashMap::new();
    function_ids.insert(SmolStr::new(name.to_ascii_uppercase()), pou_id);
    let mut pou_params = HashMap::new();
    pou_params.insert(pou_id, params);
    let mut pou_has_return_slot = HashSet::new();
    if has_return_slot {
        pou_has_return_slot.insert(pou_id);
    }
    (
        VmModule {
            code: Vec::new(),
            strings: Vec::new(),
            types: TypeTable::default(),
            refs: Vec::new(),
            consts: Vec::new(),
            pou_by_id,
            program_ids: HashMap::new(),
            function_ids,
            function_block_ids: HashMap::new(),
            class_ids: HashMap::new(),
            native_symbol_specs: Vec::new(),
            pou_params,
            pou_has_return_slot,
            method_table_by_owner: HashMap::new(),
            debug_map: super::super::debug_map::VmDebugMap::default(),
            instruction_budget: super::super::DEFAULT_INSTRUCTION_BUDGET,
        },
        pou_id,
    )
}

fn expr_arg(name: Option<&str>, value: Value) -> super::VmNativeArg {
    super::VmNativeArg {
        name: name.map(SmolStr::new),
        value: super::VmNativeArgValue::Expr(value),
    }
}

fn target_arg(name: Option<&str>, reference: ValueRef) -> super::VmNativeArg {
    super::VmNativeArg {
        name: name.map(SmolStr::new),
        value: super::VmNativeArgValue::Target(reference),
    }
}

fn empty_caller_frame() -> VmFrame {
    VmFrame {
        pou_id: 0,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: vec![],
        runtime_instance: None,
        instance_owner: None,
    }
}

fn instance_field(runtime: &Runtime, instance: crate::memory::InstanceId, name: &str) -> Value {
    let offset = runtime
        .storage
        .declared_instance_field_offset(instance, name)
        .unwrap_or_else(|| panic!("declared instance field {name}"));
    runtime
        .storage
        .read_instance_field_by_offset(instance, offset)
        .unwrap_or_else(|| panic!("read instance field {name}"))
        .clone()
}

fn seed_ctu_instance(runtime: &mut Runtime) -> crate::memory::InstanceId {
    let instance = runtime.storage.create_instance("CTU");
    assert!(runtime
        .storage
        .set_instance_var(instance, "CU", Value::Bool(false)));
    assert!(runtime
        .storage
        .set_instance_var(instance, "R", Value::Bool(false)));
    assert!(runtime
        .storage
        .set_instance_var(instance, "PV", Value::Int(0)));
    assert!(runtime
        .storage
        .set_instance_var(instance, "Q", Value::Bool(false)));
    assert!(runtime
        .storage
        .set_instance_var(instance, "CV", Value::Int(0)));
    instance
}

fn write_int_to_global(initial: Value, value: i64) -> Result<Value, VmTrap> {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("OUT", initial);
    let reference = runtime
        .storage
        .ref_for_global("OUT")
        .expect("global output ref");
    let target = super::VmWriteTarget::from_reference(&reference);
    let mut caller_frame = empty_caller_frame();
    super::write_output_int(&mut runtime, &mut caller_frame, &target, value)?;
    Ok(runtime
        .storage
        .get_global("OUT")
        .expect("output global")
        .clone())
}

fn dispatch_stdlib(
    runtime: &mut Runtime,
    frame: &mut VmFrame,
    name: &str,
    args: &[super::VmNativeArg],
) -> Result<Value, VmTrap> {
    super::dispatch_native_stdlib_call(
        runtime,
        frame,
        &SmolStr::new(name),
        &SmolStr::new(name),
        None,
        args,
    )
}

fn output_target_arg(
    runtime: &mut Runtime,
    param_name: Option<&str>,
    global_name: &str,
) -> super::VmNativeArg {
    runtime.storage.set_global(global_name, Value::DInt(-1));
    let reference = runtime
        .storage
        .ref_for_global(global_name)
        .unwrap_or_else(|| panic!("{global_name} output ref"));
    target_arg(param_name, reference)
}

fn assert_invalid_argument_count(err: VmTrap, expected: usize, got: usize) {
    assert!(
        matches!(
            err,
            VmTrap::Runtime(RuntimeError::InvalidArgumentCount { expected: e, got: g })
                if e == expected && g == got
        ),
        "unexpected error: {err:?}"
    );
}
