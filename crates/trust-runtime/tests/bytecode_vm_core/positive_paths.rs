#[test]
fn vm_executes_program_with_stack_and_pc_progression() {
    let source = r#"
        PROGRAM Main
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;
    let mut harness = vm_harness(source);
    harness.assert_eq("count", 0i32);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected VM cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("count", 1i32);
}

#[test]
fn vm_opcode_positive_path_covers_arith_logical_branch_jump_load_store_ref() {
    let source = r#"
        PROGRAM Main
        VAR
            i: DINT := 0;
            acc: DINT := 0;
            gate: BOOL := FALSE;
        END_VAR
        WHILE i < 4 DO
            gate := (i < 2) AND TRUE;
            IF gate THEN
                acc := acc + i;
            END_IF;
            i := i + 1;
        END_WHILE;
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(body.contains(&0x02), "expected JUMP opcode in main body");
    assert!(
        body.contains(&0x03) || body.contains(&0x04),
        "expected JUMP_IF_TRUE/FALSE opcode in main body"
    );
    assert!(
        body.contains(&0x20),
        "expected LOAD_REF opcode in main body"
    );
    assert!(
        body.contains(&0x21),
        "expected STORE_REF opcode in main body"
    );
    assert!(body.contains(&0x40), "expected ADD opcode in main body");
    assert!(body.contains(&0x46), "expected AND opcode in main body");

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "opcode positive-path execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("i", 4i32);
    harness.assert_eq("acc", 1i32);
}

#[test]
fn vm_opcode_positive_path_covers_call_native_stdlib_dispatch() {
    let source = r#"
        PROGRAM Main
        VAR
            out_sel : INT := 0;
        END_VAR
        out_sel := SEL(G := TRUE, IN0 := INT#4, IN1 := INT#7);
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x09),
        "expected CALL_NATIVE opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "CALL_NATIVE stdlib dispatch failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_sel", 7i16);
}

#[test]
fn vm_call_native_builtin_function_block_executes_body_and_copies_outputs() {
    let source = r#"
        PROGRAM Main
        VAR
            counter : CTU;
            pulse : BOOL := TRUE;
            reset : BOOL := FALSE;
            preset : INT := INT#2;
            reached : BOOL := FALSE;
            count : INT := INT#-1;
        END_VAR
        counter(CU := pulse, R := reset, PV := preset, Q => reached, CV => count);
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x09),
        "expected CALL_NATIVE opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "CALL_NATIVE builtin function-block dispatch failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("count", 1i16);
    harness.assert_eq("reached", false);
}

