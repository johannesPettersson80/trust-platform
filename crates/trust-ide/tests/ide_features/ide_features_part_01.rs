use super::*;

#[test]
fn test_completion_top_level() {
    // Use a source with just whitespace to get top-level context
    let source = "   ";
    let (db, file) = setup(source);
    let completions = complete(&db, file, TextSize::from(0));

    // At top level (outside POU), should have statement keywords in general context
    // or POU keywords if detected as top level
    // The completion should return some items
    assert!(
        !completions.is_empty(),
        "Should have some completions at top level"
    );
}

#[test]
fn test_completion_type_annotation() {
    let source = "PROGRAM Test VAR x : END_VAR END_PROGRAM";
    let (db, file) = setup(source);
    // Position after ": "
    let pos = TextSize::from(source.find(": ").unwrap() as u32 + 2);
    let completions = complete(&db, file, pos);

    // Should have type keywords
    assert!(
        completions.iter().any(|c| c.label == "INT"),
        "Should have INT type in type annotation context"
    );
    assert!(
        completions.iter().any(|c| c.label == "BOOL"),
        "Should have BOOL type in type annotation context"
    );
}

#[test]
fn test_completion_statement_context() {
    let source = "PROGRAM Test\n\nEND_PROGRAM";
    let (db, file) = setup(source);
    // Position inside program body
    let pos = TextSize::from(source.find("\n\n").unwrap() as u32 + 1);
    let completions = complete(&db, file, pos);

    // Should have statement keywords
    assert!(
        completions.iter().any(|c| c.label == "IF"),
        "Should have IF keyword in statement context"
    );
    assert!(
        completions.iter().any(|c| c.label == "FOR"),
        "Should have FOR keyword in statement context"
    );
    assert!(
        completions.iter().any(|c| c.label == "VAR"),
        "Should have VAR snippet in statement context"
    );
}

#[test]
fn test_completion_includes_symbols() {
    let source = r#"PROGRAM Test
    VAR myVar : INT; END_VAR

END_PROGRAM"#;
    let (db, file) = setup(source);
    // Position after the VAR block
    let pos = TextSize::from(source.find("END_VAR").unwrap() as u32 + 12);
    let completions = complete(&db, file, pos);

    // Should include the variable
    assert!(
        completions.iter().any(|c| c.label == "myVar"),
        "Should include declared variable in completions"
    );
}

#[test]
fn test_completion_member_access_struct_fields() {
    let source = r#"
TYPE
    ST_Cmd : STRUCT
        Enable : BOOL;
        TargetSpeed : REAL;
    END_STRUCT;
END_TYPE

PROGRAM Test
VAR
    Cmd : ST_Cmd;
END_VAR

Cmd.
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("Cmd.").unwrap() as u32 + 4);
    let completions = complete(&db, file, pos);

    assert!(
        completions.iter().any(|c| c.label == "Enable"),
        "Should include struct field Enable after member access"
    );
    assert!(
        completions.iter().any(|c| c.label == "TargetSpeed"),
        "Should include struct field TargetSpeed after member access"
    );
}

// =============================================================================
// Go To Definition Tests
// =============================================================================

#[test]
fn test_goto_definition_struct_in_namespace() {
    let source = r#"
NAMESPACE Demo
TYPE
    Payload : STRUCT
        value : INT;
    END_STRUCT;
    Alias : Payload;
END_TYPE
END_NAMESPACE
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("Payload;").unwrap() as u32);
    let result = goto_definition(&db, file, pos).expect("definition");
    let expected = source.find("Payload : STRUCT").unwrap() as u32;
    assert_eq!(result.range.start(), TextSize::from(expected));
}

#[test]
fn test_goto_definition_boundary_positions_for_typed_literal_and_local_var() {
    let source = r#"
TYPE
    E_State : (Idle := 0, Running := 1);
END_TYPE

PROGRAM Main
VAR
    x : INT;
    s : E_State;
END_VAR

s := E_State#Idle;
x := x + 1;
END_PROGRAM
"#;
    let (db, file) = setup(source);

    let enum_hash_pos = TextSize::from(source.find("E_State#Idle").unwrap() as u32 + 7);
    let enum_def = goto_definition(&db, file, enum_hash_pos).expect("enum definition");
    let enum_decl = source.find("E_State : (").unwrap() as u32;
    assert_eq!(
        enum_def.range.start(),
        TextSize::from(enum_decl),
        "cursor on '#' boundary should still resolve enum definition"
    );

    let local_boundary_pos = TextSize::from(source.find("x + 1").unwrap() as u32 + 1);
    let local_def =
        goto_definition(&db, file, local_boundary_pos).expect("local variable definition");
    let local_decl = source.find("x : INT").unwrap() as u32;
    assert_eq!(
        local_def.range.start(),
        TextSize::from(local_decl),
        "cursor at local symbol boundary should resolve declaration"
    );
}

