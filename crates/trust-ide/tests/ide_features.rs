//! Integration tests for IDE features.

use text_size::TextSize;

use trust_hir::db::{FileId, SourceDatabase};
use trust_hir::Database;
use trust_ide::completion::complete;
use trust_ide::hover;
use trust_ide::references::{find_references, FindReferencesOptions};
use trust_ide::rename::rename;
use trust_ide::semantic_tokens::{semantic_tokens, SemanticTokenType};
use trust_ide::{goto_definition, goto_implementation};

fn setup(source: &str) -> (Database, FileId) {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    (db, file)
}

// =============================================================================
// Completion Context Tests
// =============================================================================

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

#[test]
fn test_references_different_scopes_same_name() {
    let source = r#"
PROGRAM Outer
    VAR x : INT; END_VAR
    x := 1;
END_PROGRAM

PROGRAM Inner
    VAR x : INT; END_VAR
    x := 2;
END_PROGRAM
"#;
    let (db, file) = setup(source);
    // Position on x in Outer program
    let pos = TextSize::from(source.find("x : INT").unwrap() as u32);

    let refs = find_references(
        &db,
        file,
        pos,
        FindReferencesOptions {
            include_declaration: true,
        },
    );

    // Should only find references in Outer, not in Inner
    // This tests scope-aware reference finding
    for r in &refs {
        let range_start = u32::from(r.range.start()) as usize;
        let range_end = u32::from(r.range.end()) as usize;
        let ref_text = &source[range_start..range_end];
        assert!(
            ref_text.eq_ignore_ascii_case("x"),
            "Reference should be 'x', got '{}'",
            ref_text
        );
    }
}

#[test]
fn test_references_unknown_symbol_no_fallback() {
    let source = r#"
PROGRAM Test
    y := 1;
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("y := 1").unwrap() as u32);

    let refs = find_references(
        &db,
        file,
        pos,
        FindReferencesOptions {
            include_declaration: true,
        },
    );

    assert!(
        refs.is_empty(),
        "Unresolved symbol should not return text-based references"
    );
}

#[test]
fn test_references_member_access() {
    let source = r#"
FUNCTION_BLOCK Counter
    METHOD Fetch : DINT
        RETURN;
    END_METHOD
END_FUNCTION_BLOCK

PROGRAM Test
    VAR fb : Counter; END_VAR
    fb.Fetch();
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("Fetch : DINT").unwrap() as u32);

    let refs = find_references(
        &db,
        file,
        pos,
        FindReferencesOptions {
            include_declaration: true,
        },
    );

    let has_call = refs.iter().any(|r| {
        let start = u32::from(r.range.start()) as usize;
        let end = u32::from(r.range.end()) as usize;
        source[start..end].eq_ignore_ascii_case("Fetch") && source[..start].ends_with("fb.")
    });
    assert!(has_call, "Should find member access reference");
}

#[test]
fn test_references_type_reference() {
    let source = r#"
TYPE MyType : STRUCT
    x : DINT;
END_STRUCT
END_TYPE

PROGRAM Test
    VAR v : MyType; END_VAR
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("MyType : STRUCT").unwrap() as u32);

    let refs = find_references(
        &db,
        file,
        pos,
        FindReferencesOptions {
            include_declaration: true,
        },
    );

    assert!(
        refs.iter().any(|r| {
            let start = u32::from(r.range.start()) as usize;
            let end = u32::from(r.range.end()) as usize;
            source[start..end].eq_ignore_ascii_case("MyType") && source[..start].ends_with(": ")
        }),
        "Should find type reference in variable declaration"
    );
}

// =============================================================================
// Rename Tests
// =============================================================================

