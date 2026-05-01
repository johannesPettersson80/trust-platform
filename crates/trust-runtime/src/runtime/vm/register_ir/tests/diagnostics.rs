#[test]
fn diagnostic_find_fallback_opcodes_in_corpus() {
    let fixtures: &[(&str, &str)] = &[
        (
            "call-binding",
            r#"
                FUNCTION Add : INT
                VAR_INPUT a : INT; b : INT := INT#2; END_VAR
                Add := a + b;
                END_FUNCTION
                FUNCTION Bump : INT
                VAR_IN_OUT x : INT; END_VAR
                VAR_INPUT inc : INT := INT#1; END_VAR
                x := x + inc; Bump := x;
                END_FUNCTION
                PROGRAM Main
                VAR v : INT := INT#10; out_named : INT := INT#0;
                    out_default : INT := INT#0; out_inout : INT := INT#0; END_VAR
                out_named := Add(b := INT#4, a := INT#3);
                out_default := Add(a := INT#3);
                out_inout := Bump(v, INT#5);
                END_PROGRAM
            "#,
        ),
        (
            "string-stdlib",
            r#"
                PROGRAM Main
                VAR out_left : STRING := ''; out_mid : STRING := '';
                    out_find_found : INT := INT#0; out_find_missing : INT := INT#0;
                    out_w_replace : WSTRING := ""; out_w_insert : WSTRING := ""; END_VAR
                out_left := LEFT(IN := 'ABCDE', L := INT#3);
                out_mid := MID(IN := 'ABCDE', L := INT#2, P := INT#2);
                out_find_found := FIND(IN1 := 'ABCDE', IN2 := 'BC');
                out_find_missing := FIND(IN1 := 'BC', IN2 := 'ABCDE');
                out_w_replace := REPLACE(IN1 := "ABCDE", IN2 := "Z", L := INT#2, P := INT#3);
                out_w_insert := INSERT(IN1 := "ABE", IN2 := "CD", P := INT#3);
                END_PROGRAM
            "#,
        ),
        (
            "refs-sizeof",
            r#"
                TYPE
                    Inner : STRUCT arr : ARRAY[0..2] OF INT; END_STRUCT;
                    Outer : STRUCT inner : Inner; END_STRUCT;
                END_TYPE
                PROGRAM Main
                VAR o : Outer; idx : INT := INT#1; value_cell : INT := INT#4;
                    r_value : REF_TO INT; r_outer : REF_TO Outer;
                    out_ref : INT := INT#0; out_after_write : INT := INT#0;
                    out_nested_chain : INT := INT#0; out_size_type_int : DINT := DINT#0; END_VAR
                r_value := REF(value_cell);
                r_outer := REF(o);
                out_ref := r_value^;
                r_value^ := r_value^ + INT#3;
                out_after_write := r_value^;
                out_nested_chain := r_outer^.inner.arr[idx];
                out_size_type_int := SIZEOF(INT);
                END_PROGRAM
            "#,
        ),
    ];

    for (name, source) in fixtures {
        let (vm_module, pou_id) = vm_module_and_main_pou(source);
        let lowered = lower_pou_to_register_ir(&vm_module, pou_id);
        match lowered {
            Err(e) => {
                panic!("fixture '{name}': lowering error: {e:?}");
            }
            Ok(program) => {
                let fallbacks: Vec<_> = program
                    .blocks
                    .iter()
                    .flat_map(|b| b.instructions.iter())
                    .filter_map(|i| match i {
                        RegisterInstr::VmFallback { opcode, .. } => Some(*opcode),
                        _ => None,
                    })
                    .collect();
                if !fallbacks.is_empty() {
                    let opcodes_hex: Vec<_> =
                        fallbacks.iter().map(|o| format!("0x{o:02X}")).collect();
                    panic!(
                        "fixture '{name}': has VmFallback instructions for opcodes: [{}]",
                        opcodes_hex.join(", ")
                    );
                }
                let has_complex = super::lowered_uses_complex_local_paths(&vm_module, &program);
                if has_complex {
                    // Find which ref indices are complex
                    let mut complex_refs = Vec::new();
                    for instr in program.blocks.iter().flat_map(|b| b.instructions.iter()) {
                        let ref_idx = match instr {
                            RegisterInstr::LoadRef { ref_idx, .. }
                            | RegisterInstr::LoadRefAddr { ref_idx, .. }
                            | RegisterInstr::StoreRef { ref_idx, .. } => *ref_idx,
                            _ => continue,
                        };
                        if let Some(VmRef::Local { path, .. }) =
                            vm_module.refs.get(ref_idx as usize)
                        {
                            if !path.is_empty() {
                                complex_refs.push(ref_idx);
                            }
                        }
                    }
                    panic!(
                            "fixture '{name}': blocked by complex_local_ref_path, ref indices: {complex_refs:?}"
                        );
                }
                eprintln!(
                    "fixture '{name}': PASS (no fallback instructions, no complex local refs)"
                );
            }
        }
    }
}

#[test]
fn diagnostic_execute_corpus_through_register_ir() {
    use crate::execution_backend::ExecutionBackend;
    use crate::harness::{bytecode_bytes_from_source, TestHarness};
    use crate::RestartMode;

    let fixtures: &[(&str, &str)] = &[
        (
            "call-binding",
            r#"
                FUNCTION Add : INT
                VAR_INPUT a : INT; b : INT := INT#2; END_VAR
                Add := a + b;
                END_FUNCTION
                FUNCTION Bump : INT
                VAR_IN_OUT x : INT; END_VAR
                VAR_INPUT inc : INT := INT#1; END_VAR
                x := x + inc; Bump := x;
                END_FUNCTION
                PROGRAM Main
                VAR v : INT := INT#10; out_named : INT := INT#0;
                    out_default : INT := INT#0; out_inout : INT := INT#0; END_VAR
                out_named := Add(b := INT#4, a := INT#3);
                out_default := Add(a := INT#3);
                out_inout := Bump(v, INT#5);
                END_PROGRAM
            "#,
        ),
        (
            "string-stdlib",
            r#"
                PROGRAM Main
                VAR out_left : STRING := ''; out_mid : STRING := '';
                    out_find_found : INT := INT#0; out_find_missing : INT := INT#0;
                    out_w_replace : WSTRING := ""; out_w_insert : WSTRING := ""; END_VAR
                out_left := LEFT(IN := 'ABCDE', L := INT#3);
                out_mid := MID(IN := 'ABCDE', L := INT#2, P := INT#2);
                out_find_found := FIND(IN1 := 'ABCDE', IN2 := 'BC');
                out_find_missing := FIND(IN1 := 'BC', IN2 := 'ABCDE');
                out_w_replace := REPLACE(IN1 := "ABCDE", IN2 := "Z", L := INT#2, P := INT#3);
                out_w_insert := INSERT(IN1 := "ABE", IN2 := "CD", P := INT#3);
                END_PROGRAM
            "#,
        ),
        (
            "refs-sizeof",
            r#"
                TYPE
                    Inner : STRUCT arr : ARRAY[0..2] OF INT; END_STRUCT;
                    Outer : STRUCT inner : Inner; END_STRUCT;
                END_TYPE
                PROGRAM Main
                VAR o : Outer; idx : INT := INT#1; value_cell : INT := INT#4;
                    r_value : REF_TO INT; r_outer : REF_TO Outer;
                    out_ref : INT := INT#0; out_after_write : INT := INT#0;
                    out_nested_chain : INT := INT#0; out_size_type_int : DINT := DINT#0; END_VAR
                r_value := REF(value_cell);
                r_outer := REF(o);
                out_ref := r_value^;
                r_value^ := r_value^ + INT#3;
                out_after_write := r_value^;
                out_nested_chain := r_outer^.inner.arr[idx];
                out_size_type_int := SIZEOF(INT);
                END_PROGRAM
            "#,
        ),
    ];

    for (name, source) in fixtures {
        let mut harness = TestHarness::from_source(source).expect("create harness");
        let bytes = bytecode_bytes_from_source(source).expect("compile bytecode");
        harness
            .runtime_mut()
            .apply_bytecode_bytes(&bytes, None)
            .expect("apply bytecode");
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("set backend");
        harness
            .runtime_mut()
            .restart(RestartMode::Cold)
            .expect("restart");
        harness.runtime_mut().set_vm_register_profile_enabled(true);
        harness.runtime_mut().reset_vm_register_profile();

        let result = harness.cycle();
        if !result.errors.is_empty() {
            panic!("fixture '{name}': cycle errors: {:?}", result.errors);
        }

        let snapshot = harness.runtime().vm_register_profile_snapshot();
        eprintln!(
            "fixture '{name}': executed={}, fallbacks={}, reasons={:?}",
            snapshot.register_programs_executed,
            snapshot.register_program_fallbacks,
            snapshot.fallback_reasons,
        );
        assert!(
                snapshot.register_programs_executed > 0,
                "fixture '{name}': expected register execution, got 0 executed and {} fallbacks, reasons: {:?}",
                snapshot.register_program_fallbacks,
                snapshot.fallback_reasons,
            );
        assert_eq!(
            snapshot.register_program_fallbacks, 0,
            "fixture '{name}': expected zero register fallbacks, reasons: {:?}",
            snapshot.fallback_reasons
        );
    }
}

#[test]
fn diagnostic_register_ir_callee_path_populates_lowering_cache() {
    use crate::execution_backend::ExecutionBackend;
    use crate::harness::{bytecode_bytes_from_source, TestHarness};
    use crate::RestartMode;

    let source = r#"
            FUNCTION Add : INT
            VAR_INPUT
                a : INT;
                b : INT := INT#2;
            END_VAR
            Add := a + b;
            END_FUNCTION

            FUNCTION Bump : INT
            VAR_IN_OUT
                x : INT;
            END_VAR
            VAR_INPUT
                inc : INT := INT#1;
            END_VAR
            x := x + inc;
            Bump := x;
            END_FUNCTION

            PROGRAM Main
            VAR
                v : INT := INT#10;
                out_named : INT := INT#0;
                out_default : INT := INT#0;
                out_inout : INT := INT#0;
            END_VAR

            out_named := Add(b := INT#4, a := INT#3);
            out_default := Add(a := INT#3);
            out_inout := Bump(v, INT#5);
            END_PROGRAM
        "#;

    let mut harness = TestHarness::from_source(source).expect("create harness");
    let bytes = bytecode_bytes_from_source(source).expect("compile bytecode");
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes, None)
        .expect("apply bytecode");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("set backend");
    harness
        .runtime_mut()
        .restart(RestartMode::Cold)
        .expect("restart");
    harness
        .runtime_mut()
        .set_vm_register_lowering_cache_enabled(true);
    harness.runtime_mut().reset_vm_register_lowering_cache();

    let first = harness.cycle();
    assert!(
        first.errors.is_empty(),
        "first cycle errors: {:?}",
        first.errors
    );
    let second = harness.cycle();
    assert!(
        second.errors.is_empty(),
        "second cycle errors: {:?}",
        second.errors
    );

    let cache = harness.runtime().vm_register_lowering_cache_snapshot();
    assert!(
        cache.cached_entries >= 2,
        "expected main + callee programs cached, got {} entries",
        cache.cached_entries
    );
    assert!(
        cache.hits > 0,
        "expected lowering-cache hits after second cycle, snapshot={cache:?}"
    );
}
