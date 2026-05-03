use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};

fn unbound_program_source() -> &'static str {
    r#"
CONFIGURATION Conf
VAR_GLOBAL
    g_flag : BOOL;
END_VAR
END_CONFIGURATION

PROGRAM Main
VAR
    arr : ARRAY[1..2] OF INT;
END_VAR
arr[1] := INT#10;
END_PROGRAM
"#
}

#[test]
fn configuration_without_program_instance_errors_by_default() {
    let error = match TestHarness::from_source(unbound_program_source()) {
        Ok(_) => panic!("unbound PROGRAM under CONFIGURATION must fail load"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("unbound PROGRAM declaration"),
        "unexpected error: {error}"
    );
    assert!(
        error.to_string().contains("Main"),
        "error should name unbound program: {error}"
    );
}

#[test]
fn explicit_extra_program_instance_keeps_test_builder_opt_in() {
    let source = SourceFile::new(unbound_program_source());
    let runtime = CompileSession::from_sources(vec![source])
        .with_extra_program_instances(["Main"])
        .build_runtime()
        .expect("explicit extra program instance should opt in");

    assert!(runtime.storage().get_global("Main").is_some());
}
