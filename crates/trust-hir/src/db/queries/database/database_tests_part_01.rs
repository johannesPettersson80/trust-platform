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

    fn expr_id_for(db: &Database, file_id: FileId, needle: &str) -> u32 {
        let source = db.source_text(file_id);
        let offset = source
            .find(needle)
            .unwrap_or_else(|| panic!("missing needle '{needle}' in source"))
            as u32;
        db.expr_id_at_offset(file_id, offset)
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