// =============================================================================
// Go To Implementation Tests
// =============================================================================

#[test]
fn test_goto_implementation_interface() {
    let source = r#"
INTERFACE ICounter
    METHOD Next : INT;
END_INTERFACE

FUNCTION_BLOCK Counter IMPLEMENTS ICounter
    METHOD Next : INT
        RETURN;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("ICounter").unwrap() as u32);
    let results = goto_implementation(&db, file, pos);
    assert!(!results.is_empty(), "expected implementation results");
    let impl_start = source.find("Counter IMPLEMENTS").unwrap() as u32;
    assert!(results
        .iter()
        .any(|res| res.range.start() == TextSize::from(impl_start)));
}

// =============================================================================
// References Tests
// =============================================================================

#[test]
fn test_references_simple_variable() {
    let source = r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := 1;
    x := x + 1;
END_PROGRAM
"#;
    let (db, file) = setup(source);
    // Position on the first 'x' in VAR block
    let pos = TextSize::from(source.find("x : INT").unwrap() as u32);

    let refs = find_references(
        &db,
        file,
        pos,
        FindReferencesOptions {
            include_declaration: true,
        },
    );

    // Should find declaration + usages
    assert!(refs.len() >= 2, "Should find multiple references to x");
}

// =============================================================================
// Action Reference Tests (CODESYS textual action vendor extension)
// =============================================================================

fn action_program_source() -> &'static str {
    r#"
PROGRAM Main
    VAR count : INT; END_VAR
    ACTION Reset
        count := 0;
    END_ACTION
    Reset();
    Reset();
END_PROGRAM
"#
}

#[test]
fn test_references_find_action_call_sites() {
    let source = action_program_source();
    let (db, file) = setup(source);
    let action_decl = source.find("ACTION Reset").unwrap() + "ACTION ".len();
    let refs = find_references(
        &db,
        file,
        TextSize::from(action_decl as u32),
        FindReferencesOptions {
            include_declaration: true,
        },
    );

    assert_eq!(
        refs.len(),
        3,
        "declaration + two bare-name call sites should resolve to the action, got {refs:?}"
    );
}

#[test]
fn test_rename_action_updates_call_sites() {
    let source = action_program_source();
    let (db, file) = setup(source);
    let action_decl = source.find("ACTION Reset").unwrap() + "ACTION ".len();
    let renamed = rename(&db, file, TextSize::from(action_decl as u32), "ResetAll")
        .expect("rename should resolve for an action declaration");

    let mut edits: Vec<_> = renamed
        .edits
        .values()
        .flat_map(|file_edits| file_edits.iter())
        .collect();
    edits.sort_by_key(|edit| (u32::from(edit.range.start()), u32::from(edit.range.end())));
    assert!(
        edits.len() >= 3,
        "declaration and both call sites must be renamed, got {edits:?}"
    );

    // Apply back-to-front so ranges stay valid.
    let mut edited = source.to_string();
    for edit in edits.iter().rev() {
        let start = u32::from(edit.range.start()) as usize;
        let end = u32::from(edit.range.end()) as usize;
        edited.replace_range(start..end, &edit.new_text);
    }
    assert!(
        edited.contains("ResetAll();"),
        "call sites renamed, got:\n{edited}"
    );
    assert!(
        !edited.contains("Reset();"),
        "stale call sites remain:\n{edited}"
    );
}

#[test]
fn test_action_call_site_resolves_to_action_symbol() {
    let source = action_program_source();
    let (db, file) = setup(source);
    let call_site = source.find("Reset();").unwrap();
    let name = trust_ide::symbol_name_at_position(&db, file, TextSize::from(call_site as u32));
    assert_eq!(
        name.as_deref(),
        Some("Reset"),
        "a bare action call must resolve to the action symbol"
    );
}
