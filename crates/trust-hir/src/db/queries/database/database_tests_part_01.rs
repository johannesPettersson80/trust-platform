    use super::*;
    use parking_lot::RwLock;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::thread;

    fn install_cross_file_fixture(db: &mut Database) -> (FileId, FileId) {
        let file_lib = FileId(10);
        let file_main = FileId(11);
        db.set_source_text(
            file_lib,
            "FUNCTION AddOne : INT\nVAR_INPUT\n    x : INT;\nEND_VAR\nAddOne := x + 1;\nEND_FUNCTION\n"
                .to_string(),
        );
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := AddOne(1);\nEND_PROGRAM\n"
                .to_string(),
        );
        (file_lib, file_main)
    }

    fn install_diagnostics_fixture(db: &mut Database) -> FileId {
        let file_lib = FileId(20);
        let file_main = FileId(21);
        db.set_source_text(
            file_lib,
            "FUNCTION AddOne : INT\nVAR_INPUT\n    x : INT;\nEND_VAR\nAddOne := x + 1;\nEND_FUNCTION\n"
                .to_string(),
        );
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := AddOne(TRUE);\nEND_PROGRAM\n"
                .to_string(),
        );
        file_main
    }

    fn install_cross_file_global_struct_fixture(db: &mut Database) -> (FileId, FileId, FileId) {
        let file_types = FileId(40);
        let file_globals = FileId(41);
        let file_main = FileId(42);
        db.set_source_text(
            file_types,
            "TYPE CARRIER :\nSTRUCT\n    A : INT;\nEND_STRUCT\nEND_TYPE\n".to_string(),
        );
        db.set_source_text(
            file_globals,
            "VAR_GLOBAL\n    G : CARRIER;\nEND_VAR\n".to_string(),
        );
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    X : INT;\nEND_VAR\nX := G.A;\nEND_PROGRAM\n".to_string(),
        );
        (file_types, file_globals, file_main)
    }

    fn install_namespaced_cross_file_global_struct_fixture(
        db: &mut Database,
    ) -> (FileId, FileId, FileId) {
        let file_types = FileId(43);
        let file_globals = FileId(44);
        let file_main = FileId(45);
        db.set_source_text(
            file_types,
            "NAMESPACE Lib\nTYPE CARRIER :\nSTRUCT\n    A : INT;\nEND_STRUCT\nEND_TYPE\nEND_NAMESPACE\n"
                .to_string(),
        );
        db.set_source_text(
            file_globals,
            "USING Lib;\nVAR_GLOBAL\n    G : CARRIER;\nEND_VAR\n".to_string(),
        );
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    X : INT;\nEND_VAR\nX := G.A;\nEND_PROGRAM\n".to_string(),
        );
        (file_types, file_globals, file_main)
    }

    fn install_cross_file_function_block_fixture(db: &mut Database) -> (FileId, FileId) {
        let file_lib = FileId(49);
        let file_main = FileId(50);
        db.set_source_text(
            file_lib,
            "FUNCTION_BLOCK FB_Accumulator\nVAR_INPUT\n    In : INT;\nEND_VAR\nVAR_OUTPUT\n    Sum : INT;\nEND_VAR\nSum := Sum + In;\nEND_FUNCTION_BLOCK\n"
                .to_string(),
        );
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    acc : FB_Accumulator;\nEND_VAR\nEND_PROGRAM\n".to_string(),
        );
        (file_lib, file_main)
    }

    fn install_cross_file_root_type_reference_fixture(db: &mut Database) -> (FileId, FileId) {
        let file_types = FileId(51);
        let file_main = FileId(52);
        db.set_source_text(
            file_types,
            "TYPE Color : (Red, Blue);\nEND_TYPE\n".to_string(),
        );
        db.set_source_text(
            file_main,
            "PROGRAM MainProg\nVAR\n    color : Color;\nEND_VAR\nEND_PROGRAM\n".to_string(),
        );
        (file_types, file_main)
    }

    fn install_cross_file_struct_member_access_fixture(db: &mut Database) -> (FileId, FileId) {
        let file_types = FileId(53);
        let file_fb = FileId(54);
        db.set_source_text(
            file_types,
            "TYPE\n    E_State : (Idle := 0, Running := 1);\n    ST_Command :\n    STRUCT\n        Enable : BOOL;\n    END_STRUCT;\n    ST_Status :\n    STRUCT\n        Running : BOOL;\n        State : E_State;\n    END_STRUCT;\nEND_TYPE\n"
                .to_string(),
        );
        db.set_source_text(
            file_fb,
            "FUNCTION_BLOCK FB_Test\nVAR_INPUT\n    Command : ST_Command;\nEND_VAR\nVAR_OUTPUT\n    Status : ST_Status;\nEND_VAR\nStatus.Running := Command.Enable;\nIF Status.State = E_State#Idle THEN\n    Status.Running := FALSE;\nEND_IF\nEND_FUNCTION_BLOCK\n"
                .to_string(),
        );
        (file_types, file_fb)
    }

    fn expr_id_for(db: &Database, file_id: FileId, needle: &str) -> u32 {
        let source = db.source_text(file_id);
        let start = source
            .find(needle)
            .unwrap_or_else(|| panic!("missing needle '{needle}' in source"))
            as u32;
        let end = start + needle.len() as u32;
        db.expr_id_for_range(file_id, start, end)
            .or_else(|| db.expr_id_at_offset(file_id, end.saturating_sub(1)))
            .unwrap_or_else(|| panic!("missing expression id for '{needle}'"))
    }

    #[test]
    fn file_symbols_reuses_unchanged_file_across_unrelated_edit() {
        let mut db = Database::new();
        let file_main = FileId(1);
        let file_aux = FileId(2);

        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    counter : INT;\nEND_VAR\ncounter := counter + 1;\nEND_PROGRAM\n"
                .to_string(),
        );
        db.set_source_text(
            file_aux,
            "PROGRAM Aux\nVAR\n    flag : BOOL;\nEND_VAR\nflag := TRUE;\nEND_PROGRAM\n".to_string(),
        );

        let before = db.file_symbols(file_main);
        db.set_source_text(
            file_aux,
            "PROGRAM Aux\nVAR\n    flag : BOOL;\nEND_VAR\nflag := FALSE;\nEND_PROGRAM\n"
                .to_string(),
        );
        let after = db.file_symbols(file_main);

        assert!(
            Arc::ptr_eq(&before, &after),
            "unchanged file symbols should be reused across unrelated edits"
        );
    }

    #[test]
    fn file_symbols_recomputes_when_its_file_changes() {
        let mut db = Database::new();
        let file = FileId(3);

        db.set_source_text(file, "PROGRAM Main\nEND_PROGRAM\n".to_string());
        let before = db.file_symbols(file);

        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := 42;\nEND_PROGRAM\n".to_string(),
        );
        let after = db.file_symbols(file);

        assert!(
            !Arc::ptr_eq(&before, &after),
            "updated file symbols should not reuse stale analysis"
        );
        assert!(
            after.lookup_any("value").is_some(),
            "updated symbol table should contain new declarations"
        );
    }

    #[test]
    fn analyze_salsa_returns_expected_cross_file_result() {
        let mut db = Database::new();
        let (_file_lib, file_main) = install_cross_file_fixture(&mut db);

        let analysis = db.analyze_salsa(file_main);

        assert!(
            analysis.symbols.lookup_any("AddOne").is_some(),
            "cross-file function should be available in analyzed symbol table"
        );
        assert!(
            analysis
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.is_error()),
            "valid fixture should not emit error diagnostics"
        );
    }

    #[test]
    fn analyze_salsa_accepts_cross_file_global_constants_in_string_lengths() {
        let mut db = Database::new();
        let file_globals = FileId(30);
        let file_main = FileId(31);

        db.set_source_text(
            file_globals,
            "VAR_GLOBAL CONSTANT\n    STRING_LENGTH : INT := INT#12;\nEND_VAR\n".to_string(),
        );
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    s : STRING[STRING_LENGTH];\nEND_VAR\nEND_PROGRAM\n"
                .to_string(),
        );

        let analysis = db.analyze_salsa(file_main);

        assert!(
            analysis
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.is_error()),
            "cross-file constant string length should analyze without errors: {:?}",
            analysis.diagnostics
        );
    }

    #[test]
    fn analyze_salsa_accepts_cross_file_root_global_struct_field_access() {
        let mut db = Database::new();
        let (_file_types, _file_globals, file_main) =
            install_cross_file_global_struct_fixture(&mut db);

        let analysis = db.analyze_salsa(file_main);

        assert!(
            analysis
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.is_error()),
            "cross-file root global struct field access should analyze without errors: {:?}",
            analysis.diagnostics
        );

        let symbol_id = analysis
            .symbols
            .lookup_any("G")
            .expect("global symbol G should resolve");
        let symbol = analysis.symbols.get(symbol_id).expect("G symbol should exist");
        let type_name = analysis
            .symbols
            .type_name(symbol.type_id)
            .expect("G should have a resolved type");
        assert_eq!(type_name.as_str(), "CARRIER");
    }

    #[test]
    fn file_symbols_attach_cross_file_root_global_struct_type_during_collection() {
        let mut db = Database::new();
        let (_file_types, file_globals, _file_main) =
            install_cross_file_global_struct_fixture(&mut db);

        let symbols = db.file_symbols(file_globals);
        let symbol_id = symbols.lookup_any("G").expect("global symbol G should resolve");
        let symbol = symbols.get(symbol_id).expect("G symbol should exist");
        let type_name = symbols
            .type_name(symbol.type_id)
            .expect("G should have a resolved type in file_symbols()");

        assert_eq!(type_name.as_str(), "CARRIER");
    }

    #[test]
    fn analyze_salsa_accepts_namespaced_using_cross_file_root_global_struct_field_access() {
        let mut db = Database::new();
        let (_file_types, _file_globals, file_main) =
            install_namespaced_cross_file_global_struct_fixture(&mut db);

        let analysis = db.analyze_salsa(file_main);

        assert!(
            analysis
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.is_error()),
            "namespaced cross-file root global struct field access should analyze without errors: {:?}",
            analysis.diagnostics
        );

        let symbol_id = analysis
            .symbols
            .lookup_any("G")
            .expect("global symbol G should resolve");
        let symbol = analysis.symbols.get(symbol_id).expect("G symbol should exist");
        let type_name = analysis
            .symbols
            .type_name(symbol.type_id)
            .expect("G should have a resolved type");
        assert_eq!(type_name.as_str(), "LIB.CARRIER");
    }

    #[test]
    fn diagnostics_salsa_reports_duplicate_cross_file_type_declarations() {
        let mut db = Database::new();
        let file_a = FileId(46);
        let file_b = FileId(47);
        let file_main = FileId(48);
        db.set_source_text(
            file_a,
            "TYPE CARRIER :\nSTRUCT\n    A : INT;\nEND_STRUCT\nEND_TYPE\n".to_string(),
        );
        db.set_source_text(
            file_b,
            "TYPE CARRIER :\nSTRUCT\n    A : DINT;\nEND_STRUCT\nEND_TYPE\n".to_string(),
        );
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    X : INT;\nEND_VAR\nX := 1;\nEND_PROGRAM\n".to_string(),
        );

        let diagnostics_a = db.diagnostics_salsa(file_a);
        let diagnostics_b = db.diagnostics_salsa(file_b);

        assert!(
            diagnostics_a.iter().any(|diagnostic| {
                diagnostic.is_error()
                    && diagnostic.message.contains("duplicate type declaration of 'CARRIER'")
            }),
            "first declaration should report duplicate type diagnostics: {:?}",
            diagnostics_a
        );
        assert!(
            diagnostics_b.iter().any(|diagnostic| {
                diagnostic.is_error()
                    && diagnostic.message.contains("duplicate type declaration of 'CARRIER'")
            }),
            "second declaration should report duplicate type diagnostics: {:?}",
            diagnostics_b
        );
    }

    #[test]
    fn analyze_salsa_keeps_cross_file_function_block_body_bound_to_real_pou_scope() {
        let mut db = Database::new();
        let (file_lib, _file_main) = install_cross_file_function_block_fixture(&mut db);

        let analysis = db.analyze_salsa(file_lib);

        assert!(
            analysis
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.is_error()),
            "cross-file FB references must not poison the defining FB scope: {:?}",
            analysis.diagnostics
        );
    }

    #[test]
    fn analyze_salsa_imported_root_type_names_do_not_collide_with_local_variables() {
        let mut db = Database::new();
        let (_file_types, file_main) = install_cross_file_root_type_reference_fixture(&mut db);

        let analysis = db.analyze_salsa(file_main);

        assert!(
            analysis
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.is_error()),
            "imported root types must not be declared into the current POU scope: {:?}",
            analysis.diagnostics
        );

        let symbol_id = analysis
            .symbols
            .lookup_any("color")
            .expect("local variable color should resolve");
        let symbol = analysis
            .symbols
            .get(symbol_id)
            .expect("local variable color should exist");
        let type_name = analysis
            .symbols
            .type_name(symbol.type_id)
            .expect("color should keep its imported type");
        assert!(
            type_name.as_str().eq_ignore_ascii_case("Color"),
            "imported type should still resolve to Color, got {type_name}"
        );
    }

    #[test]
    fn analyze_salsa_cross_file_struct_types_support_member_access_inside_pou_bodies() {
        let mut db = Database::new();
        let (_file_types, file_fb) = install_cross_file_struct_member_access_fixture(&mut db);

        let analysis = db.analyze_salsa(file_fb);

        assert!(
            analysis
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.is_error()),
            "cross-file struct imports must still behave like structs inside POU bodies: {:?}",
            analysis.diagnostics
        );

        let symbol_id = analysis
            .symbols
            .lookup_any("Status")
            .expect("Status output should resolve");
        let symbol = analysis
            .symbols
            .get(symbol_id)
            .expect("Status output should exist");
        let type_name = analysis
            .symbols
            .type_name(symbol.type_id)
            .expect("Status should keep its imported struct type");
        assert!(
            type_name.as_str().eq_ignore_ascii_case("ST_Status"),
            "Status should resolve to ST_Status, got {type_name}"
        );
    }

    #[test]
    fn analyze_salsa_reuses_result_without_edits() {
        let mut db = Database::new();
        let (_file_lib, file_main) = install_cross_file_fixture(&mut db);

        let first = db.analyze_salsa(file_main);
        let second = db.analyze_salsa(file_main);

        assert!(
            Arc::ptr_eq(&first, &second),
            "salsa analyze should reuse cached analysis when inputs are unchanged"
        );
    }

    #[test]
    fn analyze_salsa_recomputes_after_target_edit() {
        let mut db = Database::new();
        let (_file_lib, file_main) = install_cross_file_fixture(&mut db);

        let before = db.analyze_salsa(file_main);
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := AddOne(2);\nEND_PROGRAM\n"
                .to_string(),
        );
        let after = db.analyze_salsa(file_main);

        assert!(
            !Arc::ptr_eq(&before, &after),
            "salsa analyze should invalidate cached analysis when the target file changes"
        );
    }

    #[test]
    fn diagnostics_salsa_reuses_result_without_edits() {
        let mut db = Database::new();
        let file_main = install_diagnostics_fixture(&mut db);

        let first = db.diagnostics_salsa(file_main);
        let second = db.diagnostics_salsa(file_main);

        assert!(
            Arc::ptr_eq(&first, &second),
            "salsa diagnostics should reuse cached diagnostics when inputs are unchanged"
        );
    }

    #[test]
    fn diagnostics_salsa_recomputes_after_target_edit() {
        let mut db = Database::new();
        let file_main = install_diagnostics_fixture(&mut db);

        let before = db.diagnostics_salsa(file_main);
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := AddOne(1);\nEND_PROGRAM\n"
                .to_string(),
        );
        let after = db.diagnostics_salsa(file_main);

        assert!(
            !Arc::ptr_eq(&before, &after),
            "salsa diagnostics should invalidate cached result when the target file changes"
        );
        assert!(
            after.len() < before.len(),
            "fixing invalid call should reduce diagnostics"
        );
    }
