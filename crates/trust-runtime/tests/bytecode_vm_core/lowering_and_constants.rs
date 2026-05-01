#[test]
fn vm_lowering_supports_exit_and_continue_in_loop_stmt_paths() {
    let source = r#"
        PROGRAM Main
        VAR
            i : INT := INT#0;
            w : INT := INT#0;
            r : INT := INT#0;
            sum_for : INT := INT#0;
            sum_while : INT := INT#0;
            sum_repeat : INT := INT#0;
        END_VAR

        FOR i := INT#0 TO INT#4 BY INT#1 DO
            IF i = INT#1 THEN
                CONTINUE;
            END_IF;
            IF i = INT#3 THEN
                EXIT;
            END_IF;
            sum_for := sum_for + i;
        END_FOR;

        WHILE w < INT#5 DO
            w := w + INT#1;
            IF w = INT#2 THEN
                CONTINUE;
            END_IF;
            IF w = INT#4 THEN
                EXIT;
            END_IF;
            sum_while := sum_while + w;
        END_WHILE;

        REPEAT
            r := r + INT#1;
            IF r = INT#2 THEN
                CONTINUE;
            END_IF;
            IF r = INT#4 THEN
                EXIT;
            END_IF;
            sum_repeat := sum_repeat + r;
        UNTIL r >= INT#6 END_REPEAT;
        END_PROGRAM
    "#;

    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x02),
        "expected JMP opcode in main loop body"
    );
    assert!(
        body.contains(&0x03),
        "expected JMP_IF_TRUE opcode in main loop body"
    );
    assert!(
        body.contains(&0x04),
        "expected JMP_IF_FALSE opcode in main loop body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "vm execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("sum_for", 2i16);
    harness.assert_eq("sum_while", 4i16);
    harness.assert_eq("sum_repeat", 4i16);
}

#[test]
fn vm_lowering_rejects_unsupported_c5_edge_case_stmt_paths() {
    let source = r#"
        PROGRAM Main
        VAR
            x : INT := INT#0;
        END_VAR
        JMP L1;
        x := INT#1;
        L1: x := x + INT#2;
        END_PROGRAM
    "#;
    let err =
        bytecode_module_from_source(source).expect_err("expected deterministic lowering error");
    let message = err.to_string();
    assert!(
        message.contains("unsupported C5 edge-case lowering path"),
        "expected deterministic C5 lowering rejection, got: {message}"
    );
}

#[test]
fn vm_rejects_invalid_string_const_utf8_payload() {
    let source = r#"
        PROGRAM Main
        VAR
            s : STRING := '';
        END_VAR
        s := 'A';
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    mutate_first_const_payload_for_primitive(&mut module, 24, vec![0xFF]);

    assert_apply_invalid_bytecode_contains(&module, "invalid STRING const UTF-8");
}

#[test]
fn vm_rejects_invalid_wstring_const_utf16_payload() {
    let source = r#"
        PROGRAM Main
        VAR
            ws : WSTRING := "";
        END_VAR
        ws := "A";
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    mutate_first_const_payload_for_primitive(&mut module, 25, vec![0x41]);

    assert_apply_invalid_bytecode_contains(&module, "invalid WSTRING const payload length");
}

#[test]
fn vm_enforces_instruction_budget() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x02);
    body.extend_from_slice(&(-5_i32).to_le_bytes());
    replace_main_body(&mut module, &body);

    let mut harness = vm_harness_from_module(source, &module);
    harness.runtime_mut().set_execution_deadline(None);
    let cycle = harness.cycle();
    assert!(
        cycle
            .errors
            .iter()
            .any(|err| matches!(err, RuntimeError::ExecutionTimeout)),
        "expected ExecutionTimeout from instruction budget, got {:?}",
        cycle.errors
    );
}