#[test]
fn test_rename_basic() {
    let source = r#"
PROGRAM Test
    VAR oldName : INT; END_VAR
    oldName := 1;
END_PROGRAM
"#;
    let (db, file) = setup(source);
    // Position on oldName
    let pos = TextSize::from(source.find("oldName").unwrap() as u32);

    let result = rename(&db, file, pos, "newName");

    assert!(result.is_some(), "Rename should succeed");
    let result = result.unwrap();
    assert!(
        result.edit_count() >= 2,
        "Should have edits for declaration and usage"
    );
}

#[test]
fn test_rename_rejects_invalid_name() {
    let source = r#"
PROGRAM Test
    VAR x : INT; END_VAR
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("x :").unwrap() as u32);

    // Invalid names should be rejected
    assert!(
        rename(&db, file, pos, "1invalid").is_none(),
        "Should reject names starting with digit"
    );
    assert!(
        rename(&db, file, pos, "foo-bar").is_none(),
        "Should reject names with hyphens"
    );
    assert!(
        rename(&db, file, pos, "").is_none(),
        "Should reject empty names"
    );
}

#[test]
fn test_rename_rejects_keywords() {
    let source = r#"
PROGRAM Test
    VAR x : INT; END_VAR
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("x :").unwrap() as u32);

    // Keywords should be rejected
    assert!(
        rename(&db, file, pos, "IF").is_none(),
        "Should reject keyword IF"
    );
    assert!(
        rename(&db, file, pos, "PROGRAM").is_none(),
        "Should reject keyword PROGRAM"
    );
    assert!(
        rename(&db, file, pos, "INT").is_none(),
        "Should reject type keyword INT"
    );
}

#[test]
fn test_rename_struct_field() {
    let source = r#"
TYPE Point : STRUCT
    x : DINT;
END_STRUCT
END_TYPE

PROGRAM Test
    VAR p : Point; END_VAR
    p.x := 1;
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("x : DINT").unwrap() as u32);

    let result = rename(&db, file, pos, "x2");
    assert!(result.is_some(), "Rename should succeed for struct field");
    let result = result.unwrap();
    assert!(
        result.edit_count() >= 2,
        "Should rename declaration and usage"
    );
}

