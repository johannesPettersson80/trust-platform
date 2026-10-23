#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::base_module;
use trust_runtime_core::bytecode::{
    BytecodeError, BytecodeModule, ConstEntry, RejectionReason, SectionData, SectionId,
    ValidationLimits, BYTECODE_MAX_INSTRUCTIONS, BYTECODE_MAX_LOCALS_PER_POU,
    BYTECODE_MAX_REFERENCES, NATIVE_CALL_KIND_FUNCTION,
};

fn module_with_code(code: Vec<u8>) -> BytecodeModule {
    let mut module = base_module();
    let length = u32::try_from(code.len()).unwrap();
    let Some(SectionData::PouBodies(body)) = module.section_mut(SectionId::PouBodies) else {
        panic!("bodies");
    };
    *body = code;
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("index");
    };
    index.entries[0].code_length = length;
    let Some(SectionData::ConstPool(pool)) = module.section_mut(SectionId::ConstPool) else {
        panic!("constants");
    };
    pool.entries.push(ConstEntry {
        type_id: 0,
        payload: vec![1],
    });
    module
}

fn load_bool(code: &mut Vec<u8>) {
    code.push(0x10);
    code.extend_from_slice(&0u32.to_le_bytes());
}
fn jump(code: &mut Vec<u8>, opcode: u8, offset: i32) {
    code.push(opcode);
    code.extend_from_slice(&offset.to_le_bytes());
}

// CALL_NATIVE with function kind, symbol 0 ("R" in the base string table) and arg_count.
fn call_native(code: &mut Vec<u8>, arg_count: u32) {
    code.push(0x09);
    code.extend_from_slice(&NATIVE_CALL_KIND_FUNCTION.to_le_bytes());
    code.extend_from_slice(&0u32.to_le_bytes());
    code.extend_from_slice(&arg_count.to_le_bytes());
}

// Accepts only an InvalidSection whose rendered text is exactly `expected`.
fn assert_refused_as_invalid_section<T: std::fmt::Debug>(
    result: Result<T, BytecodeError>,
    expected: &str,
) {
    match result {
        Err(BytecodeError::InvalidSection(ref message)) if message == expected => {}
        other => panic!("expected InvalidSection({expected:?}), got {other:?}"),
    }
}

#[test]
fn deep_linear_body_does_not_retain_a_stack_per_instruction() {
    let depth = trust_runtime_core::vm::VM_MAX_OPERAND_STACK;
    let mut code = Vec::new();
    load_bool(&mut code);
    code.extend(std::iter::repeat_n(0x11, depth - 1));
    code.extend(std::iter::repeat_n(
        0x00,
        BYTECODE_MAX_INSTRUCTIONS - 2 * depth - 1,
    ));
    code.extend(std::iter::repeat_n(0x12, depth));
    code.push(0x06);
    let module = module_with_code(code);
    // Run the memory assertion in a fresh process; a previous test's high-water
    // mark must not conceal this validator's allocations.
    #[cfg(target_os = "linux")]
    if std::env::var_os("TRUST_VALIDATOR_MEMORY_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "deep_linear_body_does_not_retain_a_stack_per_instruction",
                "--nocapture",
            ])
            .env("TRUST_VALIDATOR_MEMORY_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let validated = module
        .validated()
        .expect("one million instructions with bounded linear analysis");
    assert!(validated.stats().peak_scratch_bytes <= 64 * 1024 * 1024);
    assert!(validated.stats().work <= ValidationLimits::default().max_work);
    #[cfg(target_os = "linux")]
    assert!(
        peak_resident_kib() < 128 * 1024,
        "allocator/RSS evidence is separate from accounted analysis storage"
    );
    let error = module
        .validated_with_limits(ValidationLimits {
            max_scratch_bytes: 1024 * 1024,
            ..ValidationLimits::default()
        })
        .unwrap_err();
    assert_eq!(
        error,
        BytecodeError::from(RejectionReason::ValidationStorageLimit)
    );
}

#[cfg(target_os = "linux")]
fn peak_resident_kib() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .expect("Linux process accounting")
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")
                .and_then(|value| value.split_whitespace().next())
                .map(|value| value.parse().expect("KiB count"))
        })
        .expect("Linux VmHWM field")
}

