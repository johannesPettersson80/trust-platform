use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::bytecode::TypeTable;
use crate::error::RuntimeError;
use crate::memory::{FrameId, MemoryLocation};
use crate::value::{RefPath, RefSegment, StructValue, Value, ValueRef};
use crate::Runtime;

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

#[test]
fn bind_vm_function_block_arguments_skips_omitted_out_without_field_resolution() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    let (module, pou_id) = manual_vm_function_block_module(vec![VmParamMeta {
        name: SmolStr::new("OUT"),
        direction: 1,
        default_const_idx: None,
    }]);
    let caller_frame = empty_caller_frame();

    let bindings = super::bind_vm_function_block_arguments(
        &mut runtime,
        &module,
        &caller_frame,
        pou_id,
        instance,
        &[],
    )
    .expect("omitted OUT should skip field binding resolution");

    assert!(bindings.is_empty());
}

#[test]
fn bind_vm_function_block_arguments_skips_omitted_inout_without_field_resolution() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    let (module, pou_id) = manual_vm_function_block_module(vec![VmParamMeta {
        name: SmolStr::new("ACC"),
        direction: 2,
        default_const_idx: None,
    }]);
    let caller_frame = empty_caller_frame();

    let bindings = super::bind_vm_function_block_arguments(
        &mut runtime,
        &module,
        &caller_frame,
        pou_id,
        instance,
        &[],
    )
    .expect("omitted IN_OUT should skip field binding resolution");

    assert!(bindings.is_empty());
}

#[test]
fn resolve_named_arg_index_prefers_in_order_next_argument() {
    let args = vec![
        super::VmNativeArg {
            name: Some(SmolStr::new("ENABLE")),
            value: super::VmNativeArgValue::Expr(Value::Bool(true)),
        },
        super::VmNativeArg {
            name: Some(SmolStr::new("VALUE")),
            value: super::VmNativeArgValue::Expr(Value::DInt(1)),
        },
    ];
    let consumed = vec![false, false];
    let mut ordered_named_index = 0usize;

    let first = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Enable"),
        &mut ordered_named_index,
    );
    let second = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Value"),
        &mut ordered_named_index,
    );

    assert_eq!(first, Some(0));
    assert_eq!(second, Some(1));
}

#[test]
fn resolve_named_arg_index_handles_omitted_middle_parameter() {
    let args = vec![
        super::VmNativeArg {
            name: Some(SmolStr::new("ENABLE")),
            value: super::VmNativeArgValue::Expr(Value::Bool(true)),
        },
        super::VmNativeArg {
            name: Some(SmolStr::new("VALUE")),
            value: super::VmNativeArgValue::Expr(Value::DInt(1)),
        },
    ];
    let consumed = vec![false, false];
    let mut ordered_named_index = 0usize;

    let first = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Enable"),
        &mut ordered_named_index,
    );
    let missing = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Increment"),
        &mut ordered_named_index,
    );
    let second = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Value"),
        &mut ordered_named_index,
    );

    assert_eq!(first, Some(0));
    assert_eq!(missing, None);
    assert_eq!(second, Some(1));
}

#[test]
fn resolve_named_arg_index_falls_back_for_out_of_order_named_arguments() {
    let args = vec![
        super::VmNativeArg {
            name: Some(SmolStr::new("VALUE")),
            value: super::VmNativeArgValue::Expr(Value::DInt(1)),
        },
        super::VmNativeArg {
            name: Some(SmolStr::new("ENABLE")),
            value: super::VmNativeArgValue::Expr(Value::Bool(true)),
        },
    ];
    let mut consumed = vec![false, false];
    let mut ordered_named_index = 0usize;

    let enable = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Enable"),
        &mut ordered_named_index,
    );
    consumed[enable.expect("enable index")] = true;
    let value = super::resolve_named_arg_index(
        &args,
        &consumed,
        &SmolStr::new("Value"),
        &mut ordered_named_index,
    );

    assert_eq!(enable, Some(1));
    assert_eq!(value, Some(0));
}

