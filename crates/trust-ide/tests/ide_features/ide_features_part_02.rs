use super::*;

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
fn test_rename_refuses_shadow_capture_with_named_reason() {
    let source = r#"
VAR_GLOBAL
    Speed : INT;
END_VAR

PROGRAM Main
VAR
    Temp : INT;
END_VAR
Temp := Speed + 1;
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("Speed : INT").unwrap() as u32);

    let err = trust_ide::rename::rename_checked(&db, file, pos, "Temp")
        .expect_err("rename must refuse reference-site shadow capture");

    assert!(
        matches!(
            err,
            trust_ide::rename::RenameError::ReferenceSiteRebind { .. }
        ),
        "expected ReferenceSiteRebind, got {err:?}"
    );
}

#[test]
fn test_rename_from_imported_use_checks_origin_conflicts_with_named_reason() {
    let globals_source = r#"
VAR_GLOBAL
    Speed : INT;
    Limit : INT;
END_VAR
"#;
    let main_source = r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
Value := Speed;
END_PROGRAM
"#;
    let mut db = Database::new();
    let globals_file = FileId(0);
    let main_file = FileId(1);
    db.set_source_text(globals_file, globals_source.to_string());
    db.set_source_text(main_file, main_source.to_string());

    let origin_pos = TextSize::from(globals_source.find("Speed : INT").unwrap() as u32);
    let origin_err = trust_ide::rename::rename_checked(&db, globals_file, origin_pos, "Limit")
        .expect_err("origin-file rename must refuse duplicate global name");
    assert!(
        matches!(
            origin_err,
            trust_ide::rename::RenameError::DeclaringScopeConflict { .. }
        ),
        "expected DeclaringScopeConflict from origin file, got {origin_err:?}"
    );

    let imported_use_pos = TextSize::from(main_source.find("Speed;").unwrap() as u32);
    let imported_err = trust_ide::rename::rename_checked(&db, main_file, imported_use_pos, "Limit")
        .expect_err("rename from imported use must still check origin conflicts");
    assert!(
        matches!(
            imported_err,
            trust_ide::rename::RenameError::CrossFileImportedSymbolConflict { .. }
        ),
        "expected CrossFileImportedSymbolConflict from imported use, got {imported_err:?}"
    );
}

#[test]
fn test_rename_refuses_case_insensitive_declaring_scope_conflict() {
    let source = r#"
VAR_GLOBAL
    Speed : INT;
    Limit : INT;
END_VAR
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("Speed : INT").unwrap() as u32);

    let err = trust_ide::rename::rename_checked(&db, file, pos, "lImIt")
        .expect_err("IEC identifiers are case-insensitive, so Limit must conflict with lImIt");

    assert!(
        matches!(
            err,
            trust_ide::rename::RenameError::DeclaringScopeConflict { .. }
        ),
        "expected case-insensitive DeclaringScopeConflict, got {err:?}"
    );
}

#[test]
fn test_rename_allows_case_only_change_for_same_symbol() {
    let source = r#"
PROGRAM Main
VAR
    Speed : INT;
END_VAR
Speed := 1;
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("Speed : INT").unwrap() as u32);

    let result = trust_ide::rename::rename_checked(&db, file, pos, "SPEED")
        .expect("a case-only rename of the same IEC symbol must remain valid");

    assert_eq!(result.edit_count(), 2);
    assert!(result
        .edits
        .values()
        .flatten()
        .all(|edit| edit.new_text == "SPEED"));
}

#[test]
fn test_rename_refuses_case_insensitive_struct_field_collision() {
    let source = r#"
TYPE MotorState : STRUCT
    Speed : DINT;
    Limit : DINT;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    State : MotorState;
END_VAR
State.Speed := 1;
END_PROGRAM
"#;
    let (db, file) = setup(source);
    let pos = TextSize::from(source.find("Speed : DINT").unwrap() as u32);

    let err = trust_ide::rename::rename_checked(&db, file, pos, "lImIt")
        .expect_err("a field rename must not collide with a case-equivalent sibling field");

    assert!(
        matches!(
            err,
            trust_ide::rename::RenameError::DeclaringScopeConflict { .. }
        ),
        "expected field DeclaringScopeConflict, got {err:?}"
    );
}

#[test]
fn test_rename_refuses_project_wide_pou_collision() {
    let primary_source = r#"
FUNCTION_BLOCK LevelController
END_FUNCTION_BLOCK
"#;
    let conflicting_source = r#"
FUNCTION_BLOCK BackupController
END_FUNCTION_BLOCK
"#;
    let usage_source = r#"
PROGRAM Main
VAR
    Controller : LevelController;
END_VAR
END_PROGRAM
"#;
    let mut db = Database::new();
    let primary_file = FileId(0);
    let conflicting_file = FileId(1);
    let usage_file = FileId(2);
    db.set_source_text(primary_file, primary_source.to_string());
    db.set_source_text(conflicting_file, conflicting_source.to_string());
    db.set_source_text(usage_file, usage_source.to_string());

    let pos = TextSize::from(primary_source.find("LevelController").unwrap() as u32);
    let err = trust_ide::rename::rename_checked(&db, primary_file, pos, "backupcontroller")
        .expect_err("a top-level POU rename must check the merged project symbol table");

    assert!(
        matches!(
            err,
            trust_ide::rename::RenameError::DeclaringScopeConflict { .. }
        ),
        "expected project-wide DeclaringScopeConflict, got {err:?}"
    );
}

#[test]
fn test_rename_refuses_cross_file_reference_capture() {
    let globals_source = r#"
VAR_GLOBAL
    Speed : INT;
END_VAR
"#;
    let main_source = r#"
PROGRAM Main
VAR
    Temp : INT;
    Observed : INT;
END_VAR
Observed := Speed;
END_PROGRAM
"#;
    let mut db = Database::new();
    let globals_file = FileId(0);
    let main_file = FileId(1);
    db.set_source_text(globals_file, globals_source.to_string());
    db.set_source_text(main_file, main_source.to_string());

    let pos = TextSize::from(globals_source.find("Speed : INT").unwrap() as u32);
    let err = trust_ide::rename::rename_checked(&db, globals_file, pos, "Temp")
        .expect_err("a cross-file reference must not be rebound to a use-site local");

    assert!(
        matches!(
            err,
            trust_ide::rename::RenameError::ReferenceSiteRebind { file_id, .. }
                if file_id == main_file
        ),
        "expected cross-file ReferenceSiteRebind, got {err:?}"
    );
}
