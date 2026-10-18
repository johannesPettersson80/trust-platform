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

    assert_apply_invalid_bytecode_contains(&module, "POU body leaves values on operand stack");
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
    body.push(0x12);
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

    assert_apply_invalid_bytecode_contains(
        &module,
        "operand stack underflow while decoding opcode 0x61",
    );
}

/// Build the `SIZEOF(INT)` module with `alias_depth` forward aliases ending at the `INT` type.
fn sizeof_over_alias_chain_module(source: &str, alias_depth: u32) -> BytecodeModule {
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let int_type_idx = first_opcode_u32_operand(&module, 0x60);
    let alias_start = match module.section(SectionId::TypeTable) {
        Some(SectionData::TypeTable(table)) => table.entries.len() as u32,
        _ => panic!("missing TYPE_TABLE"),
    };
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
    module
}

#[test]
fn an_alias_chain_deeper_than_64_edges_is_refused_before_construction() {
    let source = r#"
        PROGRAM Main
        VAR
            out_size : DINT := DINT#0;
        END_VAR
        out_size := SIZEOF(INT);
        END_PROGRAM
    "#;

    // spec 12 §4.6, lines 232-235: an alias counts as one structural edge, so the
    // 65th edge is rejected before construction. A 129-edge chain reaches an alias
    // at that depth during preparation, which refuses it with its own refusal naming
    // the bound. Catches an admission that accepts a type deeper than the 64-edge
    // bound, and a depth refusal reported as a type mismatch.
    let deep = sizeof_over_alias_chain_module(source, 129);
    let error = trust_runtime::harness::CompileSession::from_source(source)
        .build_runtime_from_bytecode(&deep, None)
        .expect_err("a 129-edge alias chain must be refused before construction");
    assert_eq!(
        error.to_string(),
        "type nesting exceeds 64 structural edges"
    );

    // Exactly 64 edges (65 nodes) are within the bound: `SIZEOF` walks the chain to
    // `INT` and answers its 2 bytes. Catches a refusal of a chain inside the bound, or
    // an alias edge that is not counted (which would admit depth 65).
    let bounded = sizeof_over_alias_chain_module(source, 64);
    let mut harness = vm_harness_from_module(source, &bounded);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "SIZEOF over a 64-edge alias chain failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_size", 2i32);
}
