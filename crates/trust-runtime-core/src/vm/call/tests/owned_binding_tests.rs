use alloc::{vec, vec::Vec};
use smol_str::SmolStr;

use super::super::super::dispatch::VmPouStackResult;
use super::super::super::module::{
    PouMap, VmModule, VmNativeArgSpec, VmNativeSymbolSpec, VmParamMeta, VmPouEntry,
};
use super::super::super::stack::OperandStack;
use super::super::super::{VmFrame, VmTrap};
use super::super::bindings::{
    bind_vm_call_arguments, bind_vm_call_arguments_owned, BoundVmCall, VmNativeArg,
    VmNativeArgValue,
};
use super::super::context::CallContext;
use super::super::continuation::{NativeCall, PreparedCall};
use super::super::{prepare_native_call, NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_METHOD};
use super::TestContext;
use crate::bytecode::{BytecodeVersion, PouKind, TypeData, TypeEntry, TypeKind, TypeTable};
use crate::error::RuntimeError;
use crate::memory::{FrameId, MemoryLocation};
use crate::value::{StorageAbi, Value, ValueRef};

const FUNCTION_ID: u32 = 7;

#[cfg(feature = "std")]
type TestMap<K, V> = std::collections::HashMap<K, V>;
#[cfg(not(feature = "std"))]
type TestMap<K, V> = alloc::collections::BTreeMap<K, V>;

fn function_module(
    params: Vec<VmParamMeta>,
    arg_specs: Vec<VmNativeArgSpec>,
    consts: Vec<Value>,
    local_ref_count: u32,
) -> VmModule {
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
        types: TypeTable {
            offsets: vec![0],
            entries: vec![TypeEntry {
                kind: TypeKind::Primitive,
                name_idx: None,
                data: TypeData::Primitive {
                    prim_id: 8, // DINT, matching the bound values and copy-back assertions.
                    max_length: 0,
                },
            }],
        },
        physical_layouts: Vec::new(),
        refs: Vec::new(),
        consts,
        pou_by_id,
        program_ids: TestMap::new(),
        #[cfg(feature = "hir")]
        function_ids: TestMap::new(),
        function_block_ids: TestMap::new(),
        class_ids: TestMap::new(),
        parent_pou_ids: Default::default(),
        interface_type_ids_by_pou: PouMap::default(),
        native_symbol_specs: vec![VmNativeSymbolSpec::Parsed {
            target_name: SmolStr::new("callee"),
            normalized_target_name: SmolStr::new("CALLEE"),
            resolved_function_pou_id: Some(FUNCTION_ID),
            conversion_spec: None,
            arg_specs,
        }],
        pou_params,
        pou_has_return_slot: Default::default(),
        method_table_by_owner: PouMap::default(),
        ref_types: Vec::new(),
        #[cfg(feature = "hir")]
        debug_map: crate::vm::debug_map::VmDebugMap::default(),
        instruction_budget: crate::vm::VM_MAX_EXECUTED_INSTRUCTIONS,
    }
}

fn input(name: &str, default_const_idx: Option<u32>) -> VmParamMeta {
    VmParamMeta {
        name: SmolStr::new(name),
        type_id: 0,
        direction: 0,
        default_const_idx,
    }
}

