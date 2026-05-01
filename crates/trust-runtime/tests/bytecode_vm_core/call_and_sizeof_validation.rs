#[test]
fn vm_rejects_invalid_call_native_symbol_index() {
    let source = r#"
        PROGRAM Main
        VAR
            keep : INT := 0;
        END_VAR
        keep := keep + INT#1;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x09);
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.extend_from_slice(&255_u32.to_le_bytes());
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.push(0x06);
    replace_main_body(&mut module, &body);

    assert_apply_invalid_bytecode_contains(&module, "invalid index 255 for native symbol");
}

#[test]
fn vm_rejects_invalid_call_native_method_missing_receiver_payload() {
    let source = r#"
        CLASS Counter
        METHOD PUBLIC Next : INT
        Next := INT#1;
        END_METHOD
        END_CLASS

        PROGRAM Main
        VAR
            c : Counter;
            out_next : INT := INT#0;
        END_VAR
        out_next := c.Next();
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    patch_first_call_native_arg_count(&mut module, 0);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert_invalid_bytecode_contains(
        &cycle.errors,
        "vm invalid CALL_NATIVE payload: arg_count smaller than native receiver arity",
    );
}

#[test]
fn vm_validator_rejects_invalid_ref_field_string_index() {
    let source = r#"
        TYPE
            Box : STRUCT
                value : INT;
            END_STRUCT;
        END_TYPE

        PROGRAM Main
        VAR
            b : Box;
            out_value : INT := INT#0;
        END_VAR
        b.value := INT#7;
        out_value := REF(b)^.value;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    patch_first_opcode_u32_operand(&mut module, 0x30, 255);

    assert_apply_invalid_bytecode_contains(&module, "invalid index 255 for string");
}

#[test]
fn vm_rejects_load_dynamic_with_non_reference_operand() {
    let source = r#"
        PROGRAM Main
        VAR
            x : INT := INT#1;
        END_VAR
        x := x;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x20);
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.push(0x32);
    body.push(0x06);
    replace_main_body(&mut module, &body);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert!(
        cycle
            .errors
            .iter()
            .any(|err| matches!(err, RuntimeError::TypeMismatch)),
        "expected TypeMismatch for LOAD_DYNAMIC on non-reference, got {:?}",
        cycle.errors
    );
}

#[test]
fn vm_validator_rejects_invalid_sizeof_type_index() {
    let source = r#"
        PROGRAM Main
        VAR
            out_size : DINT := DINT#0;
        END_VAR
        out_size := SIZEOF(INT);
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    patch_first_opcode_u32_operand(&mut module, 0x60, 255);

    assert_apply_invalid_bytecode_contains(&module, "invalid index 255 for type");
}

#[test]
fn vm_rejects_legacy_sizeof_value_opcode_with_empty_stack() {
    let source = r#"
        PROGRAM Main
        VAR
            out_size : DINT := DINT#0;
        END_VAR
        out_size := DINT#0;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let body = vec![
        0x61, // SIZEOF_VALUE without operand -> stack underflow
        0x06, // RET
    ];
    replace_main_body(&mut module, &body);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert_invalid_bytecode_contains(&cycle.errors, "vm operand stack underflow");
}

#[test]
fn vm_rejects_sizeof_type_with_excessive_non_cyclic_alias_depth() {
    let source = r#"
        PROGRAM Main
        VAR
            out_size : DINT := DINT#0;
        END_VAR
        out_size := SIZEOF(INT);
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let int_type_idx = first_opcode_u32_operand(&module, 0x60);
    let alias_start = match module.section(SectionId::TypeTable) {
        Some(SectionData::TypeTable(table)) => table.entries.len() as u32,
        _ => panic!("missing TYPE_TABLE"),
    };
    let alias_depth = 129u32;
    if let Some(SectionData::TypeTable(table)) = module.section_mut(SectionId::TypeTable) {
        for i in 0..alias_depth {
            let target_type_id = if i + 1 < alias_depth {
                alias_start + i + 1
            } else {
                int_type_idx
            };
            table.entries.push(TypeEntry {
                kind: TypeKind::Alias,
                name_idx: None,
                data: TypeData::Alias { target_type_id },
            });
        }
    } else {
        panic!("missing TYPE_TABLE");
    }
    patch_first_opcode_u32_operand(&mut module, 0x60, alias_start);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert_invalid_bytecode_contains(&cycle.errors, "SIZEOF type nesting exceeds max depth");
}

