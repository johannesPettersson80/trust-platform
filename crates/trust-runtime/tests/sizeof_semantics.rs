use trust_hir::types::POINTER_REFERENCE_HANDLE_SIZE_BYTES;
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};

#[test]
fn sizeof_variable_and_type_operands_build_and_run() {
    let source = r#"
        TYPE Pair :
        STRUCT
            a : DINT;
            b : BOOL;
        END_STRUCT
        END_TYPE

        PROGRAM Main
        VAR
            x : DINT;
            s : STRING[20];
            ws : WSTRING[3];
            out_x : DINT := DINT#0;
            out_type : DINT := DINT#0;
            out_s : DINT := DINT#0;
            out_ws : DINT := DINT#0;
        END_VAR

        out_x := SIZEOF(x);
        out_type := SIZEOF(Pair);
        out_s := SIZEOF(s);
        out_ws := SIZEOF(ws);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("build harness");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_x", 4i32);
    harness.assert_eq("out_type", 5i32);
    harness.assert_eq("out_s", 20i32);
    harness.assert_eq("out_ws", 6i32);
}

#[test]
fn sizeof_variable_shadows_type_name_and_qualified_type_remains_available() {
    let sources = [
        r#"
        NAMESPACE Demo
        TYPE Packet : LWORD; END_TYPE
        END_NAMESPACE
        "#,
        r#"
        PROGRAM Main
        VAR
            Packet : INT;
            out_var : DINT := DINT#0;
            out_type : DINT := DINT#0;
        END_VAR

        out_var := SIZEOF(Packet);
        out_type := SIZEOF(Demo.Packet);
        END_PROGRAM
        "#,
    ];

    let mut harness = TestHarness::from_sources(&sources).expect("build harness");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_var", 2i32);
    harness.assert_eq("out_type", 8i32);
}

#[test]
fn sizeof_works_in_array_bounds_for_variable_operands() {
    let source = r#"
        TYPE Packet :
        STRUCT
            a : DINT;
            b : BOOL;
        END_STRUCT
        END_TYPE

        PROGRAM Main
        VAR
            packet : Packet;
            bytes : ARRAY[0..SIZEOF(packet)-1] OF BYTE;
            out_packet : DINT := DINT#0;
            out_bytes : DINT := DINT#0;
        END_VAR

        out_packet := SIZEOF(packet);
        out_bytes := SIZEOF(bytes);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("build harness");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_packet", 5i32);
    harness.assert_eq("out_bytes", 5i32);
}

#[test]
fn sizeof_bare_name_prefers_variable_over_top_level_type_name() {
    let source = r#"
        TYPE Packet : LWORD; END_TYPE

        PROGRAM Main
        VAR
            Packet : INT;
            bytes : ARRAY[0..SIZEOF(Packet)-1] OF BYTE;
            out_var : DINT := DINT#0;
            out_bytes : DINT := DINT#0;
        END_VAR

        out_var := SIZEOF(Packet);
        out_bytes := SIZEOF(bytes);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("build harness");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_var", 2i32);
    harness.assert_eq("out_bytes", 2i32);
}

#[test]
fn sizeof_pointer_operands_const_fold_in_array_bounds() {
    let source = r#"
        PROGRAM Main
        VAR
            p : POINTER TO INT;
            bytes : ARRAY[0..SIZEOF(p)-1] OF BYTE;
            out_p : DINT := DINT#0;
            out_bytes : DINT := DINT#0;
        END_VAR

        out_p := SIZEOF(p);
        out_bytes := SIZEOF(bytes);
        END_PROGRAM
    "#;

    let expected = i32::try_from(POINTER_REFERENCE_HANDLE_SIZE_BYTES).expect("pointer size fits");
    let mut harness = TestHarness::from_source(source).expect("build harness");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_p", expected);
    harness.assert_eq("out_bytes", expected);
}

#[test]
fn sizeof_pointer_and_reference_operands_use_platform_pointer_size() {
    let source = r#"
        PROGRAM Main
        VAR
            p : POINTER TO INT;
            r : REF_TO INT;
            out_p : DINT := DINT#0;
            out_r : DINT := DINT#0;
        END_VAR

        out_p := SIZEOF(p);
        out_r := SIZEOF(r);
        END_PROGRAM
    "#;

    let expected = i32::try_from(POINTER_REFERENCE_HANDLE_SIZE_BYTES).expect("pointer size fits");
    let mut harness = TestHarness::from_source(source).expect("build harness");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_p", expected);
    harness.assert_eq("out_r", expected);
}

#[test]
fn sizeof_pointer_contract_matches_platform_word_size() {
    assert_eq!(
        POINTER_REFERENCE_HANDLE_SIZE_BYTES,
        std::mem::size_of::<usize>() as u64
    );
    #[cfg(target_pointer_width = "64")]
    assert_eq!(POINTER_REFERENCE_HANDLE_SIZE_BYTES, 8);
    #[cfg(target_pointer_width = "32")]
    assert_eq!(POINTER_REFERENCE_HANDLE_SIZE_BYTES, 4);
}

#[test]
fn sizeof_rejects_call_operands_during_build() {
    let source = r#"
        FUNCTION Value : DINT
        Value := DINT#1;
        END_FUNCTION

        PROGRAM Main
        VAR out_size : DINT; END_VAR
        out_size := SIZEOF(Value());
        END_PROGRAM
    "#;

    let err = CompileSession::from_source(source)
        .build_runtime()
        .expect_err("SIZEOF(call) should fail");
    let rendered = err.to_string();
    assert!(rendered.contains("SIZEOF"), "{rendered}");
}

#[test]
fn sizeof_rejects_function_block_instance_operands_during_build() {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR
            value : DINT;
        END_VAR
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            fb : Counter;
            out_size : DINT;
        END_VAR
        out_size := SIZEOF(fb);
        END_PROGRAM
    "#;

    let err = CompileSession::from_source(source)
        .build_runtime()
        .expect_err("SIZEOF(fb instance) should fail");
    let rendered = err.to_string();
    assert!(rendered.contains("SIZEOF"), "{rendered}");
}

#[test]
fn sizeof_rejects_unknown_identifiers_during_build() {
    let source = r#"
        PROGRAM Main
        VAR out_size : DINT; END_VAR
        out_size := SIZEOF(DoesNotExist);
        END_PROGRAM
    "#;

    let err = CompileSession::from_source(source)
        .build_runtime()
        .expect_err("unknown SIZEOF operand should fail");
    let rendered = err.to_string();
    assert!(rendered.contains("SIZEOF"), "{rendered}");
}

#[test]
fn sizeof_complete_program_fixture_supports_variable_and_type_operands() {
    let sources = vec![
        SourceFile::with_path(
            "types.st",
            include_str!("fixtures/complete_program/types.st"),
        ),
        SourceFile::with_path("lib.st", include_str!("fixtures/complete_program/lib.st")),
        SourceFile::with_path("api.st", include_str!("fixtures/complete_program/api.st")),
        SourceFile::with_path("impl.st", include_str!("fixtures/complete_program/impl.st")),
        SourceFile::with_path("main.st", include_str!("fixtures/complete_program/main.st")),
        SourceFile::with_path(
            "config.st",
            include_str!("fixtures/complete_program/config.st"),
        ),
    ];

    CompileSession::from_sources(sources)
        .label_errors(true)
        .build_runtime()
        .expect("complete program with SIZEOF should compile");
}