#[test]
fn unpack_native_call_payload_preserves_receiver_and_argument_order() {
    let mut operand_stack = OperandStack::default();
    let target_ref = ValueRef {
        location: MemoryLocation::Global,
        offset: 3,
        path: RefPath::new(),
    };
    operand_stack
        .push(Value::Instance(crate::memory::InstanceId(7)))
        .expect("push receiver");
    operand_stack.push(Value::DInt(11)).expect("push expr arg");
    operand_stack
        .push(Value::Reference(Some(target_ref.clone())))
        .expect("push target arg");

    let (receiver, args) = super::unpack_native_call_payload(
        &mut operand_stack,
        &[
            VmNativeArgSpec {
                name: Some(SmolStr::new("lhs")),
                is_target: false,
            },
            VmNativeArgSpec {
                name: Some(SmolStr::new("out")),
                is_target: true,
            },
        ],
        1,
    )
    .expect("decode payload");

    assert_eq!(
        receiver,
        Some(Value::Instance(crate::memory::InstanceId(7)))
    );
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].name.as_deref(), Some("lhs"));
    assert!(matches!(
        args[0].value,
        super::VmNativeArgValue::Expr(Value::DInt(11))
    ));
    assert_eq!(args[1].name.as_deref(), Some("out"));
    match &args[1].value {
        super::VmNativeArgValue::Target(reference) => assert_eq!(reference, &target_ref),
        super::VmNativeArgValue::Expr(_) => panic!("expected target arg"),
    }
}

#[test]
fn preparse_native_symbol_spec_parses_named_and_target_args() {
    let entry = preparse_native_symbol_spec(&SmolStr::new("Add|E:a|T:out"));
    match entry {
        VmNativeSymbolSpec::Parsed {
            target_name,
            normalized_target_name,
            resolved_function_pou_id,
            conversion_spec,
            arg_specs,
        } => {
            assert_eq!(target_name, SmolStr::new("Add"));
            assert_eq!(normalized_target_name, SmolStr::new("ADD"));
            assert_eq!(resolved_function_pou_id, None);
            assert!(conversion_spec.is_none());
            assert_eq!(arg_specs.len(), 2);
            assert_eq!(arg_specs[0].name.as_deref(), Some("a"));
            assert!(!arg_specs[0].is_target);
            assert_eq!(arg_specs[1].name.as_deref(), Some("out"));
            assert!(arg_specs[1].is_target);
        }
        VmNativeSymbolSpec::ParseError(err) => {
            panic!("unexpected parse error: {err}");
        }
    }
}

#[test]
fn preparse_native_symbol_spec_caches_conversion_spec() {
    let entry = preparse_native_symbol_spec(&SmolStr::new("INT_TO_UDINT|E:IN"));
    match entry {
        VmNativeSymbolSpec::Parsed {
            normalized_target_name,
            conversion_spec,
            ..
        } => {
            assert_eq!(normalized_target_name, SmolStr::new("INT_TO_UDINT"));
            assert!(conversion_spec.is_some());
        }
        VmNativeSymbolSpec::ParseError(err) => {
            panic!("unexpected parse error: {err}");
        }
    }
}

#[test]
fn bind_conversion_value_accepts_positional_and_named_in_only() {
    let mut runtime = Runtime::new();
    let frame = empty_caller_frame();

    let positional = [expr_arg(None, Value::DInt(7))];
    assert_eq!(
        super::bind_conversion_value(&mut runtime, &frame, &positional).unwrap(),
        Value::DInt(7)
    );

    let named = [expr_arg(Some("in"), Value::DInt(8))];
    assert_eq!(
        super::bind_conversion_value(&mut runtime, &frame, &named).unwrap(),
        Value::DInt(8)
    );

    let wrong_name = [expr_arg(Some("OUT"), Value::DInt(9))];
    let err = super::bind_conversion_value(&mut runtime, &frame, &wrong_name)
        .expect_err("conversion only accepts IN");
    assert!(matches!(
        err,
        VmTrap::Runtime(RuntimeError::InvalidArgumentName(name)) if name == "OUT"
    ));

    let mixed = [
        expr_arg(None, Value::DInt(1)),
        expr_arg(Some("IN"), Value::DInt(2)),
    ];
    let err = super::bind_conversion_value(&mut runtime, &frame, &mixed)
        .expect_err("mixed named and positional args should fail before arity");
    assert!(matches!(
        err,
        VmTrap::Runtime(RuntimeError::InvalidArgumentName(name)) if name == "<unnamed>"
    ));
}

#[test]
fn resolve_native_symbol_specs_caches_resolved_function_id() {
    let mut specs = vec![
        preparse_native_symbol_spec(&SmolStr::new("Add|E:a")),
        preparse_native_symbol_spec(&SmolStr::new("Len|E:in")),
    ];
    let mut function_ids = HashMap::new();
    function_ids.insert(SmolStr::new("ADD"), 7);

    super::resolve_native_symbol_specs(&mut specs, &function_ids);

    match &specs[0] {
        VmNativeSymbolSpec::Parsed {
            resolved_function_pou_id,
            ..
        } => assert_eq!(*resolved_function_pou_id, Some(7)),
        VmNativeSymbolSpec::ParseError(err) => panic!("unexpected parse error: {err}"),
    }
    match &specs[1] {
        VmNativeSymbolSpec::Parsed {
            resolved_function_pou_id,
            ..
        } => assert_eq!(*resolved_function_pou_id, None),
        VmNativeSymbolSpec::ParseError(err) => panic!("unexpected parse error: {err}"),
    }
}

