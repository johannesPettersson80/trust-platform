    #[test]
    fn expr_id_at_offset_returns_none_for_missing_file() {
        let db = Database::new();
        assert!(
            db.expr_id_at_offset(FileId(36), 0).is_none(),
            "missing files should not produce expression ids"
        );
    }

    #[test]
    fn expr_id_at_offset_tracks_updated_source() {
        let mut db = Database::new();
        let file = FileId(37);
        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := 1;\nEND_PROGRAM\n".to_string(),
        );

        let old_offset = db
            .source_text(file)
            .find("1")
            .expect("old literal should exist") as u32;
        assert!(
            db.expr_id_at_offset(file, old_offset).is_some(),
            "initial source should resolve an expression id"
        );

        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := value + 2;\nEND_PROGRAM\n"
                .to_string(),
        );

        let new_offset = db
            .source_text(file)
            .find("value + 2")
            .expect("updated expression should exist") as u32;
        assert!(
            db.expr_id_at_offset(file, new_offset).is_some(),
            "updated source should resolve expression ids from fresh parse cache"
        );
    }

    #[test]
    fn salsa_event_counters_emit_query_categories() {
        let mut db = Database::new_with_salsa_observability();
        let file = FileId(39);
        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := value + 1;\nEND_PROGRAM\n"
                .to_string(),
        );

        db.reset_salsa_event_counters();
        let _ = db.file_symbols(file);
        let first = db.salsa_event_snapshot();
        assert!(
            first.total > 0 && first.recomputes > 0,
            "first query should emit observable execution events"
        );

        let _ = db.file_symbols(file);
        let second = db.salsa_event_snapshot();
        assert!(
            second.total > first.total,
            "second query should continue emitting events"
        );
        assert!(
            second.cache_hits >= first.cache_hits,
            "memoized query path should not decrease cache-hit counters"
        );
    }

    #[test]
    fn project_type_catalog_reuses_result_when_type_preludes_are_unchanged() {
        let mut db = Database::new();
        let file_types = FileId(41);
        let file_other = FileId(42);
        db.set_source_text(
            file_types,
            "TYPE CARRIER :\nSTRUCT\n    A : INT;\nEND_STRUCT\nEND_TYPE\n".to_string(),
        );
        db.set_source_text(
            file_other,
            "PROGRAM Main\nVAR\n    X : INT;\nEND_VAR\nX := 1;\nEND_PROGRAM\n".to_string(),
        );

        let first = db.with_synced_salsa_state(|state| {
            salsa_backend::project_type_catalog_query(&state.db, salsa_backend::project_inputs(state))
                .clone()
        });

        db.set_source_text(
            file_other,
            "PROGRAM Main\nVAR\n    X : INT;\nEND_VAR\nX := 2;\nEND_PROGRAM\n".to_string(),
        );

        let second = db.with_synced_salsa_state(|state| {
            salsa_backend::project_type_catalog_query(&state.db, salsa_backend::project_inputs(state))
                .clone()
        });

        assert!(
            Arc::ptr_eq(&first, &second),
            "non-type edits should not invalidate the project type catalog"
        );
    }

    #[test]
    fn dependent_type_edit_reinvalidates_cross_file_global_struct_analysis() {
        let mut db = Database::new();
        let file_types = FileId(43);
        let file_globals = FileId(44);
        let file_main = FileId(45);

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

        let first = db.analyze_salsa(file_main);
        assert!(
            first.diagnostics.iter().all(|diagnostic| !diagnostic.is_error()),
            "initial cross-file field access should analyze cleanly: {:?}",
            first.diagnostics
        );

        db.set_source_text(
            file_types,
            "TYPE CARRIER :\nSTRUCT\n    B : INT;\nEND_STRUCT\nEND_TYPE\n".to_string(),
        );

        let second = db.analyze_salsa(file_main);
        assert!(
            second
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.is_error()),
            "dependent type edit should invalidate downstream analysis: {:?}",
            second.diagnostics
        );
    }

    #[test]
    fn semantic_kernel_cross_file_resolution_tracks_lsp_style_dependency_edits() {
        let mut db = Database::new();
        let file_lib = FileId(46);
        let file_main = FileId(47);

        db.set_source_text(
            file_lib,
            "FUNCTION Pick : INT\nVAR_INPUT\n    x : INT;\nEND_VAR\nPick := x + 1;\nEND_FUNCTION\n"
                .to_string(),
        );
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := Pick(1);\nEND_PROGRAM\n"
                .to_string(),
        );

        let first = db.analyze_salsa(file_main);
        assert!(
            first.diagnostics.iter().all(|diagnostic| !diagnostic.is_error()),
            "initial dependency should resolve cleanly: {:?}",
            first.diagnostics
        );

        db.set_source_text(
            file_lib,
            "FUNCTION Pick : BOOL\nVAR_INPUT\n    x : INT;\nEND_VAR\nPick := x > 0;\nEND_FUNCTION\n"
                .to_string(),
        );

        let after_return_type_edit = db.analyze_salsa(file_main);
        assert!(
            after_return_type_edit.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == crate::diagnostics::DiagnosticCode::IncompatibleAssignment
            }),
            "edited dependency should invalidate semantic resolution and report an assignment mismatch: {:?}",
            after_return_type_edit.diagnostics
        );

        db.set_source_text(
            file_lib,
            "FUNCTION Pick : INT\nVAR_INPUT\n    x : INT;\nEND_VAR\nPick := x + 2;\nEND_FUNCTION\n"
                .to_string(),
        );

        let after_fix = db.analyze_salsa(file_main);
        assert!(
            after_fix
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.is_error()),
            "fixing the dependency should clear the stale semantic error: {:?}",
            after_fix.diagnostics
        );
    }

    #[test]
    fn cancellation_requests_keep_queries_stable() {
        let mut setup_db = Database::new_with_salsa_observability();
        let file = FileId(40);
        setup_db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := value + 1;\nEND_PROGRAM\n"
                .to_string(),
        );
        let db = Arc::new(setup_db);

        let worker_db = Arc::clone(&db);
        let worker = thread::spawn(move || {
            for _ in 0..80 {
                let _ = worker_db.analyze(file);
                let _ = worker_db.diagnostics(file);
            }
        });

        for _ in 0..80 {
            db.trigger_salsa_cancellation();
        }

        worker
            .join()
            .expect("query worker should finish without panic");
        let snapshot = db.salsa_event_snapshot();
        assert!(
            snapshot.cancellation_flags > 0,
            "cancellation requests should emit cancellation event counters"
        );
    }

    #[test]
    fn concurrent_edit_and_query_loops_do_not_panic() {
        let file = FileId(37);
        let db = Arc::new(RwLock::new(Database::new()));
        db.write().set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := 0;\nEND_PROGRAM\n".to_string(),
        );

        let writer_db = Arc::clone(&db);
        let writer = thread::spawn(move || {
            for value in 0..120 {
                writer_db.write().set_source_text(
                    file,
                    format!(
                        "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := {};\nEND_PROGRAM\n",
                        value % 10
                    ),
                );
            }
        });

        let mut readers = Vec::new();
        for _ in 0..2 {
            let reader_db = Arc::clone(&db);
            readers.push(thread::spawn(move || {
                for _ in 0..200 {
                    let guard = reader_db.read();
                    let analysis = guard.analyze(file);
                    assert!(
                        analysis
                            .diagnostics
                            .iter()
                            .all(|diagnostic| !diagnostic.is_error()),
                        "concurrent read path should remain stable while edits happen"
                    );
                    let source = guard.source_text(file);
                    let offset = source.find("value :=").unwrap_or(0) as u32;
                    let _ = guard.expr_id_at_offset(file, offset);
                    let _ = guard.file_symbols(file);
                    let _ = guard.diagnostics(file);
                }
            }));
        }

        writer.join().expect("writer thread should finish");
        for reader in readers {
            reader.join().expect("reader thread should finish");
        }
    }

    #[test]
    fn query_boundary_sequence_no_longer_panics() {
        let mut db = Database::new();
        let file = FileId(38);
        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := 1;\nEND_PROGRAM\n".to_string(),
        );

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for value in 0..100 {
                db.set_source_text(
                    file,
                    format!(
                        "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := {};\nEND_PROGRAM\n",
                        value
                    ),
                );
                let _ = db.file_symbols(file);
                let _ = db.analyze(file);
                let _ = db.diagnostics(file);
                let source = db.source_text(file);
                let offset = source.find("value :=").unwrap_or(0) as u32;
                let _ = db.expr_id_at_offset(file, offset);
            }
        }));

        assert!(
            result.is_ok(),
            "query boundary sequence should not panic after owned-state refactor"
        );
    }
