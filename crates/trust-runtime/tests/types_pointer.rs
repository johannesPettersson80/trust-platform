use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::{bytecode_module_from_source, TestHarness};
use trust_runtime::{
    bytecode::{SectionData, SectionId},
    value::{ArrayValue, Value},
};

fn vm_harness(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("compile harness");
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

#[test]
fn pointer_types_support_adr_deref_index_and_null_in_runtime_and_vm() {
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

    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings,
        other => panic!("expected STRING_TABLE, got {other:?}"),
    };
    let index = match module.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => index,
        other => panic!("expected POU_INDEX, got {other:?}"),
    };
    let main = index
        .entries
        .iter()
        .find(|entry| strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("MAIN"))
        .expect("main entry");
    let bodies = match module.section(SectionId::PouBodies) {
        Some(SectionData::PouBodies(bodies)) => bodies,
        other => panic!("expected POU_BODIES, got {other:?}"),
    };
    let body = &bodies[main.code_offset as usize..(main.code_offset + main.code_length) as usize];
    assert!(
        body.contains(&0x22),
        "expected LOAD_REF_ADDR opcode in main body"
    );
    assert!(
        body.contains(&0x32),
        "expected LOAD_DYNAMIC opcode in main body"
    );
    assert!(body.contains(&0x33), "expected STORE opcode in main body");

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

    let mut vm = vm_harness(source);
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
fn pointer_to_string_supports_indexed_deref_read_and_write_in_runtime_and_vm() {
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

    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings,
        other => panic!("expected STRING_TABLE, got {other:?}"),
    };
    let index = match module.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => index,
        other => panic!("expected POU_INDEX, got {other:?}"),
    };
    let main = index
        .entries
        .iter()
        .find(|entry| strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("MAIN"))
        .expect("main entry");
    let bodies = match module.section(SectionId::PouBodies) {
        Some(SectionData::PouBodies(bodies)) => bodies,
        other => panic!("expected POU_BODIES, got {other:?}"),
    };
    let body = &bodies[main.code_offset as usize..(main.code_offset + main.code_length) as usize];
    assert!(
        body.contains(&0x22),
        "expected LOAD_REF_ADDR opcode in main body"
    );
    assert!(
        body.contains(&0x31),
        "expected INDEX_REF opcode in main body"
    );
    assert!(body.contains(&0x33), "expected STORE opcode in main body");

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

    let mut vm = vm_harness(source);
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