#[test]
fn many_deep_branch_entries_exhaust_the_aggregate_budget() {
    let mut code = Vec::new();
    load_bool(&mut code);
    code.extend(std::iter::repeat_n(0x11, 31));
    for _ in 0..1024 {
        code.push(0x11);
        jump(&mut code, 0x03, 0);
    }
    code.extend(std::iter::repeat_n(0x12, 32));
    code.push(0x06);
    let module = module_with_code(code);
    let stats = module
        .validated()
        .expect("bounded hosted defaults admit this fixture")
        .stats();
    let error = module
        .validated_with_limits(ValidationLimits {
            max_scratch_bytes: stats.peak_scratch_bytes - 1,
            ..ValidationLimits::default()
        })
        .unwrap_err();
    assert_eq!(
        error,
        BytecodeError::from(RejectionReason::ValidationStorageLimit)
    );
}

#[test]
fn exact_accounted_limits_pass_and_one_less_fails() {
    let module = module_with_code(vec![0x00, 0x06]);
    let stats = module.validated().unwrap().stats();
    module
        .validated_with_limits(ValidationLimits {
            max_scratch_bytes: stats.peak_scratch_bytes,
            max_work: stats.work,
        })
        .unwrap();
    assert_eq!(
        module
            .validated_with_limits(ValidationLimits {
                max_scratch_bytes: stats.peak_scratch_bytes - 1,
                max_work: stats.work
            })
            .unwrap_err(),
        BytecodeError::from(RejectionReason::ValidationStorageLimit)
    );
    assert_eq!(
        module
            .validated_with_limits(ValidationLimits {
                max_scratch_bytes: stats.peak_scratch_bytes,
                max_work: stats.work - 1
            })
            .unwrap_err(),
        BytecodeError::from(RejectionReason::ValidationWorkLimit)
    );
}

#[test]
fn unchanged_loop_terminates_analysis_and_growing_backedge_rejects() {
    let mut code = Vec::new();
    jump(&mut code, 0x02, -5);
    module_with_code(code)
        .validated()
        .expect("unchanged entry state reaches a fixed point");
    let mut code = Vec::new();
    load_bool(&mut code);
    jump(&mut code, 0x02, -10);
    assert_eq!(
        module_with_code(code).validated().unwrap_err(),
        BytecodeError::from(RejectionReason::InconsistentOperandStackDepthAtControlFlowMerge)
    );
}

#[test]
fn unreachable_malformed_instruction_is_still_decoded() {
    let mut code = Vec::new();
    jump(&mut code, 0x02, 1);
    code.push(0xff);
    assert_eq!(
        module_with_code(code).validated().unwrap_err(),
        BytecodeError::InvalidOpcode(0xff)
    );
}

#[test]
fn multiple_pous_share_one_work_budget() {
    let mut code = vec![0x00; 1024];
    code.push(0x06);
    let mut module = module_with_code(code);
    let first = module.validated().unwrap().stats();
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("index");
    };
    let mut second = index.entries[0].clone();
    second.id = 2;
    index.entries.push(second);
    assert_eq!(
        module
            .validated_with_limits(ValidationLimits {
                max_work: first.work + 1024,
                ..ValidationLimits::default()
            })
            .unwrap_err(),
        BytecodeError::from(RejectionReason::ValidationWorkLimit)
    );
}

#[test]
fn widening_loop_reaches_a_fixed_point() {
    use trust_runtime_core::bytecode::{RefEntry, RefLocation};
    let mut code = Vec::new();
    load_bool(&mut code); // 0..5
    code.push(0x11); // leader at 5
    jump(&mut code, 0x03, 11); // 6..11 -> exit at 22
    code.push(0x12); // remove known BOOL
    code.push(0x20);
    code.extend_from_slice(&0u32.to_le_bytes()); // unknown reference value
    jump(&mut code, 0x02, -17); // 17..22 -> leader at 5
    code.extend_from_slice(&[0x12, 0x06]);
    let mut module = module_with_code(code);
    let Some(SectionData::RefTable(refs)) = module.section_mut(SectionId::RefTable) else {
        panic!("refs");
    };
    refs.entries.push(RefEntry {
        location: RefLocation::Global,
        owner_id: 0,
        offset: 0,
        segments: vec![],
    });
    let validated = module
        .validated()
        .expect("Bool merges monotonically to Unknown");
    assert!(validated.stats().work < 4096);
}

