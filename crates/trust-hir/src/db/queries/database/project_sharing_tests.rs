// Checklist G276c: a library file's symbol table is collected once, in its own project, and shared
// by the projects that use the library (`project_sharing.rs`, specification 14 §10.2). Included in
// `database.rs`'s test module.
    /// The symbol table of `library` in the project of `user`, as that project's analysis holds it.
    fn library_table_in_project_of(db: &Database, user: FileId, library: FileId) -> Arc<SymbolTable> {
        db.with_synced_salsa_state(|state| {
            let project = salsa_backend::project_of(state, user).expect("the project");
            salsa_backend::project_symbol_tables_query(&state.db, project)
                .get(&library)
                .cloned()
                .expect("the library file's table")
        })
    }

    /// Checklist G276c: two projects that use one library take the library file's symbol table from
    /// the library's own project, so the three hold one table. Catches a library file collected
    /// again in each project that names the library.
    #[test]
    fn projects_that_use_a_library_share_its_files_symbol_tables() {
        let mut db = Database::new();
        let (user_a, user_b, library) = (FileId(61), FileId(62), FileId(63));
        db.set_source_text(
            user_a,
            "PROGRAM A\nVAR\n    limit : FB_Limit;\nEND_VAR\nEND_PROGRAM\n".to_string(),
        );
        db.set_source_text(
            user_b,
            "PROGRAM B\nVAR\n    limit : FB_Limit;\nEND_VAR\nEND_PROGRAM\n".to_string(),
        );
        db.set_source_text(
            library,
            "FUNCTION_BLOCK FB_Limit\nVAR_INPUT\n    value : INT;\nEND_VAR\nEND_FUNCTION_BLOCK\n"
                .to_string(),
        );
        let mut layout = ProjectLayout::default();
        for (file, project) in [(user_a, 1), (user_b, 2), (library, 3)] {
            layout.files.insert(file, ProjectId(project));
        }
        layout.library_files.insert(ProjectId(1), vec![library]);
        layout.library_files.insert(ProjectId(2), vec![library]);
        db.set_project_layout(layout);

        let own = library_table_in_project_of(&db, library, library);
        assert!(
            Arc::ptr_eq(&library_table_in_project_of(&db, user_a, library), &own),
            "project A takes the library's own table"
        );
        assert!(
            Arc::ptr_eq(&library_table_in_project_of(&db, user_b, library), &own),
            "project B takes the library's own table"
        );
        assert!(
            db.diagnostics(user_a).iter().all(|diagnostic| diagnostic.severity != crate::diagnostics::DiagnosticSeverity::Error),
            "project A still resolves FB_Limit: {:?}",
            db.diagnostics(user_a)
        );
    }

    /// Checklist G276c: a project that the library's project can reach, as mutual libraries and a
    /// ring of libraries are, collects the file itself. Catches a project whose query asks, through
    /// its libraries, for its own, which stops the analysis with a query cycle.
    #[test]
    fn mutual_libraries_collect_each_others_files_without_a_query_cycle() {
        let mut db = Database::new();
        let files = [FileId(71), FileId(72), FileId(73)];
        for (index, file) in files.iter().enumerate() {
            db.set_source_text(
                *file,
                format!("FUNCTION_BLOCK FB_{index}\nVAR_INPUT\n    value : INT;\nEND_VAR\nEND_FUNCTION_BLOCK\n"),
            );
        }
        // A ring: project 1 sees project 2's file, 2 sees 3's, and 3 sees 1's, none of them
        // transitively; then 1 and 2 see each other.
        for (rings, layout_sees) in [
            (true, vec![(1, files[1]), (2, files[2]), (3, files[0])]),
            (false, vec![(1, files[1]), (2, files[0])]),
        ] {
            let mut layout = ProjectLayout::default();
            for (index, file) in files.iter().enumerate() {
                layout.files.insert(*file, ProjectId(index as u32 + 1));
            }
            for (project, file) in layout_sees {
                layout.library_files.insert(ProjectId(project), vec![file]);
            }
            db.set_project_layout(layout);
            let seen_by_1 = library_table_in_project_of(&db, files[0], files[1]);
            let own = library_table_in_project_of(&db, files[1], files[1]);
            assert!(
                !Arc::ptr_eq(&seen_by_1, &own),
                "project 1 collects project 2's file itself (ring: {rings})"
            );
        }
    }
