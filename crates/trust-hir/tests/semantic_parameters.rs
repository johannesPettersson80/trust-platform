mod common;
use common::*;

// Parameter Tests
#[test]
fn test_function_parameters_collected() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION Add : INT
    VAR_INPUT
        a : INT;
        b : INT;
    END_VAR
    Add := a + b;
END_FUNCTION
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);

    // Find the function
    let func_sym = symbols.iter().find(|s| s.name == "Add").unwrap();
    assert!(matches!(func_sym.kind, SymbolKind::Function { .. }));
    let func_id = func_sym.id;

    // Find parameters
    let params: Vec<_> = symbols
        .iter()
        .filter(|s| s.parent == Some(func_id) && matches!(&s.kind, SymbolKind::Parameter { .. }))
        .collect();
    assert_eq!(params.len(), 2);

    // Check parameter names
    let param_names: Vec<_> = params.iter().map(|p| p.name.as_str()).collect();
    assert!(param_names.contains(&"a"));
    assert!(param_names.contains(&"b"));

    // Check parameter directions
    for param in &params {
        if let SymbolKind::Parameter { direction } = &param.kind {
            assert_eq!(*direction, ParamDirection::In);
        }
    }
}

#[test]
fn test_function_block_with_outputs() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION_BLOCK Counter
    VAR_INPUT
        Enable : BOOL;
    END_VAR
    VAR_OUTPUT
        Count : DINT;
    END_VAR
    VAR
        localVar : DINT;
    END_VAR
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);

    // Count different symbol kinds
    let fb_sym = symbols.iter().find(|s| s.name == "Counter").unwrap();
    let fb_id = fb_sym.id;

    let inputs: Vec<_> = symbols
        .iter()
        .filter(|s| {
            matches!(
                &s.kind,
                SymbolKind::Parameter {
                    direction: ParamDirection::In
                }
            ) && s.parent == Some(fb_id)
        })
        .collect();
    let outputs: Vec<_> = symbols
        .iter()
        .filter(|s| {
            matches!(
                &s.kind,
                SymbolKind::Parameter {
                    direction: ParamDirection::Out
                }
            ) && s.parent == Some(fb_id)
        })
        .collect();

    assert_eq!(inputs.len(), 1, "Should have 1 input parameter");
    assert_eq!(outputs.len(), 1, "Should have 1 output parameter");

    // Check that localVar is somewhere in the symbols (may be Variable or other)
    let local_found = symbols.iter().any(|s| s.name == "localVar");
    assert!(local_found, "Should have 'localVar' symbol");
}

/// A function block's body does not assign its own input (IEC 61131-3 Ed.3 Figure 13 rule 2a);
/// under the CODESYS profile it may, as CODESYS practice does (`sEventTypeOverride := ...`;
/// IEC_DEVIATIONS DEV-029). Catches the CODESYS rule applied to an IEC project, and the profile
/// ignored.
#[test]
fn test_own_pou_var_input_member_is_writable() {
    let source = r#"
FUNCTION_BLOCK FBEntity
VAR_INPUT
    sOverride : STRING;
    iCount : INT;
END_VAR
    sOverride := 'manual';
    iCount := iCount + 1;
END_FUNCTION_BLOCK
"#;
    check_has_error(source, DiagnosticCode::InvalidAssignmentTarget);
    check_no_errors_in(Dialect::Codesys, source);
}

/// Outside its instance a function block's input may be assigned separately, for the next call
/// (IEC 61131-3 Ed.3 Figure 13 rule 2b), under both profiles. Catches a profile that rejects it.
#[test]
fn test_external_fb_var_input_member_is_writable() {
    let source = r#"
FUNCTION_BLOCK FBTune
VAR_INPUT
    level : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR t : FBTune; END_VAR
    t.level := 5;
END_PROGRAM
"#;
    check_no_errors(source);
    check_no_errors_in(Dialect::Codesys, source);
}

/// Another program's input is not reachable by the program's name (IEC 61131-3 Ed.3 §6.6.4 items
/// 2 and 3); under the CODESYS profile `Program.input := value` writes it (IEC_DEVIATIONS
/// DEV-030). Catches the CODESYS access accepted in an IEC project, and the profile ignored.
#[test]
fn test_external_program_var_input_member_is_writable() {
    let source = r#"
PROGRAM Setpoint
VAR_INPUT
    target : INT;
END_VAR
END_PROGRAM
PROGRAM Main
    Setpoint.target := 42;
END_PROGRAM
"#;
    assert!(
        !check_errors(source).is_empty(),
        "IEC rejects Setpoint.target"
    );
    check_no_errors_in(Dialect::Codesys, source);
}

/// A function's parameter stays call-local under both profiles. Catches the CODESYS rule for
/// function block and program inputs leaking into functions.
#[test]
fn test_function_parameter_assignment_stays_rejected() {
    let source = r#"
FUNCTION Scale : INT
VAR_INPUT
    factor : INT;
END_VAR
    factor := 2;
    Scale := factor;
END_FUNCTION
"#;
    check_has_error(source, DiagnosticCode::InvalidAssignmentTarget);
    check_has_error_in(
        Dialect::Codesys,
        source,
        DiagnosticCode::InvalidAssignmentTarget,
    );
}