#[test]
fn conditional_errors_keep_fallthrough_first_lifo_order() {
    let mut code = Vec::new();
    load_bool(&mut code);
    jump(&mut code, 0x03, 13); // target is the other invalid path at 23
    load_bool(&mut code);
    load_bool(&mut code);
    code.extend_from_slice(&[0x40, 0x12, 0x06]); // BOOL arithmetic on fallthrough
    load_bool(&mut code);
    code.push(0x06); // nonempty stack on target
    assert_eq!(
        module_with_code(code).validated().unwrap_err(),
        BytecodeError::from(RejectionReason::ArithmeticOpcodeExpectsNumericOperands)
    );
}

#[test]
fn branch_diamonds_do_not_confuse_linear_analysis_with_runtime_depth() {
    let mut code = Vec::new();
    // Each real path has at most one value; the conservative linear pass sees
    // both arms and accumulates one extra value per diamond.
    for _ in 0..=trust_runtime_core::vm::VM_MAX_OPERAND_STACK {
        load_bool(&mut code);
        jump(&mut code, 0x03, 10);
        load_bool(&mut code);
        jump(&mut code, 0x02, 5);
        load_bool(&mut code);
        code.push(0x12);
    }
    code.push(0x06);
    module_with_code(code)
        .validated()
        .expect("runtime depth is a control-flow property");
}

#[test]
fn legacy_call_keeps_structural_error_precedence() {
    let mut module = module_with_code(vec![0x05, 0, 0, 0, 0, 0xff]);
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("index");
    };
    index.entries[0].id = 0;
    assert_eq!(
        module.validated().unwrap_err(),
        BytecodeError::InvalidOpcode(0xff)
    );
    let Some(SectionData::PouBodies(body)) = module.section_mut(SectionId::PouBodies) else {
        panic!("body");
    };
    body[5] = 0x06;
    assert_eq!(
        module.validated().unwrap_err(),
        BytecodeError::from(RejectionReason::LegacyCall)
    );
}

#[test]
fn struct_built_type_discriminant_must_match_its_payload() {
    use trust_runtime_core::bytecode::TypeKind;
    let mut module = module_with_code(vec![0x06]);
    let Some(SectionData::TypeTable(types)) = module.section_mut(SectionId::TypeTable) else {
        panic!("types");
    };
    types.entries[0].kind = TypeKind::Class;
    assert_eq!(
        module.validated().unwrap_err(),
        BytecodeError::from(RejectionReason::TypePayloadMismatch)
    );
}

#[test]
fn nested_type_metadata_consumes_the_shared_work_budget() {
    use trust_runtime_core::bytecode::{Field, TypeData, TypeEntry, TypeKind};
    let mut module = module_with_code(vec![0x06]);
    let baseline = module.validated().unwrap().stats().work;
    let Some(SectionData::TypeTable(types)) = module.section_mut(SectionId::TypeTable) else {
        panic!("types");
    };
    types.entries.push(TypeEntry {
        kind: TypeKind::Struct,
        name_idx: None,
        data: TypeData::Struct {
            fields: vec![
                Field {
                    name_idx: 0,
                    type_id: 0,
                    bit_offset: None,
                };
                4096
            ],
        },
    });
    assert_eq!(
        module
            .validated_with_limits(ValidationLimits {
                max_work: baseline + 1024,
                ..ValidationLimits::default()
            })
            .unwrap_err(),
        BytecodeError::from(RejectionReason::ValidationWorkLimit)
    );
    module
        .validated()
        .expect("nested metadata fits normal hosted limits");
}

#[test]
fn call_native_argument_boundary_is_exact_through_module_validation() {
    // 1,024 BOOL values are pushed; the call pops all of them and its result is discarded.
    let mut admitted = Vec::new();
    load_bool(&mut admitted);
    admitted.extend(std::iter::repeat_n(0x11, 1023));
    call_native(&mut admitted, 1024);
    admitted.extend_from_slice(&[0x12, 0x06]);
    module_with_code(admitted)
        .validated()
        .expect("1,024 native-call arguments are admitted");

    // Site: validate/pou_and_instr.rs:214-218. Operand decoding refuses 1,025 before
    // any stack shape is analysed.
    let mut refused = Vec::new();
    call_native(&mut refused, 1025);
    refused.push(0x06);
    assert_eq!(
        module_with_code(refused).validated().unwrap_err(),
        BytecodeError::from(RejectionReason::CallNativeArgCountOutOfRange)
    );
}

