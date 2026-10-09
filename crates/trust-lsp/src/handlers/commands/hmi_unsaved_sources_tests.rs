// Specification 43 sections 4 and AT-01: binding metadata follows open editor source,
// including errors and runtime-ignore headers, without writing that source to disk.
#[test]
fn hmi_agent_bindings_compile_unsaved_source_and_surface_its_errors() {
    let root = temp_dir("trust-hmi-agent-unsaved");
    std::fs::create_dir_all(root.join("src")).unwrap();
    let file = root.join("src/main.st");
    let saved = "PROGRAM Main\nVAR Saved : REAL; END_VAR\nEND_PROGRAM\n";
    std::fs::write(&file, saved).unwrap();
    let root_uri = Url::from_directory_path(&root).unwrap();
    let file_uri = Url::from_file_path(&file).unwrap();
    let mut context = MockContext { workspace_folders: vec![root_uri], ..MockContext::default() };
    context.insert_document(Document::new(file_uri.clone(), 2,
        "PROGRAM Main\nVAR Unsaved : BOOL; END_VAR\nEND_PROGRAM\n".into(), FileId(17), true, 0));
    let result = hmi_bindings_value_with_context(&context, vec![]).unwrap();
    assert_eq!(result["ok"], true, "{result}");
    let variables = result["variables"].to_string();
    assert!(variables.contains("Main.Unsaved"), "unsaved binding missing: {result}");
    assert!(!variables.contains("Main.Saved"), "stale disk binding advertised: {result}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), saved);
    context.insert_document(Document::new(file_uri, 3,
        "PROGRAM Main\nVAR Broken : NoSuchType; END_VAR\nEND_PROGRAM\n".into(), FileId(17), true, 0));
    let invalid = hmi_bindings_value_with_context(&context, vec![]).unwrap();
    assert_eq!(invalid["ok"], false, "unsaved compile error hidden: {invalid}");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn hmi_agent_closed_document_cache_does_not_override_saved_source() {
    let root = temp_dir("trust-hmi-agent-closed");
    std::fs::create_dir_all(root.join("src")).unwrap();
    let file = root.join("src/main.st");
    std::fs::write(&file, "PROGRAM Main\nVAR Current : REAL; END_VAR\nEND_PROGRAM\n").unwrap();
    let mut context = MockContext { workspace_folders: vec![Url::from_directory_path(&root).unwrap()], ..MockContext::default() };
    context.insert_document(Document::new(Url::from_file_path(&file).unwrap(), 2,
        "PROGRAM Main\nVAR Old : BOOL; END_VAR\nEND_PROGRAM\n".into(), FileId(18), false, 0));
    let result = hmi_bindings_value_with_context(&context, vec![]).unwrap();
    assert!(result["variables"].to_string().contains("Main.Current"), "{result}");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn hmi_agent_source_revision_binds_metadata_to_exact_program_text() {
    let root = temp_dir("trust-hmi-agent-source-revision");
    std::fs::create_dir_all(root.join("src")).unwrap();
    let file = root.join("src/main.st");
    std::fs::write(&file, "PROGRAM Main\nVAR Level : REAL := 1.0; END_VAR\nEND_PROGRAM\n").unwrap();
    let mut context = MockContext { workspace_folders: vec![Url::from_directory_path(&root).unwrap()], ..MockContext::default() };
    let before = hmi_bindings_value_with_context(&context, vec![]).unwrap();
    assert_eq!(before["sourceRevision"].as_str().unwrap_or("").len(), 64, "source fingerprint missing: {before}");
    let repeat = hmi_bindings_value_with_context(&context, vec![]).unwrap();
    assert_eq!(before, repeat, "a cached response preserves the complete source-bound metadata");
    let mut caller_copy = repeat;
    caller_copy["variables"] = json!([]);
    assert_eq!(hmi_bindings_value_with_context(&context, vec![]).unwrap(), before, "callers cannot mutate the retained response");
    context.insert_document(Document::new(Url::from_file_path(&file).unwrap(), 2,
        "PROGRAM Main\nVAR Level : REAL := 2.0; END_VAR\nEND_PROGRAM\n".into(), FileId(19), true, 0));
    let edited = hmi_bindings_value_with_context(&context, vec![]).unwrap();
    assert_ne!(before["sourceRevision"], edited["sourceRevision"], "even a changed initial value changes the program revision");
    std::fs::remove_dir_all(root).unwrap();
}