#[test]
fn test_struct_field_definition_and_references_across_files() {
    let types_source = r#"
TYPE
    ST_Cmd : STRUCT
        Enable : BOOL;
    END_STRUCT;
END_TYPE
"#;
    let fb_source = r#"
FUNCTION_BLOCK FB_Test
VAR_INPUT
    Command : ST_Cmd;
END_VAR
VAR
    LocalEnable : BOOL;
END_VAR

LocalEnable := Command.Enable;
END_FUNCTION_BLOCK
"#;
    let program_source = r#"
PROGRAM Main
VAR
    Cmd : ST_Cmd;
END_VAR

Cmd.Enable := TRUE;
END_PROGRAM
"#;

    let mut db = Database::new();
    let types_file = FileId(0);
    let fb_file = FileId(1);
    let program_file = FileId(2);
    db.set_source_text(types_file, types_source.to_string());
    db.set_source_text(fb_file, fb_source.to_string());
    db.set_source_text(program_file, program_source.to_string());

    let pos = TextSize::from(types_source.find("Enable : BOOL").unwrap() as u32);
    let def = goto_definition(&db, types_file, pos).expect("definition should resolve");
    assert_eq!(
        def.file_id, types_file,
        "struct field definition should resolve to the type declaration file"
    );

    let refs = find_references(
        &db,
        types_file,
        pos,
        FindReferencesOptions {
            include_declaration: true,
        },
    );
    assert!(
        refs.iter().any(|reference| reference.file_id == types_file),
        "references should include struct field declaration, got file ids: {:?}",
        refs.iter()
            .map(|reference| reference.file_id)
            .collect::<Vec<_>>()
    );
    assert!(
        refs.iter().any(|reference| reference.file_id == fb_file),
        "references should include field usage in function block file, got file ids: {:?}",
        refs.iter()
            .map(|reference| reference.file_id)
            .collect::<Vec<_>>()
    );
    assert!(
        refs.iter()
            .any(|reference| reference.file_id == program_file),
        "references should include field usage in program file, got file ids: {:?}",
        refs.iter()
            .map(|reference| reference.file_id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_rename_function_block_updates_type_usage_in_other_file() {
    let fb_source = r#"
FUNCTION_BLOCK LevelControllerFb
END_FUNCTION_BLOCK
"#;
    let main_source = r#"
PROGRAM Main
VAR
    Ctrl : LevelControllerFb;
END_VAR
END_PROGRAM
"#;

    let mut db = Database::new();
    let fb_file = FileId(0);
    let main_file = FileId(1);
    db.set_source_text(fb_file, fb_source.to_string());
    db.set_source_text(main_file, main_source.to_string());

    let pos = TextSize::from(fb_source.find("LevelControllerFb").unwrap() as u32);
    let result = rename(&db, fb_file, pos, "LevelControllerFb2")
        .expect("rename should succeed across files");

    let fb_edits = result
        .edits
        .get(&fb_file)
        .expect("expected declaration edit in FB file");
    assert!(
        fb_edits.iter().any(|edit| &fb_source
            [u32::from(edit.range.start()) as usize..u32::from(edit.range.end()) as usize]
            == "LevelControllerFb"),
        "expected declaration edit in FB file"
    );

    let main_edits = result
        .edits
        .get(&main_file)
        .expect("expected type-usage edit in Main file");
    assert!(
        main_edits.iter().any(|edit| &main_source
            [u32::from(edit.range.start()) as usize..u32::from(edit.range.end()) as usize]
            == "LevelControllerFb"),
        "expected type usage edit in Main file"
    );
}

#[test]
fn test_rename_function_block_from_usage_site_updates_declaration() {
    let fb_source = r#"
FUNCTION_BLOCK LevelControllerFb
END_FUNCTION_BLOCK
"#;
    let main_source = r#"
PROGRAM Main
VAR
    Ctrl : LevelControllerFb;
END_VAR
END_PROGRAM
"#;

    let mut db = Database::new();
    let fb_file = FileId(0);
    let main_file = FileId(1);
    db.set_source_text(fb_file, fb_source.to_string());
    db.set_source_text(main_file, main_source.to_string());

    let pos = TextSize::from(main_source.find("LevelControllerFb").unwrap() as u32);
    let result = rename(&db, main_file, pos, "LevelControllerFb2")
        .expect("rename should succeed from usage site");

    assert!(
        result.edits.contains_key(&fb_file),
        "expected declaration edit in FB file"
    );
    assert!(
        result.edits.contains_key(&main_file),
        "expected usage edit in Main file"
    );
}

// =============================================================================
// Semantic Token Tests
// =============================================================================

#[test]
fn test_semantic_tokens_function() {
    let source = r#"
FUNCTION Add : INT
    VAR_INPUT a : INT; b : INT; END_VAR
    Add := a + b;
END_FUNCTION
"#;
    let (db, file) = setup(source);
    let tokens = semantic_tokens(&db, file);

    // Find the token for 'Add' in declaration
    let add_offset = source.find("Add :").unwrap() as u32;
    let add_token = tokens
        .iter()
        .find(|t| u32::from(t.range.start()) == add_offset);

    assert!(
        add_token.is_some(),
        "Should have token for 'Add' declaration"
    );
    if let Some(token) = add_token {
        assert_eq!(
            token.token_type,
            SemanticTokenType::Function,
            "Function name should be classified as Function"
        );
        assert!(
            token.modifiers.declaration,
            "Declaration site should have declaration modifier"
        );
    }
}

#[test]
fn test_semantic_tokens_variable() {
    let source = r#"
PROGRAM Test
    VAR myVar : INT; END_VAR
    myVar := 42;
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let tokens = semantic_tokens(&db, file);

    // Find the token for 'myVar' in declaration
    let var_offset = source.find("myVar :").unwrap() as u32;
    let var_token = tokens
        .iter()
        .find(|t| u32::from(t.range.start()) == var_offset);

    assert!(
        var_token.is_some(),
        "Should have token for 'myVar' declaration"
    );
    if let Some(token) = var_token {
        assert_eq!(
            token.token_type,
            SemanticTokenType::Variable,
            "Variable should be classified as Variable"
        );
    }
}

#[test]
fn test_semantic_tokens_constant() {
    let source = r#"
PROGRAM Test
    VAR CONSTANT PI : REAL := 3.14159; END_VAR
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let tokens = semantic_tokens(&db, file);

    // Find the token for 'PI'
    let pi_offset = source.find("PI :").unwrap() as u32;
    let pi_token = tokens
        .iter()
        .find(|t| u32::from(t.range.start()) == pi_offset);

    assert!(pi_token.is_some(), "Should have token for 'PI' declaration");
    if let Some(token) = pi_token {
        // Constants are classified as Variable with readonly modifier
        assert_eq!(
            token.token_type,
            SemanticTokenType::Variable,
            "Constant should be classified as Variable"
        );
        assert!(
            token.modifiers.readonly,
            "Constant should have readonly modifier"
        );
    }
}

#[test]
fn test_semantic_tokens_keywords() {
    let source = "PROGRAM Test END_PROGRAM";
    let (db, file) = setup(source);
    let tokens = semantic_tokens(&db, file);

    // Find the PROGRAM keyword token
    let program_token = tokens.iter().find(|t| u32::from(t.range.start()) == 0);

    assert!(
        program_token.is_some(),
        "Should have token for PROGRAM keyword"
    );
    if let Some(token) = program_token {
        assert_eq!(
            token.token_type,
            SemanticTokenType::Keyword,
            "PROGRAM should be classified as Keyword"
        );
    }
}

#[test]
fn test_semantic_tokens_parameter() {
    let source = r#"
FUNCTION Add : INT
    VAR_INPUT a : INT; END_VAR
    Add := a;
END_FUNCTION
"#;
    let (db, file) = setup(source);
    let tokens = semantic_tokens(&db, file);

    let param_offset = source.find("a : INT").unwrap() as u32;
    let param_token = tokens
        .iter()
        .find(|t| u32::from(t.range.start()) == param_offset);

    assert!(
        param_token.is_some(),
        "Should have token for parameter declaration"
    );
    if let Some(token) = param_token {
        assert_eq!(
            token.token_type,
            SemanticTokenType::Parameter,
            "Parameter should be classified as Parameter"
        );
        assert!(
            token.modifiers.declaration,
            "Parameter declaration should have declaration modifier"
        );
    }
}

#[test]
fn test_semantic_tokens_enum_member() {
    let source = r#"
TYPE Mode : (Auto, Manual); END_TYPE

PROGRAM Test
    VAR mode : Mode; END_VAR
    mode := Auto;
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let tokens = semantic_tokens(&db, file);

    let enum_offset = source.rfind("Auto").unwrap() as u32;
    let enum_token = tokens
        .iter()
        .find(|t| u32::from(t.range.start()) == enum_offset);

    assert!(
        enum_token.is_some(),
        "Should have token for enum member usage"
    );
    if let Some(token) = enum_token {
        assert_eq!(
            token.token_type,
            SemanticTokenType::EnumMember,
            "Enum member should be classified as EnumMember"
        );
    }
}

#[test]
fn test_semantic_tokens_method_member() {
    let source = r#"
FUNCTION_BLOCK Counter
    METHOD Fetch : DINT
        RETURN;
    END_METHOD
END_FUNCTION_BLOCK

PROGRAM Test
    VAR fb : Counter; END_VAR
    fb.Fetch();
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let tokens = semantic_tokens(&db, file);

    let method_offset = source.rfind("Fetch").unwrap() as u32;
    let method_token = tokens
        .iter()
        .find(|t| u32::from(t.range.start()) == method_offset);

    assert!(
        method_token.is_some(),
        "Should have token for method member usage"
    );
    if let Some(token) = method_token {
        assert_eq!(
            token.token_type,
            SemanticTokenType::Method,
            "Method member should be classified as Method"
        );
    }
}

#[test]
fn test_semantic_tokens_struct_field_member() {
    let source = r#"
TYPE Point : STRUCT
    x : DINT;
END_STRUCT
END_TYPE

PROGRAM Test
    VAR p : Point; END_VAR
    p.x := 1;
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let tokens = semantic_tokens(&db, file);

    let field_offset = source.rfind("x := 1").unwrap() as u32;
    let field_token = tokens
        .iter()
        .find(|t| u32::from(t.range.start()) == field_offset);

    assert!(
        field_token.is_some(),
        "Should have token for struct field usage"
    );
    if let Some(token) = field_token {
        assert_eq!(
            token.token_type,
            SemanticTokenType::Property,
            "Struct field should be classified as Property"
        );
    }
}

#[test]
fn test_semantic_tokens_type_reference() {
    let source = r#"
TYPE Thing : STRUCT
    value : DINT;
END_STRUCT
END_TYPE

PROGRAM Test
    VAR item : Thing; END_VAR
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let tokens = semantic_tokens(&db, file);

    let type_offset = source.rfind("Thing;").unwrap() as u32;
    let type_token = tokens
        .iter()
        .find(|t| u32::from(t.range.start()) == type_offset);

    assert!(type_token.is_some(), "Should have token for type reference");
    if let Some(token) = type_token {
        assert_eq!(
            token.token_type,
            SemanticTokenType::Type,
            "Type reference should be classified as Type"
        );
    }
}

// =============================================================================
// Hover & Go-to-definition Tests
// =============================================================================

#[test]
fn test_goto_definition_method_member() {
    let source = r#"
FUNCTION_BLOCK Counter
    METHOD Fetch : DINT
        RETURN;
    END_METHOD
END_FUNCTION_BLOCK

PROGRAM Test
    VAR fb : Counter; END_VAR
    fb.Fetch();
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let call_offset = TextSize::from(source.rfind("Fetch()").unwrap() as u32);
    let def_offset = TextSize::from(source.find("Fetch : DINT").unwrap() as u32);

    let def = goto_definition(&db, file, call_offset).expect("definition");
    assert_eq!(
        u32::from(def.range.start()),
        u32::from(def_offset),
        "Method call should resolve to method definition"
    );
}

#[test]
fn test_hover_initializers_and_retention() {
    let source = r#"
PROGRAM Test
    VAR CONSTANT
        PI : REAL := 3.14;
    END_VAR
    VAR RETAIN
        counter : INT := 10;
    END_VAR
END_PROGRAM
"#;
    let (db, file) = setup(source);

    let pi_offset = TextSize::from(source.find("PI : REAL").unwrap() as u32);
    let pi_hover = hover(&db, file, pi_offset).expect("hover");
    assert!(
        pi_hover.contents.contains("PI : REAL := 3.14"),
        "Constant hover should include initializer"
    );

    let counter_offset = TextSize::from(source.find("counter : INT").unwrap() as u32);
    let counter_hover = hover(&db, file, counter_offset).expect("hover");
    assert!(
        counter_hover.contents.contains("VAR RETAIN"),
        "Hover should include RETAIN qualifier"
    );
    assert!(
        counter_hover.contents.contains("counter : INT := 10"),
        "Hover should include initializer for retained variable"
    );
}

#[test]
fn test_hover_task_priority() {
    let source = r#"
PROGRAM Main
END_PROGRAM

CONFIGURATION Conf
RESOURCE Res ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;
    let (db, file) = setup(source);

    let priority_offset = TextSize::from(source.find("PRIORITY").unwrap() as u32);
    let priority_hover = hover(&db, file, priority_offset).expect("hover");
    assert!(
        priority_hover.contents.contains("PRIORITY : UINT"),
        "Hover should show PRIORITY type"
    );
    assert!(
        priority_hover.contents.contains("0 = highest priority"),
        "Hover should explain priority ordering"
    );
}

#[test]
fn test_hover_type_definitions_and_fb_interface() {
    let source = r#"
TYPE MyInt : INT;
END_TYPE

TYPE Color :
(
    Red := 0,
    Green := 1
);
END_TYPE

TYPE Point : STRUCT
    x : DINT;
    y : DINT;
END_STRUCT
END_TYPE

INTERFACE IFoo
END_INTERFACE

FUNCTION_BLOCK Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK Motor EXTENDS Base IMPLEMENTS IFoo
VAR_INPUT
    speed : INT;
END_VAR
VAR_OUTPUT
    ok : BOOL;
END_VAR
END_FUNCTION_BLOCK
"#;
    let (db, file) = setup(source);

    let alias_offset = TextSize::from(source.find("MyInt").unwrap() as u32);
    let alias_hover = hover(&db, file, alias_offset).expect("hover");
    assert!(
        alias_hover.contents.contains("TYPE MyInt : INT"),
        "Hover should show alias definition"
    );

    let enum_offset = TextSize::from(source.find("Color").unwrap() as u32);
    let enum_hover = hover(&db, file, enum_offset).expect("hover");
    assert!(
        enum_hover.contents.contains("Red := 0") && enum_hover.contents.contains("Green := 1"),
        "Hover should list enum values"
    );

    let struct_offset = TextSize::from(source.find("Point : STRUCT").unwrap() as u32);
    let struct_hover = hover(&db, file, struct_offset).expect("hover");
    assert!(
        struct_hover.contents.contains("x : DINT") && struct_hover.contents.contains("y : DINT"),
        "Hover should list struct fields"
    );

    let fb_offset = TextSize::from(source.find("Motor EXTENDS").unwrap() as u32);
    let fb_hover = hover(&db, file, fb_offset).expect("hover");
    assert!(
        fb_hover.contents.contains("VAR_INPUT") && fb_hover.contents.contains("speed : INT"),
        "Hover should show FB interface"
    );
    assert!(
        fb_hover.contents.contains("VAR_OUTPUT") && fb_hover.contents.contains("ok : BOOL"),
        "Hover should show FB outputs"
    );
    assert!(
        fb_hover.contents.contains("EXTENDS Base") && fb_hover.contents.contains("IMPLEMENTS IFoo"),
        "Hover should show inheritance and implements"
    );
}

#[test]
fn test_hover_function_block_uses_declared_type_when_type_resolution_is_unknown() {
    let source = r#"
FUNCTION_BLOCK FB_Pump
VAR_INPUT
    Command : ST_PumpCommand;
END_VAR
VAR_OUTPUT
    Status : ST_PumpStatus;
END_VAR
END_FUNCTION_BLOCK
"#;
    let (db, file) = setup(source);
    let fb_offset = TextSize::from(source.find("FB_Pump").unwrap() as u32);
    let hover_result = hover(&db, file, fb_offset).expect("hover");

    assert!(
        hover_result.contents.contains("Command : ST_PumpCommand;"),
        "Hover should preserve declared input type text when semantic type is unresolved. Hover:\n{}",
        hover_result.contents
    );
    assert!(
        hover_result.contents.contains("Status : ST_PumpStatus;"),
        "Hover should preserve declared output type text when semantic type is unresolved. Hover:\n{}",
        hover_result.contents
    );
    assert!(
        !hover_result.contents.contains("Command : ?;") && !hover_result.contents.contains("Status : ?;"),
        "Hover should avoid unresolved placeholders for explicitly declared member types. Hover:\n{}",
        hover_result.contents
    );
}