#[test]
fn decode_charge_refuses_instruction_1000001_before_its_jump_is_checked() {
    // Site: validate/instructions.rs:66. 1,000,000 NOPs are admitted. The next
    // instruction is a JUMP far outside the body; the decode charge refuses it before
    // its opcode is read or its target is checked.
    let mut code = vec![0x00; BYTECODE_MAX_INSTRUCTIONS];
    jump(&mut code, 0x02, i32::MAX);
    assert_refused_as_invalid_section(
        module_with_code(code).validated(),
        "decoded module instructions exceed fixed resource limit",
    );
}

#[test]
fn reference_table_overflow_is_the_first_declared_resource_fault() {
    use trust_runtime_core::bytecode::{RefEntry, RefLocation};
    let mut module = base_module();
    let Some(SectionData::RefTable(refs)) = module.section_mut(SectionId::RefTable) else {
        panic!("refs");
    };
    let entry = RefEntry {
        location: RefLocation::Global,
        owner_id: 0,
        offset: 0,
        segments: vec![],
    };
    refs.entries = vec![entry; BYTECODE_MAX_REFERENCES + 1];
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("index");
    };
    // Sites: validate/module_validate.rs:91 calls validate/resource_limits.rs:12-16,
    // which checks REF_TABLE before POU locals. The POU's local count also exceeds its
    // limit, so the REF_TABLE check must be the one reported.
    index.entries[0].local_ref_count = BYTECODE_MAX_LOCALS_PER_POU as u32 + 1;
    assert_refused_as_invalid_section(
        module.validated(),
        "REF_TABLE entries exceed fixed resource limit",
    );
}

#[test]
fn pou_local_overflow_is_reported_when_the_reference_table_fits() {
    let mut module = base_module();
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("index");
    };
    index.entries[0].local_ref_count = BYTECODE_MAX_LOCALS_PER_POU as u32 + 1;
    assert_refused_as_invalid_section(
        module.validated(),
        "POU local references exceed fixed resource limit",
    );
}

#[test]
fn decoded_instruction_limit_precedes_the_operand_stack_limit_in_one_body() {
    // Sites: validate/instructions.rs:66 (decode charge) runs for the whole body before
    // the DUP push in validate/stack_shape.rs:214-219, whose depth check is
    // validate/budget.rs:82. The 16,384th DUP would reach 16,385 stack values, but the
    // body's 1,000,001st instruction is refused while decoding.
    let depth = trust_runtime_core::vm::VM_MAX_OPERAND_STACK;
    let mut code = Vec::new();
    load_bool(&mut code);
    code.extend(std::iter::repeat_n(0x11, depth));
    code.extend(std::iter::repeat_n(0x00, BYTECODE_MAX_INSTRUCTIONS - depth));
    assert_refused_as_invalid_section(
        module_with_code(code).validated(),
        "decoded module instructions exceed fixed resource limit",
    );
}

#[test]
fn operand_stack_limit_refuses_the_value_above_the_fixed_depth() {
    // Site: validate/budget.rs:82 via the DUP push in validate/stack_shape.rs:214-219.
    // One PUSH_CONST and `depth` DUPs leave depth + 1 values; the last DUP is refused.
    let depth = trust_runtime_core::vm::VM_MAX_OPERAND_STACK;
    let mut code = Vec::new();
    load_bool(&mut code);
    code.extend(std::iter::repeat_n(0x11, depth));
    code.push(0x06);
    assert_refused_as_invalid_section(
        module_with_code(code).validated(),
        "operand stack values exceed fixed resource limit",
    );
}

#[test]
fn operand_stack_limit_admits_exactly_the_fixed_depth() {
    // One PUSH_CONST and depth - 1 DUPs leave exactly depth values, all popped again.
    let depth = trust_runtime_core::vm::VM_MAX_OPERAND_STACK;
    let mut code = Vec::new();
    load_bool(&mut code);
    code.extend(std::iter::repeat_n(0x11, depth - 1));
    code.extend(std::iter::repeat_n(0x12, depth));
    code.push(0x06);
    module_with_code(code)
        .validated()
        .expect("exactly the operand-stack limit is admitted");
}