#[test]
fn bind_vm_call_arguments_rejects_too_many_positional_arguments() {
    let mut runtime = Runtime::new();
    let (module, pou_id) = manual_vm_function_module(
        "DoWork",
        vec![VmParamMeta {
            name: SmolStr::new("IN"),
            direction: 0,
            default_const_idx: None,
        }],
        false,
    );
    let err = super::bind_vm_call_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        &[
            expr_arg(None, Value::DInt(1)),
            expr_arg(None, Value::DInt(2)),
        ],
    )
    .expect_err("extra positional argument should fail");

    match err {
        super::VmTrap::InvalidNativeCall(message) => {
            assert!(message.contains("too many positional arguments"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn bind_vm_call_arguments_keeps_omitted_middle_named_input_as_null() {
    let mut runtime = Runtime::new();
    let (module, pou_id) = manual_vm_function_module(
        "DoWork",
        vec![
            VmParamMeta {
                name: SmolStr::new("A"),
                direction: 0,
                default_const_idx: None,
            },
            VmParamMeta {
                name: SmolStr::new("B"),
                direction: 0,
                default_const_idx: None,
            },
            VmParamMeta {
                name: SmolStr::new("C"),
                direction: 0,
                default_const_idx: None,
            },
        ],
        false,
    );
    let (locals, out_bindings) = super::bind_vm_call_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        &[
            expr_arg(Some("A"), Value::DInt(1)),
            expr_arg(Some("C"), Value::DInt(3)),
        ],
    )
    .expect("named omission should bind remaining params");

    assert!(out_bindings.is_empty());
    assert_eq!(locals, vec![Value::DInt(1), Value::Null, Value::DInt(3)]);
}

#[test]
fn bind_stdlib_named_values_rejects_duplicate_named_argument() {
    let mut runtime = Runtime::new();
    let params = crate::stdlib::StdParams::Fixed(vec![SmolStr::new("IN"), SmolStr::new("N")]);
    let err = super::bind_stdlib_named_values(
        &mut runtime,
        &empty_caller_frame(),
        &params,
        &[
            expr_arg(Some("IN"), Value::DInt(1)),
            expr_arg(Some("IN"), Value::DInt(2)),
        ],
    )
    .expect_err("duplicate stdlib named arg should fail");

    assert!(matches!(
        err,
        super::VmTrap::Runtime(RuntimeError::InvalidArgumentName(name))
            if name == SmolStr::new("IN")
    ));
}

#[test]
fn bind_stdlib_named_values_variadic_rejects_hole() {
    let mut runtime = Runtime::new();
    let params = crate::stdlib::StdParams::Variadic {
        fixed: vec![SmolStr::new("IN")],
        prefix: SmolStr::new("IN"),
        start: 2,
        min: 2,
    };
    let err = super::bind_stdlib_named_values(
        &mut runtime,
        &empty_caller_frame(),
        &params,
        &[
            expr_arg(Some("IN"), Value::DInt(1)),
            expr_arg(Some("IN2"), Value::DInt(2)),
            expr_arg(Some("IN4"), Value::DInt(4)),
        ],
    )
    .expect_err("variadic hole should fail");

    assert!(matches!(
        err,
        super::VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: 4,
            got: 3,
        })
    ));
}

#[test]
fn bind_vm_function_block_arguments_supports_mixed_out_and_inout_rebinding() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", crate::value::Value::DInt(0)));
    assert!(runtime
        .storage
        .set_instance_var(instance, "ACC", crate::value::Value::DInt(11)));
    runtime.storage.set_global("OUT", Value::DInt(0));
    runtime.storage.set_global("ACC_SRC", Value::DInt(5));
    let out_ref = runtime
        .storage
        .ref_for_global("OUT")
        .expect("out global ref");
    let acc_ref = runtime
        .storage
        .ref_for_global("ACC_SRC")
        .expect("acc source ref");
    let (module, pou_id) = manual_vm_function_block_module(vec![
        VmParamMeta {
            name: SmolStr::new("OUT"),
            direction: 1,
            default_const_idx: None,
        },
        VmParamMeta {
            name: SmolStr::new("ACC"),
            direction: 2,
            default_const_idx: None,
        },
    ]);

    let bindings = super::bind_vm_function_block_arguments(
        &mut runtime,
        &module,
        &empty_caller_frame(),
        pou_id,
        instance,
        &[
            target_arg(Some("OUT"), out_ref.clone()),
            target_arg(Some("ACC"), acc_ref.clone()),
        ],
    )
    .expect("mixed OUT and IN_OUT binding should succeed");

    assert_eq!(bindings.len(), 2);
}