#[test]
fn vm_opcode_positive_path_covers_call_native_oop_dispatch() {
    let source = r#"
        INTERFACE ICounter
        METHOD Inc : INT
        VAR_INPUT
            delta : INT;
        END_VAR
        END_METHOD
        END_INTERFACE

        CLASS Counter IMPLEMENTS ICounter
        VAR PUBLIC
            value : INT := INT#0;
        END_VAR
        METHOD PUBLIC Inc : INT
        VAR_INPUT
            delta : INT;
        END_VAR
        value := value + delta;
        Inc := value;
        END_METHOD
        END_CLASS

        FUNCTION_BLOCK ThisCounter
        VAR
            count : INT := INT#5;
        END_VAR
        VAR_OUTPUT
            value : INT;
        END_VAR
        value := THIS.count;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK BaseFb
        VAR PUBLIC
            count : INT := INT#10;
        END_VAR
        METHOD PUBLIC GetCount : INT
        GetCount := count;
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK DerivedFb EXTENDS BaseFb
        VAR PUBLIC
            extra : INT := INT#3;
        END_VAR
        METHOD PUBLIC GetCount : INT
        GetCount := count + extra;
        END_METHOD
        METHOD PUBLIC GetSuper : INT
        GetSuper := SUPER.GetCount();
        END_METHOD
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            i : ICounter;
            c : Counter;
            fb_this : ThisCounter;
            fb_derived : DerivedFb;
            out_this : INT := INT#0;
            out_override : INT := INT#0;
            out_super : INT := INT#0;
            out_iface : INT := INT#0;
            out_direct : INT := INT#0;
        END_VAR
        i := c;
        fb_this(value => out_this);
        out_override := fb_derived.GetCount();
        out_super := fb_derived.GetSuper();
        out_iface := i.Inc(INT#1);
        out_direct := c.Inc(INT#2);
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x09),
        "expected CALL_NATIVE opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "CALL_NATIVE OOP dispatch failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_this", 5i16);
    harness.assert_eq("out_override", 13i16);
    harness.assert_eq("out_super", 10i16);
    harness.assert_eq("out_iface", 1i16);
    harness.assert_eq("out_direct", 3i16);
}

#[test]
fn vm_call_native_method_polymorphic_receiver_dispatch_remains_correct() {
    let source = r#"
        INTERFACE ICounter
        METHOD Inc : INT
        VAR_INPUT
            delta : INT;
        END_VAR
        END_METHOD
        END_INTERFACE

        CLASS CounterA IMPLEMENTS ICounter
        VAR PUBLIC
            value : INT := INT#0;
        END_VAR
        METHOD PUBLIC Inc : INT
        VAR_INPUT
            delta : INT;
        END_VAR
        value := value + delta;
        Inc := value;
        END_METHOD
        END_CLASS

        CLASS CounterB IMPLEMENTS ICounter
        VAR PUBLIC
            value : INT := INT#0;
        END_VAR
        METHOD PUBLIC Inc : INT
        VAR_INPUT
            delta : INT;
        END_VAR
        value := value + (delta * INT#10);
        Inc := value;
        END_METHOD
        END_CLASS

        PROGRAM Main
        VAR
            i : ICounter;
            a : CounterA;
            b : CounterB;
            out_a1 : INT := INT#0;
            out_b1 : INT := INT#0;
            out_a2 : INT := INT#0;
        END_VAR
        i := a;
        out_a1 := i.Inc(INT#1);
        i := b;
        out_b1 := i.Inc(INT#2);
        i := a;
        out_a2 := i.Inc(INT#3);
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x09),
        "expected CALL_NATIVE opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "CALL_NATIVE polymorphic method dispatch failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_a1", 1i16);
    harness.assert_eq("out_b1", 20i16);
    harness.assert_eq("out_a2", 4i16);
}

#[test]
fn vm_call_native_direct_binding_preserves_named_default_out_and_inout_contracts() {
    let source = r#"
        FUNCTION MixFn : INT
        VAR_INPUT
            a : INT;
            b : INT := INT#5;
        END_VAR
        VAR_OUTPUT
            out_sum : INT;
        END_VAR
        VAR_IN_OUT
            acc : INT;
        END_VAR
        out_sum := a + b;
        acc := acc + out_sum;
        MixFn := acc;
        END_FUNCTION

        FUNCTION_BLOCK MixFb
        VAR_INPUT
            in_a : INT;
            in_b : INT := INT#4;
        END_VAR
        VAR_OUTPUT
            out_sum : INT;
        END_VAR
        VAR_IN_OUT
            acc : INT;
        END_VAR
        out_sum := in_a + in_b;
        acc := acc + out_sum;
        END_FUNCTION_BLOCK

        CLASS MixClass
        METHOD PUBLIC Apply : INT
        VAR_INPUT
            a : INT;
            b : INT := INT#6;
        END_VAR
        VAR_OUTPUT
            out_sum : INT;
        END_VAR
        VAR_IN_OUT
            acc : INT;
        END_VAR
        out_sum := a + b;
        acc := acc + out_sum;
        Apply := acc;
        END_METHOD
        END_CLASS

        PROGRAM Main
        VAR
            fb : MixFb;
            obj : MixClass;
            total_fn : INT := INT#10;
            total_fb : INT := INT#20;
            total_method : INT := INT#30;
            out_fn : INT := INT#0;
            out_fb : INT := INT#0;
            out_method : INT := INT#0;
            result_fn : INT := INT#0;
            result_method : INT := INT#0;
        END_VAR
        result_fn := MixFn(a := INT#2, out_sum => out_fn, acc := total_fn);
        fb(in_a := INT#3, out_sum => out_fb, acc := total_fb);
        result_method := obj.Apply(a := INT#4, out_sum => out_method, acc := total_method);
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x09),
        "expected CALL_NATIVE opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "CALL_NATIVE direct binding parity failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_fn", 7i16);
    harness.assert_eq("total_fn", 17i16);
    harness.assert_eq("result_fn", 17i16);
    harness.assert_eq("out_fb", 7i16);
    harness.assert_eq("total_fb", 27i16);
    harness.assert_eq("out_method", 10i16);
    harness.assert_eq("total_method", 40i16);
    harness.assert_eq("result_method", 40i16);
}

#[test]
fn vm_call_native_direct_binding_module_swap_reloads_default_metadata() {
    let source_v1 = r#"
        FUNCTION AddDefault : INT
        VAR_INPUT
            a : INT;
            b : INT := INT#5;
        END_VAR
        AddDefault := a + b;
        END_FUNCTION

        PROGRAM Main
        VAR
            result : INT := INT#0;
        END_VAR
        result := AddDefault(a := INT#2);
        END_PROGRAM
    "#;
    let source_v2 = r#"
        FUNCTION AddDefault : INT
        VAR_INPUT
            a : INT;
            b : INT := INT#50;
        END_VAR
        AddDefault := a + b;
        END_FUNCTION

        PROGRAM Main
        VAR
            result : INT := INT#0;
        END_VAR
        result := AddDefault(a := INT#2);
        END_PROGRAM
    "#;
    let bytes_v1 = bytecode_bytes_from_source(source_v1).expect("build bytecode v1");
    let bytes_v2 = bytecode_bytes_from_source(source_v2).expect("build bytecode v2");

    let mut harness = TestHarness::from_source(source_v1).expect("compile runtime");
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes_v1, None)
        .expect("apply bytecode v1");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime v1");
    let first = harness.cycle();
    assert!(
        first.errors.is_empty(),
        "CALL_NATIVE v1 failed: {:?}",
        first.errors
    );
    harness.assert_eq("result", 7i16);

    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes_v2, None)
        .expect("apply bytecode v2");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime v2");
    let second = harness.cycle();
    assert!(
        second.errors.is_empty(),
        "CALL_NATIVE v2 failed after module swap: {:?}",
        second.errors
    );
    harness.assert_eq("result", 52i16);
}

#[test]
fn vm_opcode_positive_path_covers_string_and_wstring_literals() {
    let source = r#"
        PROGRAM Main
        VAR
            s : STRING := '';
            ws : WSTRING := "";
            str_eq : BOOL := FALSE;
            wstr_lt : BOOL := FALSE;
        END_VAR
        s := 'AB';
        ws := "CD";
        str_eq := s = 'AB';
        wstr_lt := ws < "CE";
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x10),
        "expected LOAD_CONST opcode for string/wstring literals"
    );
    assert!(body.contains(&0x50), "expected EQ opcode in main body");
    assert!(body.contains(&0x52), "expected LT opcode in main body");

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "string/wstring literal execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("s", Value::String("AB".into()));
    harness.assert_eq("ws", Value::WString("CD".to_string()));
    harness.assert_eq("str_eq", true);
    harness.assert_eq("wstr_lt", true);
}

#[test]
fn vm_opcode_positive_path_covers_dynamic_reference_and_nested_chains() {
    let source = r#"
        TYPE
            Inner : STRUCT
                arr : ARRAY[0..2] OF INT;
            END_STRUCT;
            Outer : STRUCT
                inner : Inner;
            END_STRUCT;
        END_TYPE

        PROGRAM Main
        VAR
            o : Outer;
            idx : INT := INT#1;
            value_cell : INT := INT#4;
            r : REF_TO INT;
            out_ref : INT := INT#0;
            out_nested : INT := INT#0;
        END_VAR
        r := REF(value_cell);
        r^ := r^ + INT#5;
        out_ref := r^;
        out_nested := REF(o)^.inner.arr[idx];
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x30),
        "expected REF_FIELD opcode in main body"
    );
    assert!(
        body.contains(&0x31),
        "expected REF_INDEX opcode in main body"
    );
    assert!(
        body.contains(&0x32),
        "expected LOAD_DYNAMIC opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "dynamic reference opcode execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_ref", 9i16);
    harness.assert_eq("out_nested", 0i16);
}

#[test]
fn vm_opcode_positive_path_covers_nested_field_chain_assignments() {
    let source = r#"
        TYPE
            Inner : STRUCT
                value : INT;
            END_STRUCT;
            Outer : STRUCT
                inner : Inner;
            END_STRUCT;
        END_TYPE

        PROGRAM Main
        VAR
            outer : Outer;
            out_value : INT := INT#0;
        END_VAR
        outer.inner.value := INT#9;
        out_value := outer.inner.value;
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x30),
        "expected REF_FIELD opcode in main body"
    );
    assert!(
        body.contains(&0x32),
        "expected LOAD_DYNAMIC opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "nested field-chain assignment execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_value", 9i16);
}

#[test]
fn vm_opcode_positive_path_covers_nested_field_index_assignments() {
    let source = r#"
        TYPE
            Item : STRUCT
                value : INT;
            END_STRUCT;
            Outer : STRUCT
                arr : ARRAY[0..2] OF INT;
            END_STRUCT;
        END_TYPE

        PROGRAM Main
        VAR
            outer : Outer;
            items : ARRAY[0..1] OF Item;
            idx : INT := INT#1;
            out_field_index : INT := INT#0;
            out_index_field : INT := INT#0;
        END_VAR
        outer.arr[idx] := INT#9;
        items[idx].value := INT#7;
        out_field_index := outer.arr[idx];
        out_index_field := items[idx].value;
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x30),
        "expected REF_FIELD opcode in main body"
    );
    assert!(
        body.contains(&0x31),
        "expected REF_INDEX opcode in main body"
    );
    assert!(
        body.contains(&0x32),
        "expected LOAD_DYNAMIC opcode in main body"
    );
    assert!(
        body.contains(&0x33),
        "expected STORE_DYNAMIC opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "nested field/index assignment execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_field_index", 9i16);
    harness.assert_eq("out_index_field", 7i16);
}

#[test]
fn vm_opcode_positive_path_covers_string_and_wstring_index_reads() {
    let source = r#"
        PROGRAM Main
        VAR
            idx : INT := INT#2;
            text_value : STRING[8] := 'ABCD';
            wide_value : WSTRING[8] := "WXYZ";
            out_char : CHAR;
            out_wchar : WCHAR;
        END_VAR

        out_char := text_value[idx];
        out_wchar := wide_value[idx + INT#1];
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x31),
        "expected REF_INDEX opcode in main body"
    );
    assert!(
        body.contains(&0x32),
        "expected LOAD_DYNAMIC opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "string index execution failed: {:?}",
        cycle.errors
    );
    assert_eq!(harness.get_output("out_char"), Some(Value::Char(b'B')));
    assert_eq!(
        harness.get_output("out_wchar"),
        Some(Value::WChar('Y' as u16))
    );
}

