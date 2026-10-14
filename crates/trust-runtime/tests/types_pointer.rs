use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::{
    bytecode::{SectionData, SectionId},
    value::{ArrayValue, Value},
};
use trust_runtime_core::vm::register_test_support::opcode_operand_len;

fn legacy_vm_harness(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let session = CompileSession::from_source(source);
    let legacy = session
        .build_bytecode_module_for_version(trust_runtime::bytecode::BytecodeVersion::LEGACY)
        .expect("produce explicit legacy bytecode");
    *harness.runtime_mut() = session
        .build_runtime_from_bytecode(&legacy, None)
        .expect("admit legacy bytecode with matching construction context");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");
    harness
}

// Inspect instruction boundaries; operands can contain any opcode byte.
fn assert_pointer_instructions(source: &str, required: &[u8]) {
    use trust_runtime::bytecode::BytecodeVersion;
    for version in [
        BytecodeVersion::LEGACY,
        BytecodeVersion::SOURCE_FREE_V2,
        BytecodeVersion::SOURCE_FREE,
    ] {
        let module = CompileSession::from_source(source)
            .build_bytecode_module_for_version(version)
            .expect("compile bytecode module");
        let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
            panic!("missing string table");
        };
        let Some(SectionData::PouIndex(index)) = module.section(SectionId::PouIndex) else {
            panic!("missing POU index");
        };
        let main = index
            .entries
            .iter()
            .find(|entry| strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("MAIN"))
            .expect("Main POU");
        let Some(SectionData::PouBodies(bodies)) = module.section(SectionId::PouBodies) else {
            panic!("missing POU bodies");
        };
        let body =
            &bodies[main.code_offset as usize..(main.code_offset + main.code_length) as usize];
        let mut opcodes = Vec::new();
        let mut pc = 0;
        while pc < body.len() {
            let opcode = body[pc];
            let width = opcode_operand_len(opcode).expect("known producer opcode");
            assert!(pc + 1 + width <= body.len());
            opcodes.push(opcode);
            pc += 1 + width;
        }
        if version == BytecodeVersion::LEGACY {
            assert!(opcodes.contains(&0x22), "legacy LOAD_REF_ADDR missing");
        } else {
            assert!(opcodes.contains(&0x23), "source-free LOAD_SELF missing");
            assert!(opcodes.contains(&0x30), "source-free FIELD_REF missing");
        }
        for opcode in required {
            assert!(
                opcodes.contains(opcode),
                "missing {opcode:02X} in {version:?}"
            );
        }
    }
}

#[test]
fn pointer_types_support_adr_deref_index_and_null_in_source_free_and_contextual_legacy() {
    let source = r#"
        PROGRAM Main
        VAR
            x : INT := INT#5;
            arr : ARRAY[1..3] OF INT;
            p_int : POINTER TO INT;
            p_arr : POINTER TO ARRAY[1..3] OF INT;
            out_x : INT := INT#0;
            out_arr : INT := INT#0;
            was_null_before : BOOL := FALSE;
            was_null_after : BOOL := FALSE;
        END_VAR
        arr[1] := INT#11;
        arr[2] := INT#22;
        arr[3] := INT#33;
        IF p_int = NULL THEN
            was_null_before := TRUE;
        END_IF;
        p_int := ADR(x);
        p_arr := ADR(arr);
        p_int^ := INT#9;
        p_arr^[2] := INT#44;
        out_x := p_int^;
        out_arr := p_arr^[2];
        p_arr := NULL;
        p_int ?= NULL;
        IF p_int = NULL THEN
            was_null_after := TRUE;
        END_IF;
        END_PROGRAM
    "#;

    assert_pointer_instructions(source, &[0x32, 0x33]);

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    assert!(
        matches!(harness.get_output("p_int"), Some(Value::Reference(None))),
        "expected p_int to start NULL_REF, got {:?}",
        harness.get_output("p_int")
    );
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime execution failed: {:?}",
        cycle.errors
    );
    assert!(
        matches!(harness.get_output("p_int"), Some(Value::Reference(None))),
        "expected p_int to end as NULL_REF, got {:?}",
        harness.get_output("p_int")
    );
    assert!(
        matches!(harness.get_output("p_arr"), Some(Value::Reference(None))),
        "expected p_arr to end as NULL_REF, got {:?}",
        harness.get_output("p_arr")
    );
    harness.assert_eq("x", 9i16);
    harness.assert_eq(
        "arr",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(
                vec![11i16.into(), 44i16.into(), 33i16.into()],
                vec![(1, 3)],
            )
            .expect("valid expected array"),
        )),
    );
    harness.assert_eq("out_x", 9i16);
    harness.assert_eq("out_arr", 44i16);
    harness.assert_eq("was_null_before", true);
    harness.assert_eq("was_null_after", true);

    let mut vm = legacy_vm_harness(source);
    assert!(
        matches!(vm.get_output("p_int"), Some(Value::Reference(None))),
        "expected vm p_int to start NULL_REF, got {:?}",
        vm.get_output("p_int")
    );
    let cycle = vm.cycle();
    assert!(
        cycle.errors.is_empty(),
        "vm execution failed: {:?}",
        cycle.errors
    );
    assert!(
        matches!(vm.get_output("p_int"), Some(Value::Reference(None))),
        "expected p_int to end as NULL_REF, got {:?}",
        vm.get_output("p_int")
    );
    assert!(
        matches!(vm.get_output("p_arr"), Some(Value::Reference(None))),
        "expected p_arr to end as NULL_REF, got {:?}",
        vm.get_output("p_arr")
    );
    vm.assert_eq("x", 9i16);
    vm.assert_eq(
        "arr",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(
                vec![11i16.into(), 44i16.into(), 33i16.into()],
                vec![(1, 3)],
            )
            .expect("valid expected array"),
        )),
    );
    vm.assert_eq("out_x", 9i16);
    vm.assert_eq("out_arr", 44i16);
    vm.assert_eq("was_null_before", true);
    vm.assert_eq("was_null_after", true);
}

#[test]
fn pointer_to_string_supports_indexed_deref_read_and_write_in_source_free_and_contextual_legacy() {
    let source = r#"
        TYPE Text5 : STRING[5];
        END_TYPE

        PROGRAM Main
        VAR
            text : Text5 := 'ABCD';
            p_text : POINTER TO Text5;
            first_char : CHAR;
            second_char : CHAR;
            out_text : Text5 := '';
        END_VAR

        p_text := ADR(text);
        first_char := p_text^[1];
        p_text^[2] := BYTE_TO_CHAR(BYTE#90);
        second_char := p_text^[2];
        out_text := text;
        END_PROGRAM
    "#;

    assert_pointer_instructions(source, &[0x31, 0x33]);

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("first_char", Value::Char(b'A'));
    harness.assert_eq("second_char", Value::Char(b'Z'));
    harness.assert_eq("out_text", Value::String("AZCD".into()));

    let mut vm = legacy_vm_harness(source);
    let cycle = vm.cycle();
    assert!(
        cycle.errors.is_empty(),
        "vm execution failed: {:?}",
        cycle.errors
    );
    vm.assert_eq("first_char", Value::Char(b'A'));
    vm.assert_eq("second_char", Value::Char(b'Z'));
    vm.assert_eq("out_text", Value::String("AZCD".into()));
}