#[test]
fn vm_fb_out_source_reads_direct_instance_field() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", crate::value::Value::DInt(41)));

    let offset = runtime
        .storage
        .declared_instance_field_offset(instance, "OUT")
        .expect("declared OUT offset");
    let source = VmFbOutSource::Direct {
        instance_id: instance,
        offset,
    };

    assert!(matches!(
        source.read(&runtime).expect("direct out source read"),
        crate::value::Value::DInt(41)
    ));
}

#[test]
fn vm_fb_out_source_reads_reference_field() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", crate::value::Value::DInt(17)));

    let reference = runtime
        .storage
        .ref_for_instance_recursive(instance, "OUT")
        .expect("reference OUT field");
    let source = VmFbOutSource::Reference(reference.clone());
    assert_eq!(reference.location, MemoryLocation::Instance(instance));
    assert!(matches!(
        source.read(&runtime).expect("reference out source read"),
        crate::value::Value::DInt(17)
    ));
}

#[test]
fn vm_fb_field_binding_out_source_uses_direct_for_declared_fields() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", crate::value::Value::DInt(23)));

    let binding = super::VmFbFieldBinding::resolve(&runtime, instance, &SmolStr::new("OUT"))
        .expect("declared field binding");
    match binding.out_source() {
        VmFbOutSource::Direct {
            instance_id,
            offset,
        } => {
            assert_eq!(instance_id, instance);
            assert_eq!(offset, 0);
        }
        VmFbOutSource::Reference(_) => panic!("expected direct output source"),
    }
}

#[test]
fn vm_fb_field_binding_out_source_falls_back_to_reference_for_inherited_fields() {
    let mut runtime = Runtime::new();
    let base = runtime.storage.create_instance("BASE");
    let derived = runtime.storage.create_instance("DERIVED");
    runtime
        .storage
        .get_instance_mut(derived)
        .expect("derived instance")
        .parent = Some(base);
    assert!(runtime
        .storage
        .set_instance_var(base, "OUT", crate::value::Value::DInt(29)));

    let binding = super::VmFbFieldBinding::resolve(&runtime, derived, &SmolStr::new("OUT"))
        .expect("inherited field binding");
    match binding.out_source() {
        VmFbOutSource::Reference(reference) => {
            assert_eq!(reference.location, MemoryLocation::Instance(base));
            assert_eq!(reference.offset, 0);
        }
        VmFbOutSource::Direct { .. } => panic!("expected inherited fallback reference"),
    }
}

#[test]
fn vm_write_target_uses_direct_storage_for_empty_path_instance_refs() {
    let mut runtime = Runtime::new();
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "OUT", Value::DInt(31)));
    let reference = runtime
        .storage
        .ref_for_instance(instance, "OUT")
        .expect("instance output ref");
    let target = VmWriteTarget::from_reference(&reference);
    assert!(matches!(
        target.clone(),
        VmWriteTarget::DirectStorage {
            location: MemoryLocation::Instance(id),
            offset: 0
        } if id == instance
    ));

    let caller_frame = VmFrame {
        pou_id: 0,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: vec![],
        runtime_instance: None,
        instance_owner: None,
    };
    assert!(matches!(
        target
            .read(&mut runtime, &caller_frame)
            .expect("direct instance read"),
        Value::DInt(31)
    ));
}

#[test]
fn vm_write_target_uses_direct_storage_for_empty_path_global_refs() {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("OUT", Value::DInt(17));
    let reference = runtime
        .storage
        .ref_for_global("OUT")
        .expect("global output ref");
    let target = VmWriteTarget::from_reference(&reference);
    assert!(matches!(
        target.clone(),
        VmWriteTarget::DirectStorage {
            location: MemoryLocation::Global,
            offset: 0
        }
    ));

    let caller_frame = VmFrame {
        pou_id: 0,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: vec![],
        runtime_instance: None,
        instance_owner: None,
    };
    assert!(matches!(
        target
            .read(&mut runtime, &caller_frame)
            .expect("direct target read"),
        Value::DInt(17)
    ));
}