#[test]
fn vm_opcode_positive_path_covers_non_ascii_string_and_wstring_index_reads() {
    let source = r#"
        PROGRAM Main
        VAR
            text_value : STRING[8] := 'ÄBC';
            wide_value : WSTRING[8] := "ÄBC";
            out_char : CHAR;
            out_wchar : WCHAR;
        END_VAR

        out_char := text_value[INT#1];
        out_wchar := wide_value[INT#1];
        END_PROGRAM
    "#;

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "non-ascii string index execution failed: {:?}",
        cycle.errors
    );
    assert_eq!(harness.get_output("out_char"), Some(Value::Char(0xC4)));
    assert_eq!(harness.get_output("out_wchar"), Some(Value::WChar(0x00C4)));
}

#[test]
fn vm_opcode_positive_path_covers_sizeof_type_and_storage_operands() {
    let source = r#"
        PROGRAM Main
        VAR
            out_size_type_int : DINT := DINT#0;
            sized : STRING[5];
            out_size_var_s : DINT := DINT#0;
        END_VAR

        out_size_type_int := SIZEOF(INT);
        out_size_var_s := SIZEOF(sized);
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x60),
        "expected SIZEOF_TYPE opcode in main body"
    );
    assert!(
        !body.contains(&0x61),
        "did not expect legacy SIZEOF_VALUE opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "SIZEOF opcode execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_size_type_int", 2i32);
    harness.assert_eq("out_size_var_s", 5i32);
}
