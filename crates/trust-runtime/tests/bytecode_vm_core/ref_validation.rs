#[test]
fn vm_validator_rejects_invalid_ref_index_operand() {
    let source = r#"
        PROGRAM Main
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile bytecode module");
    let (main_offset, main_length) = {
        let strings = match module.section(SectionId::StringTable) {
            Some(SectionData::StringTable(strings)) => strings,
            _ => panic!("missing string table"),
        };
        let index = match module.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index,
            _ => panic!("missing pou index"),
        };
        let main = index
            .entries
            .iter()
            .find(|entry| {
                entry.kind == PouKind::Program
                    && strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("MAIN")
            })
            .expect("main entry");
        (main.code_offset as usize, main.code_length as usize)
    };
    if main_length < 5 {
        panic!("main body too short for patch");
    }
    if let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) {
        code[main_offset] = 0x20;
        code[main_offset + 1..main_offset + 5].copy_from_slice(&255_u32.to_le_bytes());
    } else {
        panic!("missing POU_BODIES");
    }

    assert_apply_invalid_bytecode_contains(&module, "invalid index 255 for ref");
}

#[test]
fn vm_validator_rejects_local_ref_outside_pou_local_range() {
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
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings.clone(),
        _ => panic!("missing STRING_TABLE"),
    };
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        let function = index
            .entries
            .iter_mut()
            .find(|entry| {
                entry.kind == PouKind::Function
                    && strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("ADDONE")
            })
            .expect("AddOne function");
        assert!(function.local_ref_count > 0, "expected function local refs");
        function.local_ref_count = 0;
    } else {
        panic!("missing POU_INDEX");
    }

    assert_apply_invalid_bytecode_contains(&module, "local ref outside POU local range");
}

#[test]
fn vm_validator_accepts_local_path_ref_for_pou_owned_local_slot() {
    let source = r#"
        FUNCTION ReadCell : DINT
        VAR
            cells : ARRAY[0..1] OF DINT;
        END_VAR
            ReadCell := 0;
        END_FUNCTION

        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings.clone(),
        _ => panic!("missing STRING_TABLE"),
    };
    let (function_id, local_ref_start, local_ref_count) = match module.section(SectionId::PouIndex)
    {
        Some(SectionData::PouIndex(index)) => {
            let function = index
                .entries
                .iter()
                .find(|entry| {
                    entry.kind == PouKind::Function
                        && strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("READCELL")
                })
                .expect("ReadCell function");
            (
                function.id,
                function.local_ref_start,
                function.local_ref_count,
            )
        }
        _ => panic!("missing POU_INDEX"),
    };
    assert!(
        local_ref_count > 0,
        "expected ReadCell to own at least one local slot"
    );

    let path_ref_idx = match module.section_mut(SectionId::RefTable) {
        Some(SectionData::RefTable(ref_table)) => {
            let owner_id = ref_table.entries[local_ref_start as usize].owner_id;
            let idx = ref_table.entries.len() as u32;
            ref_table.entries.push(RefEntry {
                location: RefLocation::Local,
                owner_id,
                offset: 0,
                segments: vec![RefSegment::Index(vec![0])],
            });
            idx
        }
        _ => panic!("missing REF_TABLE"),
    };

    let mut body = vec![0x20];
    body.extend_from_slice(&path_ref_idx.to_le_bytes());
    body.push(0x06);
    let new_offset =
        if let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) {
            let offset = code.len() as u32;
            code.extend_from_slice(&body);
            offset
        } else {
            panic!("missing POU_BODIES");
        };
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        let function = index
            .entries
            .iter_mut()
            .find(|entry| entry.id == function_id)
            .expect("ReadCell function entry");
        function.code_offset = new_offset;
        function.code_length = body.len() as u32;
    } else {
        panic!("missing POU_INDEX");
    }
    module.sections.retain(|section| {
        section.id != SectionId::DebugMap.as_raw()
            && section.id != SectionId::DebugStringTable.as_raw()
    });

    let bytes = module.encode().expect("encode module");
    let mut runtime = Runtime::new();
    runtime
        .apply_bytecode_bytes(&bytes, None)
        .expect("local path refs owned by the POU local frame should validate");
}

#[test]
fn vm_validator_rejects_invalid_const_index_operand() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x10);
    body.extend_from_slice(&255_u32.to_le_bytes());
    body.push(0x06);
    replace_main_body(&mut module, &body);

    assert_apply_invalid_bytecode_contains(&module, "invalid index 255 for const");
}

#[test]
fn vm_validator_rejects_duplicate_pou_ids() {
    let source = r#"
        FUNCTION Helper : DINT
            Helper := DINT#1;
        END_FUNCTION

        PROGRAM Main
        VAR
            value : DINT := DINT#0;
        END_VAR
            value := Helper();
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        assert!(index.entries.len() >= 2, "expected multiple POU entries");
        let duplicate_id = index.entries[0].id;
        index.entries[1].id = duplicate_id;
    } else {
        panic!("missing POU_INDEX");
    }

    assert_apply_invalid_bytecode_contains(&module, "duplicate POU id");
}

#[test]
fn vm_rejects_invalid_opcode() {
    let source = r#"
        PROGRAM Main
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile bytecode module");
    let (_, main_offset, _) = main_pou_entry(&module);
    if let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) {
        code[main_offset] = 0xFF;
    } else {
        panic!("missing POU_BODIES");
    }

    assert_apply_invalid_bytecode_contains(&module, "invalid opcode 0xFF");
}

#[test]
fn vm_rejects_malformed_operands() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    replace_main_body(&mut module, &[0x20]);

    assert_apply_invalid_bytecode_contains(&module, "unexpected end of input");
}

#[test]
fn vm_rejects_invalid_jump_target() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x02);
    body.extend_from_slice(&(4_096_i32).to_le_bytes());
    body.push(0x06);
    replace_main_body(&mut module, &body);

    assert_apply_invalid_bytecode_contains(&module, "invalid jump target");
}

#[test]
fn vm_validator_rejects_unsupported_call_method_opcode() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x07);
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.push(0x06);
    replace_main_body(&mut module, &body);

    assert_apply_invalid_bytecode_contains(&module, "unsupported runtime opcode CALL_METHOD");
}