#[test]
fn vm_write_target_uses_caller_local_direct_for_empty_path_vm_locals() {
    let mut runtime = Runtime::new();
    let reference = ValueRef {
        location: MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID)),
        offset: 0,
        path: RefPath::new(),
    };
    let target = VmWriteTarget::from_reference(&reference);
    assert!(matches!(
        target.clone(),
        VmWriteTarget::CallerLocalDirect { offset: 0 }
    ));

    let mut caller_frame = VmFrame {
        pou_id: 0,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 1,
        locals: vec![Value::DInt(21)],
        runtime_instance: None,
        instance_owner: None,
    };
    assert!(matches!(
        target
            .read(&mut runtime, &caller_frame)
            .expect("local direct read"),
        Value::DInt(21)
    ));
    target
        .write(&mut runtime, &mut caller_frame, Value::DInt(42))
        .expect("local direct write");
    assert!(matches!(
        caller_frame.locals.first(),
        Some(&Value::DInt(42))
    ));
}

#[test]
fn vm_write_target_keeps_nested_path_targets_on_reference_fallback() {
    let reference = ValueRef {
        location: MemoryLocation::Global,
        offset: 0,
        path: [RefSegment::Field(SmolStr::new("VALUE"))]
            .into_iter()
            .collect(),
    };
    let target = VmWriteTarget::from_reference(&reference);
    assert!(matches!(target.clone(), VmWriteTarget::Reference(_)));
}

#[test]
fn read_vm_target_value_matches_generic_reference_path_across_reference_shapes() {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("GLOBAL", Value::DInt(7));
    let mut struct_fields = IndexMap::new();
    struct_fields.insert(SmolStr::new("VALUE"), Value::DInt(8));
    runtime.storage.set_global(
        "STRUCT",
        Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
            SmolStr::new("TEST_STRUCT"),
            struct_fields,
        ))),
    );
    let instance = runtime.storage.create_instance("FB");
    assert!(runtime
        .storage
        .set_instance_var(instance, "ACC", Value::DInt(9)));

    let global_ref = runtime
        .storage
        .ref_for_global("GLOBAL")
        .expect("global ref");
    let struct_ref = runtime
        .storage
        .ref_for_global("STRUCT")
        .expect("struct ref");
    let nested_ref = ValueRef {
        location: struct_ref.location,
        offset: struct_ref.offset,
        path: [RefSegment::Field(SmolStr::new("VALUE"))]
            .into_iter()
            .collect(),
    };
    let instance_ref = runtime
        .storage
        .ref_for_instance(instance, "ACC")
        .expect("instance ref");

    let caller_frame = VmFrame {
        pou_id: 0,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 1,
        locals: vec![Value::DInt(10)],
        runtime_instance: None,
        instance_owner: None,
    };
    let local_ref = ValueRef {
        location: MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID)),
        offset: 0,
        path: RefPath::new(),
    };

    for reference in [global_ref, nested_ref, instance_ref, local_ref] {
        let direct = super::read_vm_target_value(&mut runtime, &caller_frame, &reference)
            .expect("direct target value");
        let generic = super::read_vm_reference(&mut runtime, &caller_frame, &reference)
            .expect("generic target value");
        assert_eq!(direct, generic);
    }
}

#[test]
fn write_output_int_inspects_target_type_without_read_clone() {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("OUT", Value::DInt(0));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let reference = runtime
        .storage
        .ref_for_global("OUT")
        .expect("global output ref");
    let target = super::VmWriteTarget::from_reference(&reference);
    let mut caller_frame = empty_caller_frame();

    super::write_output_int(&mut runtime, &mut caller_frame, &target, 17)
        .expect("write output int");

    assert_eq!(runtime.storage.get_global("OUT"), Some(&Value::DInt(17)));
    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.value_ops.read_value_clones, 0);
}

#[test]
fn read_vm_target_value_avoids_clone_counter_for_scalar_direct_target() {
    let mut runtime = Runtime::new();
    runtime.storage.set_global("OUT", Value::DInt(23));
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();

    let reference = runtime
        .storage
        .ref_for_global("OUT")
        .expect("global output ref");
    let caller_frame = empty_caller_frame();

    let value = super::read_vm_target_value(&mut runtime, &caller_frame, &reference)
        .expect("read target value");

    assert_eq!(value, Value::DInt(23));
    let profile = runtime.vm_register_profile_snapshot();
    assert_eq!(profile.value_ops.read_value_clones, 0);
}

#[test]
fn preparse_native_symbol_spec_preserves_parse_error_message() {
    let entry = preparse_native_symbol_spec(&SmolStr::new("Add|Q:oops"));
    match entry {
        VmNativeSymbolSpec::ParseError(err) => {
            assert!(err.contains("must start with E/T"));
        }
        VmNativeSymbolSpec::Parsed { .. } => {
            panic!("expected parse error");
        }
    }
}
