#[test]
fn vm_malformed_bytecode_fuzz_smoke_budget() {
    let source = r#"
        FUNCTION AddOne : DINT
        VAR_INPUT
            x : DINT;
        END_VAR
        VAR
            y : DINT;
        END_VAR
            y := x + 1;
            AddOne := y;
        END_FUNCTION

        PROGRAM Main
        VAR
            count : DINT := 0;
            s : STRING := '';
            ws : WSTRING := "";
        END_VAR
            count := AddOne(count);
            s := 'A';
            ws := "A";
        END_PROGRAM
    "#;

    for seed in 0..8 {
        let mut module = bytecode_module_from_source(source).expect("compile module");
        let expected = match seed {
            0 => {
                if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex)
                {
                    let duplicate_id = index.entries[0].id;
                    index.entries[1].id = duplicate_id;
                }
                "duplicate POU id"
            }
            1 => {
                replace_main_body(&mut module, &[0x07]);
                "unsupported runtime opcode CALL_METHOD"
            }
            2 => {
                let mut body = vec![0x10];
                body.extend_from_slice(&255_u32.to_le_bytes());
                replace_main_body(&mut module, &body);
                "invalid index 255 for const"
            }
            3 => {
                replace_main_body(&mut module, &[0x20]);
                "unexpected end of input"
            }
            4 => {
                let mut body = vec![0x20];
                body.extend_from_slice(&255_u32.to_le_bytes());
                replace_main_body(&mut module, &body);
                "invalid index 255 for ref"
            }
            5 => {
                let mut body = vec![0x02];
                body.extend_from_slice(&(4_096_i32).to_le_bytes());
                body.push(0x06);
                replace_main_body(&mut module, &body);
                "invalid jump target"
            }
            6 => {
                let strings = match module.section(SectionId::StringTable) {
                    Some(SectionData::StringTable(strings)) => strings.clone(),
                    _ => panic!("missing STRING_TABLE"),
                };
                if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex)
                {
                    let function = index
                        .entries
                        .iter_mut()
                        .find(|entry| {
                            entry.kind == PouKind::Function
                                && strings.entries[entry.name_idx as usize]
                                    .eq_ignore_ascii_case("ADDONE")
                        })
                        .expect("AddOne function");
                    function.local_ref_count = 0;
                }
                "local ref outside POU local range"
            }
            7 => {
                mutate_first_const_payload_for_primitive(&mut module, 24, vec![0xFF]);
                "invalid STRING const UTF-8"
            }
            _ => unreachable!(),
        };

        assert_apply_invalid_bytecode_contains(&module, expected);
    }
}

#[test]
fn vm_rejects_stack_underflow() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    replace_main_body(&mut module, &[0x12, 0x06]);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert_invalid_bytecode_contains(&cycle.errors, "vm operand stack underflow");
}

#[test]
fn vm_rejects_stack_overflow() {
    let source = r#"
        PROGRAM Main
        VAR
            keep: DINT := 1;
        END_VAR
        keep := keep + 0;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let const_idx = match module.section(SectionId::ConstPool) {
        Some(SectionData::ConstPool(pool)) if !pool.entries.is_empty() => 0_u32,
        Some(_) => panic!("expected const pool entries for overflow fixture"),
        _ => panic!("missing CONST_POOL"),
    };

    let mut body = Vec::new();
    body.push(0x10);
    body.extend_from_slice(&const_idx.to_le_bytes());
    body.push(0x11);
    body.push(0x02);
    body.extend_from_slice(&(-6_i32).to_le_bytes());
    replace_main_body(&mut module, &body);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert_invalid_bytecode_contains(&cycle.errors, "vm operand stack overflow");
}

#[test]
fn vm_call_stack_handles_call_and_return() {
    let source = r#"
        FUNCTION Foo : DINT
        Foo := 1;
        END_FUNCTION

        PROGRAM Main
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let (main_id, foo_id) = {
        let strings = match module.section(SectionId::StringTable) {
            Some(SectionData::StringTable(strings)) => strings,
            _ => panic!("missing string table"),
        };
        let index = match module.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index,
            _ => panic!("missing pou index"),
        };
        let mut main_id = None;
        let mut foo_id = None;
        for entry in &index.entries {
            let name = &strings.entries[entry.name_idx as usize];
            if name.eq_ignore_ascii_case("MAIN") {
                main_id = Some(entry.id);
            }
            if name.eq_ignore_ascii_case("FOO") {
                foo_id = Some(entry.id);
            }
        }
        (main_id.expect("Main POU id"), foo_id.expect("Foo POU id"))
    };

    let main_body = {
        let mut bytes = Vec::new();
        bytes.push(0x05);
        bytes.extend_from_slice(&foo_id.to_le_bytes());
        bytes.push(0x00);
        bytes.push(0x06);
        bytes
    };
    let foo_body = vec![0x06];

    let (main_offset, foo_offset) =
        if let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) {
            let main_offset = code.len() as u32;
            code.extend_from_slice(&main_body);
            let foo_offset = code.len() as u32;
            code.extend_from_slice(&foo_body);
            (main_offset, foo_offset)
        } else {
            panic!("missing POU_BODIES");
        };
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        for entry in &mut index.entries {
            if entry.id == main_id {
                entry.code_offset = main_offset;
                entry.code_length = main_body.len() as u32;
            } else if entry.id == foo_id {
                entry.code_offset = foo_offset;
                entry.code_length = foo_body.len() as u32;
            }
        }
    }
    module.sections.retain(|section| {
        section.id != SectionId::DebugMap.as_raw()
            && section.id != SectionId::DebugStringTable.as_raw()
    });

    let bytes = module.encode().expect("encode module");
    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes, None)
        .expect("apply bytecode");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");

    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "call/ret VM execution should succeed, got {:?}",
        cycle.errors
    );
}