fn arg_spec(name: &str, is_target: bool) -> VmNativeArgSpec {
    VmNativeArgSpec {
        name: Some(SmolStr::new(name)),
        is_target,
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

fn local_target(offset: usize) -> ValueRef {
    ValueRef {
        location: MemoryLocation::Local(FrameId(crate::vm::context::VM_LOCAL_SENTINEL_FRAME_ID)),
        offset,
        path: Vec::new(),
    }
}

fn prepare_function(
    context: &mut TestContext,
    module: &VmModule,
    frame: &mut VmFrame,
    args: impl IntoIterator<Item = Value>,
    arg_count: u32,
) -> Result<NativeCall, VmTrap> {
    let mut stack = OperandStack::default();
    for value in args {
        stack.push(value)?;
    }
    prepare_native_call(
        context,
        module,
        frame,
        &mut stack,
        NATIVE_CALL_KIND_FUNCTION,
        0,
        arg_count,
    )
}

fn prepared_function(call: NativeCall) -> PreparedCall {
    let NativeCall::User(call) = call else {
        panic!("expected a user function call");
    };
    call
}

#[test]
fn private_function_route_moves_expressions_while_public_binder_keeps_copy_admission() {
    let module = function_module(
        vec![input("A", None), input("B", None)],
        vec![arg_spec("A", false), arg_spec("B", false)],
        Vec::new(),
        2,
    );
    let mut context = TestContext::new();
    let mut frame = caller(Vec::new());

    let call = prepared_function(
        prepare_function(
            &mut context,
            &module,
            &mut frame,
            [Value::DInt(11), Value::DInt(22)],
            2,
        )
        .unwrap(),
    );
    assert_eq!(
        call.locals.as_deref(),
        Some(&[Value::DInt(11), Value::DInt(22)][..])
    );
    assert_eq!(context.clone_admissions.get(), 0);

    context.clone_admissions.set(0);
    let borrowed_args = vec![
        VmNativeArg {
            name: Some(SmolStr::new("A")),
            value: VmNativeArgValue::Expr(Value::DInt(11)),
        },
        VmNativeArg {
            name: Some(SmolStr::new("B")),
            value: VmNativeArgValue::Expr(Value::DInt(22)),
        },
    ];
    let bound: BoundVmCall =
        bind_vm_call_arguments(&mut context, &module, &frame, FUNCTION_ID, &borrowed_args).unwrap();
    assert_eq!(bound.locals, vec![Value::DInt(11), Value::DInt(22)]);
    assert_eq!(context.clone_admissions.get(), 2);
    assert!(matches!(
        &borrowed_args[0].value,
        VmNativeArgValue::Expr(Value::DInt(11))
    ));
    assert!(matches!(
        &borrowed_args[1].value,
        VmNativeArgValue::Expr(Value::DInt(22))
    ));
}

#[test]
fn private_function_reuses_the_return_slot_fact_for_sparse_pou_ids() {
    let mut module = function_module(Vec::new(), Vec::new(), Vec::new(), 1);
    let other_pou = module.pou_by_id.get(&FUNCTION_ID).unwrap().clone();
    module.pou_by_id.insert(100_003, other_pou);
    module.pou_params.insert(100_003, Vec::new());
    module.pou_has_return_slot.insert(100_003);
    let mut context = TestContext::new();
    let mut frame = caller(Vec::new());

    let void_call =
        prepared_function(prepare_function(&mut context, &module, &mut frame, [], 0).unwrap());
    assert!(!void_call.capture_return);

    module.pou_has_return_slot.insert(FUNCTION_ID);
    let returning_call =
        prepared_function(prepare_function(&mut context, &module, &mut frame, [], 0).unwrap());
    assert!(returning_call.capture_return);
}

#[test]
fn public_binding_preserves_missing_pou_before_missing_parameter_metadata() {
    let mut module = function_module(Vec::new(), Vec::new(), Vec::new(), 0);
    let pou = module.pou_by_id.remove(&FUNCTION_ID).unwrap();
    let mut context = TestContext::new();
    let frame = caller(Vec::new());

    let error =
        bind_vm_call_arguments(&mut context, &module, &frame, FUNCTION_ID, &[]).unwrap_err();
    assert!(matches!(error, VmTrap::MissingPou(id) if id == FUNCTION_ID));

    module.pou_by_id.insert(FUNCTION_ID, pou);
    module.pou_params.remove(&FUNCTION_ID);
    let error =
        bind_vm_call_arguments(&mut context, &module, &frame, FUNCTION_ID, &[]).unwrap_err();
    assert!(matches!(
        error,
        VmTrap::InvalidNativeCall(message)
            if message.as_str() == "missing parameter metadata for pou id 7"
    ));
}

#[test]
fn private_method_route_uses_the_owned_expression_binder() {
    let mut module = function_module(
        vec![input("A", None)],
        vec![arg_spec("A", false)],
        Vec::new(),
        1,
    );
    module.native_symbol_specs[0] = VmNativeSymbolSpec::Parsed {
        target_name: SmolStr::new("method"),
        normalized_target_name: SmolStr::new("METHOD"),
        resolved_function_pou_id: None,
        conversion_spec: None,
        arg_specs: vec![arg_spec("A", false)],
    };
    module.class_ids.insert(SmolStr::new("OWNER"), 8);
    module.method_table_by_owner.insert(
        8,
        [(SmolStr::new("METHOD"), FUNCTION_ID)]
            .into_iter()
            .collect(),
    );

    let mut context = TestContext::new();
    let instance = context.storage.create_instance("Owner");
    let mut frame = caller(Vec::new());
    let mut stack = OperandStack::default();
    stack.push(Value::Instance(instance)).unwrap();
    stack.push(Value::DInt(19)).unwrap();
    let NativeCall::User(call) = prepare_native_call(
        &mut context,
        &module,
        &mut frame,
        &mut stack,
        NATIVE_CALL_KIND_METHOD,
        0,
        2,
    )
    .unwrap() else {
        panic!("expected a user method call");
    };

    assert_eq!(call.locals.as_deref(), Some(&[Value::DInt(19)][..]));
    assert_eq!(call.instance, Some(instance));
    assert_eq!(context.clone_admissions.get(), 0);
}

#[test]
fn omitted_default_keeps_checked_constant_copy() {
    let module = function_module(
        vec![input("A", Some(0))],
        Vec::new(),
        vec![Value::DInt(37)],
        1,
    );
    let mut context = TestContext::new();
    let mut frame = caller(Vec::new());
    let call =
        prepared_function(prepare_function(&mut context, &module, &mut frame, [], 0).unwrap());

    assert_eq!(call.locals.as_deref(), Some(&[Value::DInt(37)][..]));
    assert_eq!(context.clone_admissions.get(), 1);
}

#[test]
fn output_and_inout_targets_keep_copyback_and_target_read_semantics() {
    let mut module = function_module(
        vec![
            VmParamMeta {
                name: SmolStr::new("OUT_VALUE"),
                type_id: 0,
                direction: 1,
                default_const_idx: None,
            },
            VmParamMeta {
                name: SmolStr::new("INOUT_VALUE"),
                type_id: 0,
                direction: 2,
                default_const_idx: None,
            },
        ],
        vec![arg_spec("OUT_VALUE", true), arg_spec("INOUT_VALUE", true)],
        Vec::new(),
        2,
    );
    module.ref_types = vec![Some(0), Some(0)];
    let mut context = TestContext::new();
    let mut frame = caller(vec![Value::DInt(4), Value::DInt(6)]);
    let mut call = prepared_function(
        prepare_function(
            &mut context,
            &module,
            &mut frame,
            [
                Value::Reference(Some(local_target(0))),
                Value::Reference(Some(local_target(1))),
            ],
            2,
        )
        .unwrap(),
    );
    assert_eq!(
        call.locals.as_deref(),
        Some(&[Value::Null, Value::DInt(6)][..])
    );
    assert_eq!(call.present.as_deref(), Some(&[false, true][..]));

    let original_locals = call.locals.clone();
    assert!(context
        .try_execute_optimized_with_parameter_presence_owned(
            &module,
            FUNCTION_ID,
            None,
            &mut call.locals,
            call.present.as_deref(),
            false,
            1,
            crate::vm::budget::ExecutionEntry::Nested,
        )
        .unwrap()
        .is_none());
    assert_eq!(call.locals, original_locals);

    let copies_before_completion = context.clone_admissions.get();
    context.reject_copies = true;
    call.complete(
        &mut context,
        &module,
        &mut frame,
        Ok(VmPouStackResult {
            return_value: None,
            locals: vec![Value::DInt(41), Value::DInt(42)],
        }),
    )
    .unwrap();
    assert_eq!(frame.locals, vec![Value::DInt(41), Value::DInt(42)]);
    assert_eq!(context.clone_admissions.get(), copies_before_completion);
    assert_eq!(context.recycled_locals, vec![Value::Null, Value::Null]);
}

#[test]
fn completed_output_moves_preserve_alias_order_and_preflight_failure_cleanup() {
    let mut module = function_module(
        ["FIRST", "SECOND"]
            .into_iter()
            .map(|name| VmParamMeta {
                name: name.into(),
                type_id: 0,
                direction: 1,
                default_const_idx: None,
            })
            .collect(),
        vec![arg_spec("FIRST", true), arg_spec("SECOND", true)],
        Vec::new(),
        2,
    );
    module.ref_types = vec![Some(0)];
    for reject_write in [false, true] {
        let mut context = TestContext::new();
        let mut frame = caller(vec![Value::DInt(6)]);
        let call = prepared_function(
            prepare_function(
                &mut context,
                &module,
                &mut frame,
                [
                    Value::Reference(Some(local_target(0))),
                    Value::Reference(Some(local_target(0))),
                ],
                2,
            )
            .unwrap(),
        );
        context.reject_copies = true;
        context.reject_writes = reject_write;
        let result = call.complete(
            &mut context,
            &module,
            &mut frame,
            Ok(VmPouStackResult {
                return_value: Some(Value::DInt(9)),
                locals: vec![Value::DInt(41), Value::DInt(42)],
            }),
        );
        if reject_write {
            assert!(matches!(
                result,
                Err(VmTrap::Runtime(RuntimeError::Overflow))
            ));
            assert_eq!(frame.locals, vec![Value::DInt(6)]);
            assert_eq!(context.recycled_locals, vec![Value::Null, Value::DInt(42)]);
        } else {
            assert_eq!(result.unwrap(), Value::DInt(9));
            assert_eq!(frame.locals, vec![Value::DInt(42)]);
            assert_eq!(context.recycled_locals, vec![Value::Null, Value::Null]);
        }
        assert_eq!(context.clone_admissions.get(), 0);
    }
}

#[test]
#[cfg(any(feature = "hir", feature = "register-ir"))]
fn selected_owned_entry_error_is_not_retried_through_the_stack_executor() {
    let module = function_module(
        vec![input("A", None)],
        vec![arg_spec("A", false)],
        Vec::new(),
        1,
    );
    let mut context = TestContext::new();
    context.owned_entry_error = true;
    let mut frame = context.frame(0);
    let call = prepared_function(
        prepare_function(&mut context, &module, &mut frame, [Value::DInt(21)], 1).unwrap(),
    );

    let error = call
        .execute_sync(&mut context, &module, &mut frame, 0)
        .unwrap_err();
    assert!(matches!(
        error,
        VmTrap::Runtime(RuntimeError::InvalidExecutionState)
    ));
    assert_eq!(context.initialized_frames, 0);
    assert_eq!(frame.locals, vec![Value::DInt(0)]);
}

#[test]
fn duplicate_named_arguments_are_rejected_before_owned_input_moves() {
    let module = function_module(
        vec![input("A", None), input("B", None)],
        Vec::new(),
        Vec::new(),
        2,
    );
    let mut context = TestContext::new();
    let frame = caller(vec![Value::DInt(9)]);
    let mut args = vec![
        VmNativeArg {
            name: Some(SmolStr::new("A")),
            value: VmNativeArgValue::Expr(Value::DInt(11)),
        },
        VmNativeArg {
            name: Some(SmolStr::new("A")),
            value: VmNativeArgValue::Expr(Value::DInt(22)),
        },
    ];

    let error = bind_vm_call_arguments_owned(&mut context, &module, &frame, FUNCTION_ID, &mut args)
        .unwrap_err();
    assert!(matches!(error, VmTrap::InvalidNativeCall(_)));
    assert!(matches!(
        &args[0].value,
        VmNativeArgValue::Expr(Value::DInt(11))
    ));
    assert!(matches!(
        &args[1].value,
        VmNativeArgValue::Expr(Value::DInt(22))
    ));
    assert_eq!(context.clone_admissions.get(), 0);
    assert_eq!(frame.locals, vec![Value::DInt(9)]);
}

#[test]
fn later_target_read_failure_preserves_caller_after_earlier_expression_move() {
    let module = function_module(
        vec![input("A", None), input("B", None)],
        Vec::new(),
        Vec::new(),
        2,
    );
    let mut context = TestContext::new();
    context.reject_reads = true;
    let frame = caller(vec![Value::DInt(9)]);
    let mut args = vec![
        VmNativeArg {
            name: Some(SmolStr::new("A")),
            value: VmNativeArgValue::Expr(Value::DInt(11)),
        },
        VmNativeArg {
            name: Some(SmolStr::new("B")),
            value: VmNativeArgValue::Target(local_target(0)),
        },
    ];

    let error = bind_vm_call_arguments_owned(&mut context, &module, &frame, FUNCTION_ID, &mut args)
        .unwrap_err();
    assert!(matches!(error, VmTrap::Runtime(RuntimeError::Overflow)));
    assert!(matches!(
        &args[0].value,
        VmNativeArgValue::Expr(Value::Null)
    ));
    assert_eq!(frame.locals, vec![Value::DInt(9)]);
}

#[cfg(any(feature = "hir", feature = "register-ir"))]
#[test]
fn private_stack_entry_transfers_array_locals_but_borrowed_entry_keeps_copy_admission() {
    let module = function_module(Vec::new(), Vec::new(), Vec::new(), 1);
    let mut context = TestContext::new();
    context.reject_copies = true;
    let array = Value::Array(alloc::boxed::Box::new(
        crate::value::ArrayValue::from_canonical_parts(vec![Value::DInt(9); 8], vec![(1, 8)]),
    ));
    let mut locals = Some(vec![array]);
    let original_ptr = locals.as_ref().unwrap().as_ptr();
    let result = crate::vm::dispatch::execute_pou_stack_with_owned_locals(
        &mut context,
        &module,
        FUNCTION_ID,
        None,
        &mut locals,
        Some(&[true]),
        false,
        0,
        crate::vm::budget::ExecutionEntry::Nested,
    )
    .unwrap();
    assert!(locals.is_none());
    assert_eq!(result.locals.as_ptr(), original_ptr);
    assert!(
        matches!(&result.locals[0], Value::Array(array) if array.elements() == vec![Value::DInt(9); 8])
    );
    assert_eq!(context.initialized_presence, vec![true]);
    assert_eq!(context.clone_admissions.get(), 0);
    assert_eq!(context.retired_frames, 1);

    let error = crate::vm::dispatch::execute_pou_stack_with_parameter_presence(
        &mut context,
        &module,
        FUNCTION_ID,
        None,
        Some(&result.locals),
        Some(&[true]),
        false,
        0,
        crate::vm::budget::ExecutionEntry::Nested,
    )
    .unwrap_err();
    assert_eq!(error, VmTrap::BudgetExceeded.into_runtime_error());
    assert_eq!(context.clone_admissions.get(), 1);
    assert_eq!(result.locals.as_ptr(), original_ptr);
}

#[cfg(any(feature = "hir", feature = "register-ir"))]
#[test]
fn owned_stack_transfer_admission_and_initialization_faults_preserve_cleanup() {
    let module = function_module(Vec::new(), Vec::new(), Vec::new(), 1);
    for reject_initialization in [false, true] {
        let mut context = TestContext::new();
        context.reject_local_transfer = !reject_initialization;
        context.reject_initialization = reject_initialization;
        let mut locals = Some(vec![Value::DInt(21)]);
        let error = crate::vm::dispatch::execute_pou_stack_with_owned_locals(
            &mut context,
            &module,
            FUNCTION_ID,
            None,
            &mut locals,
            Some(&[true]),
            false,
            0,
            crate::vm::budget::ExecutionEntry::Nested,
        )
        .unwrap_err();
        assert_eq!(error, RuntimeError::Overflow);
        assert_eq!(
            context.initialized_frames,
            usize::from(reject_initialization)
        );
        assert_eq!(context.retired_frames, 1);
        assert_eq!(context.clone_admissions.get(), 0);
        if reject_initialization {
            assert!(locals.is_none());
        } else {
            assert_eq!(locals, Some(vec![Value::DInt(21)]));
        }
    }
}

#[cfg(any(feature = "hir", feature = "register-ir"))]
#[test]
fn declined_owned_hook_enters_stack_once_and_restores_parent_after_callee_fault() {
    for fault in [false, true] {
        let mut module = function_module(
            vec![input("A", None)],
            vec![arg_spec("A", false)],
            Vec::new(),
            1,
        );
        if fault {
            module.code = vec![0xff];
            module.pou_by_id.get_mut(&FUNCTION_ID).unwrap().code_end = 1;
        }
        let mut context = TestContext::new();
        let mut frame = context.frame(17);
        let call = prepared_function(
            prepare_function(&mut context, &module, &mut frame, [Value::DInt(21)], 1).unwrap(),
        );
        context.reject_copies = true;
        let result = call.execute_sync(&mut context, &module, &mut frame, 0);
        if fault {
            assert_eq!(
                result.unwrap_err().into_runtime_error(),
                VmTrap::InvalidOpcode(0xff).into_runtime_error()
            );
        } else {
            assert_eq!(result.unwrap(), Value::Null);
        }
        assert_eq!(context.owned_entry_calls, 1);
        assert_eq!(context.initialized_frames, 1);
        assert_eq!(context.retired_frames, 1);
        assert_eq!(frame.locals, vec![Value::DInt(17)]);
        assert_eq!(context.storage.execution_frame_owner_count(), 0);
        assert_eq!(context.clone_admissions.get(), 0);
    }
}

#[cfg(any(feature = "hir", feature = "register-ir"))]
#[test]
fn owned_stack_entry_preserves_supplied_null_presence_and_partial_copy_fallback() {
    let module = function_module(Vec::new(), Vec::new(), Vec::new(), 2);
    for present in [false, true] {
        let mut context = TestContext::new();
        let mut locals = Some(vec![Value::Null, Value::Null]);
        let result = crate::vm::dispatch::execute_pou_stack_with_owned_locals(
            &mut context,
            &module,
            FUNCTION_ID,
            None,
            &mut locals,
            Some(&[present, false]),
            false,
            0,
            crate::vm::budget::ExecutionEntry::Nested,
        )
        .unwrap();
        assert_eq!(result.locals, vec![Value::Null, Value::Null]);
        assert_eq!(context.initialized_presence, vec![present, false]);
        assert!(locals.is_none());
    }
    let mut context = TestContext::new();
    let mut partial = Some(vec![Value::DInt(21)]);
    let result = crate::vm::dispatch::execute_pou_stack_with_owned_locals(
        &mut context,
        &module,
        FUNCTION_ID,
        None,
        &mut partial,
        Some(&[true]),
        false,
        0,
        crate::vm::budget::ExecutionEntry::Nested,
    )
    .unwrap();
    assert_eq!(partial, Some(vec![Value::DInt(21)]));
    assert_eq!(result.locals, vec![Value::DInt(21), Value::Null]);
    assert_eq!(context.clone_admissions.get(), 1);

    context.reject_copies = true;
    let mut full = Some(vec![Value::DInt(21), Value::Null]);
    let error = crate::vm::dispatch::execute_pou_stack_with_owned_locals(
        &mut context,
        &module,
        FUNCTION_ID,
        None,
        &mut full,
        Some(&[true; 3]),
        false,
        0,
        crate::vm::budget::ExecutionEntry::Nested,
    )
    .unwrap_err();
    // Preserve copy-admission precedence over malformed presence on fallback.
    assert_eq!(error, VmTrap::BudgetExceeded.into_runtime_error());
    assert_eq!(full, Some(vec![Value::DInt(21), Value::Null]));
}

#[cfg(any(feature = "hir", feature = "register-ir"))]
#[test]
fn owned_stack_in_out_copyback_uses_completed_locals_after_parent_resume() {
    let mut module = function_module(
        vec![VmParamMeta {
            name: "VALUE".into(),
            type_id: 0,
            direction: 2,
            default_const_idx: None,
        }],
        vec![arg_spec("VALUE", true)],
        vec![Value::DInt(42)],
        1,
    );
    module.refs = vec![crate::vm::module::VmRef::Local {
        owner_frame_id: 0,
        offset: 0,
        path: crate::value::RefPath::new(),
    }];
    module.ref_types = vec![Some(0)];
    module.code = vec![0x10, 0, 0, 0, 0, 0x21, 0, 0, 0, 0];
    module.pou_by_id.get_mut(&FUNCTION_ID).unwrap().code_end = module.code.len();
    let mut context = TestContext::new();
    let mut frame = context.frame(41);
    let call = prepared_function(
        prepare_function(
            &mut context,
            &module,
            &mut frame,
            [Value::Reference(Some(local_target(0)))],
            1,
        )
        .unwrap(),
    );
    let copies_before_entry = context.clone_admissions.get();
    assert_eq!(
        call.execute_sync(&mut context, &module, &mut frame, 0)
            .unwrap(),
        Value::Null
    );
    assert_eq!(frame.locals, vec![Value::DInt(42)]);
    assert_eq!(context.initialized_presence, vec![true]);
    // The one remaining copy is PUSH_CONST; local entry and completed output move.
    assert_eq!(context.clone_admissions.get(), copies_before_entry + 1);
    assert_eq!(context.recycled_locals, vec![Value::Null]);
    assert_eq!(context.storage.execution_frame_owner_count(), 0);
}

#[cfg(any(feature = "hir", feature = "register-ir"))]
#[test]
fn declined_owned_hook_entry_admission_fault_restores_parent_without_initializing_callee() {
    let module = function_module(
        vec![input("A", None)],
        vec![arg_spec("A", false)],
        Vec::new(),
        1,
    );
    let mut context = TestContext::new();
    let mut frame = context.frame(17);
    let call = prepared_function(
        prepare_function(&mut context, &module, &mut frame, [Value::DInt(21)], 1).unwrap(),
    );
    context.reject_local_transfer = true;
    assert_eq!(
        call.execute_sync(&mut context, &module, &mut frame, 0)
            .unwrap_err()
            .into_runtime_error(),
        RuntimeError::Overflow
    );
    assert_eq!(context.owned_entry_calls, 1);
    assert_eq!(context.initialized_frames, 0);
    assert_eq!(context.retired_frames, 1);
    assert_eq!(frame.locals, vec![Value::DInt(17)]);
    assert_eq!(context.storage.execution_frame_owner_count(), 0);
    assert_eq!(context.clone_admissions.get(), 0);
}

#[cfg(all(feature = "std", feature = "test-support"))]
#[test]
fn array_target_attribution_uses_resolved_formals_for_reordered_named_inputs() {
    use crate::vm::diagnostic_timing::{
        attribution, finish_callsite_accounting_log, start_callsite_accounting_log,
        CopyConsumerKind, CopyParameterRole, CopyReadKind,
    };
    for (direction, role) in [(0, CopyParameterRole::Input), (2, CopyParameterRole::InOut)] {
        let mut params = vec![input("A", None), input("B", None)];
        for param in &mut params {
            param.direction = direction;
        }
        let mut module = function_module(params, Vec::new(), Vec::new(), 2);
        module.types = TypeTable {
            offsets: vec![0, 1],
            entries: vec![
                TypeEntry {
                    kind: TypeKind::Array,
                    name_idx: None,
                    data: TypeData::Array {
                        elem_type_id: 1,
                        dims: vec![(0, 1)],
                    },
                },
                TypeEntry {
                    kind: TypeKind::Primitive,
                    name_idx: None,
                    data: TypeData::Primitive {
                        prim_id: 8,
                        max_length: 0,
                    },
                },
            ],
        };
        let array = |n| {
            Value::Array(alloc::boxed::Box::new(
                crate::value::ArrayValue::from_canonical_parts(
                    vec![Value::DInt(n); 2],
                    vec![(0, 1)],
                ),
            ))
        };
        let frame = caller(vec![array(11), array(22)]);
        let target = |name: &str, offset| VmNativeArg {
            name: Some(SmolStr::new(name)),
            value: VmNativeArgValue::Target(ValueRef {
                location: MemoryLocation::Local(frame.reference_frame_id()),
                offset,
                path: Vec::new(),
            }),
        };
        let mut args = vec![target("B", 1), target("A", 0)];
        let mut context = TestContext::new();
        start_callsite_accounting_log();
        let bound = {
            let _native = attribution::native(0);
            bind_vm_call_arguments_owned(&mut context, &module, &frame, FUNCTION_ID, &mut args)
                .unwrap()
                .0
        };
        let snapshot = finish_callsite_accounting_log();
        assert!(snapshot.is_valid());
        assert_eq!(bound.locals, frame.locals);
        assert_eq!(bound.out_bindings.len(), if direction == 2 { 2 } else { 0 });
        assert_eq!(snapshot.copy_len, 1);
        assert_eq!(snapshot.copies[0].calls, 2);
        assert_eq!(snapshot.array_consumers.len(), 2);
        for formal in 0..2 {
            let row = snapshot
                .array_consumers
                .iter()
                .find(|r| r.consumer.formal == Some(formal))
                .unwrap();
            assert_eq!(row.consumer.pou_id, Some(FUNCTION_ID));
            assert_eq!(row.consumer.symbol_idx, Some(0));
            assert_eq!(row.consumer.kind, CopyConsumerKind::UserCall);
            assert_eq!(row.consumer.role, role);
            assert_eq!(row.consumer.read, CopyReadKind::CallerLocal);
            assert_eq!((row.copy.calls, row.copy.shape_len_sum), (1, 2));
        }
        assert_eq!(
            snapshot
                .array_consumers
                .iter()
                .map(|r| r.copy.calls)
                .sum::<u64>(),
            snapshot.copies[0].calls
        );
    }
}

#[test]
fn standard_library_calls_complete_immediately() {
    // The register leaf flag counts a standard-library call as one that cannot
    // hand a user call back to its caller (`native_call_may_yield`).
    assert!(!super::super::native_call_may_yield(
        crate::bytecode::NATIVE_CALL_KIND_STDLIB
    ));
    let mut module = function_module(Vec::new(), Vec::new(), Vec::new(), 0);
    module.native_symbol_specs = vec![crate::vm::symbols::preparse_native_symbol_spec(
        &SmolStr::new("ABS|E:IN"),
    )];
    let mut context = TestContext::new();
    let mut frame = caller(Vec::new());
    let mut stack = OperandStack::default();
    stack.push(Value::Int(-3)).unwrap();
    let call = prepare_native_call(
        &mut context,
        &module,
        &mut frame,
        &mut stack,
        crate::bytecode::NATIVE_CALL_KIND_STDLIB,
        0,
        1,
    )
    .unwrap();
    assert!(matches!(call, NativeCall::Immediate(Value::Int(3))));
}
