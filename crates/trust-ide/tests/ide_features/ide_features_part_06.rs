use super::*;

#[test]
fn test_ide_diagnostics_allow_var_input_pointer_write_through() {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_INPUT
    PT : POINTER TO ARRAY[0..9] OF BYTE;
END_VAR
    PT^[2] := BYTE#7;
END_FUNCTION_BLOCK
"#;
    let (db, file) = setup(source);
    let diagnostics = collect_diagnostics(&db, file);
    assert!(
        diagnostics.iter().all(|diag| !diag
            .message
            .contains("cannot assign to input parameter 'PT'")),
        "unexpected diagnostics: {:?}",
        diagnostics
            .iter()
            .map(|diag| diag.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_ide_diagnostics_reject_array_wildcard_outside_parameter_sections() {
    let source = r#"
PROGRAM Main
VAR
    x : ARRAY[*] OF BYTE;
END_VAR
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let diagnostics = collect_diagnostics(&db, file);
    assert!(
        diagnostics
            .iter()
            .any(|diag| diag.message.contains("array wildcard '*' is only allowed")),
        "expected wildcard diagnostic, got {:?}",
        diagnostics
            .iter()
            .map(|diag| diag.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_completion_constant_parameter_uses_constant_kind() {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_INPUT
    CONSTANT A : INT;
END_VAR
VAR_OUTPUT
    C : INT;
END_VAR
    C := |A;
END_FUNCTION_BLOCK
"#;
    let cursor = source.find('|').expect("cursor");
    let mut cleaned = source.to_string();
    cleaned.remove(cursor);

    let (db, file) = setup(&cleaned);
    let items = complete(&db, file, TextSize::from(cursor as u32));
    let a_item = items
        .iter()
        .find(|item| item.label.eq_ignore_ascii_case("A"))
        .expect("A completion");
    assert_eq!(a_item.kind, CompletionKind::Constant);
}

#[test]
fn test_semantic_tokens_parameter_constant_is_readonly() {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_INPUT
    CONSTANT A : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
    let (db, file) = setup(source);
    let tokens = semantic_tokens(&db, file);

    let a_offset = source.find("A : INT").unwrap() as u32;
    let a_token = tokens
        .iter()
        .find(|t| u32::from(t.range.start()) == a_offset)
        .expect("token for A");

    assert_eq!(a_token.token_type, SemanticTokenType::Parameter);
    assert!(a_token.modifiers.readonly);
}

#[test]
fn test_semantic_tokens_var_temp_constant_is_readonly() {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_TEMP
    CONSTANT T : INT := INT#7;
END_VAR
END_FUNCTION_BLOCK
"#;
    let (db, file) = setup(source);
    let tokens = semantic_tokens(&db, file);

    let t_offset = source.find("T : INT").unwrap() as u32;
    let t_token = tokens
        .iter()
        .find(|t| u32::from(t.range.start()) == t_offset)
        .expect("token for T");

    assert_eq!(t_token.token_type, SemanticTokenType::Variable);
    assert!(t_token.modifiers.readonly);
}

#[test]
fn test_hover_parameter_constant_mentions_constant_and_array_star() {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_INPUT
    CONSTANT A : ARRAY[*] OF BYTE;
END_VAR
VAR_OUTPUT
    X : BYTE;
END_VAR
    X := A[0];
END_FUNCTION_BLOCK
"#;
    let (db, file) = setup(source);
    let offset = source.find("A[0]").expect("A usage") as u32;
    let result = hover(&db, file, TextSize::from(offset)).expect("hover");

    assert!(result.contents.contains("CONSTANT"));
    assert!(result.contents.contains("ARRAY[*] OF BYTE"));
}

#[test]
fn test_hover_function_block_member_sections_show_constant_headers_and_array_star() {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_IN_OUT
    CONSTANT Buf : ARRAY[*] OF BYTE;
END_VAR
VAR_TEMP
    CONSTANT T : INT := INT#7;
END_VAR
END_FUNCTION_BLOCK
"#;
    let (db, file) = setup(source);
    let offset = source.find("Fb").expect("fb name") as u32;
    let result = hover(&db, file, TextSize::from(offset)).expect("hover");

    assert!(result.contents.contains("VAR_IN_OUT CONSTANT"));
    assert!(result.contents.contains("Buf : ARRAY[*] OF BYTE;"));
    assert!(result.contents.contains("VAR_TEMP CONSTANT"));
    assert!(result.contents.contains("T : INT := INT#7;"));
}

#[test]
fn test_signature_help_var_in_out_constant_mentions_constant() {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_IN_OUT
    CONSTANT Z : ARRAY[*] OF INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    data : ARRAY[0..2] OF INT;
    fb : Fb;
END_VAR
    fb(|);
END_PROGRAM
"#;
    let cursor = source.find('|').expect("cursor");
    let mut cleaned = source.to_string();
    cleaned.remove(cursor);
    let (db, file) = setup(&cleaned);

    let result = signature_help(&db, file, TextSize::from(cursor as u32)).expect("signature help");
    assert!(result.signatures[0].label.contains("CONSTANT"));
    assert!(result.signatures[0].label.contains("ARRAY[*] OF INT"));
    assert!(result.signatures[0].parameters[0]
        .label
        .contains("CONSTANT"));
}

#[test]
fn test_signature_help_method_var_input_mentions_method_parameters() {
    let source = r#"
FUNCTION_BLOCK Motor
METHOD PUBLIC Start : BOOL
VAR_INPUT
    Var1 : BOOL;
    Var2 : BOOL;
END_VAR
    Start := Var1 AND Var2;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    motor : Motor;
    result : BOOL;
END_VAR
    result := motor.Start(|);
END_PROGRAM
"#;
    let cursor = source.find('|').expect("cursor");
    let mut cleaned = source.to_string();
    cleaned.remove(cursor);
    let (db, file) = setup(&cleaned);

    let result = signature_help(&db, file, TextSize::from(cursor as u32)).expect("signature help");
    assert!(result.signatures[0].label.contains("Start("));
    assert!(result.signatures[0].label.contains("Var1: BOOL"));
    assert!(result.signatures[0].label.contains("Var2: BOOL"));
    assert_eq!(result.active_parameter, 0);
}
