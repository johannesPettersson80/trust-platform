use super::*;

fn counter_source() -> &'static str {
    r#"
PROGRAM Main
VAR
    count : DINT;
END_VAR
count := count + 1;
END_PROGRAM
"#
}

fn helper_source() -> &'static str {
    r#"
FUNCTION AddOne : INT
VAR_INPUT
    value : INT;
END_VAR
AddOne := value + 1;
END_FUNCTION
"#
}

fn unbound_program_source() -> &'static str {
    r#"
CONFIGURATION Conf
END_CONFIGURATION

PROGRAM Main
VAR
    value : INT;
END_VAR
value := INT#7;
END_PROGRAM
"#
}

#[test]
fn harness_api_contract_source_file_constructors_preserve_text_and_path() {
    let virtual_source = SourceFile::new("PROGRAM Main\nEND_PROGRAM");
    assert_eq!(virtual_source.path, None);
    assert_eq!(virtual_source.text, "PROGRAM Main\nEND_PROGRAM");

    let path_source = SourceFile::with_path("src/main.st", "PROGRAM Main\nEND_PROGRAM");
    assert_eq!(path_source.path.as_deref(), Some("src/main.st"));
    assert_eq!(path_source.text, "PROGRAM Main\nEND_PROGRAM");
}

#[test]
fn harness_api_contract_compile_error_preserves_exact_message() {
    let error = CompileError::new("specific compile failure");
    assert_eq!(error.to_string(), "specific compile failure");
    let as_error: &dyn std::error::Error = &error;
    assert_eq!(as_error.to_string(), "specific compile failure");
}

#[test]
fn harness_api_contract_single_source_session_defaults_to_unlabeled() {
    let session = CompileSession::from_source(counter_source());
    assert_eq!(session.sources.len(), 1);
    assert_eq!(session.sources[0].path, None);
    assert_eq!(session.sources[0].text, counter_source());
    assert!(!session.label_errors);
    assert!(session.extra_program_instances.is_empty());
    assert!(session.instrumentation_errors.is_empty());
}

#[test]
fn harness_api_contract_multi_source_session_preserves_order_paths_and_labels() {
    let session = CompileSession::from_sources(vec![
        SourceFile::with_path("first.st", helper_source()),
        SourceFile::with_path("second.st", counter_source()),
    ]);
    assert!(session.label_errors);
    assert_eq!(
        session
            .sources()
            .iter()
            .map(|source| source.path.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("first.st"), Some("second.st")]
    );
    assert_eq!(session.sources()[0].text, helper_source());
    assert_eq!(session.sources()[1].text, counter_source());
}

#[test]
fn harness_api_contract_explicit_label_selection_overrides_default() {
    let multi = CompileSession::from_sources(vec![
        SourceFile::new(helper_source()),
        SourceFile::new(counter_source()),
    ])
    .label_errors(false);
    assert!(!multi.label_errors);

    let single = CompileSession::from_source(counter_source()).label_errors(true);
    assert!(single.label_errors);
}

#[test]
fn harness_api_contract_extra_program_names_preserve_caller_order() {
    let session = CompileSession::from_source(unbound_program_source())
        .with_extra_program_instances(["Main", "Other", "Main"]);
    assert_eq!(
        session
            .extra_program_instances
            .iter()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>(),
        vec!["Main", "Other", "Main"]
    );
}

#[test]
fn harness_api_contract_declarations_use_complete_source_set() {
    let declarations = CompileSession::from_sources(vec![
        SourceFile::new(helper_source()),
        SourceFile::new(counter_source()),
    ])
    .declarations()
    .unwrap();
    let metadata = declarations.metadata();
    assert!(metadata
        .programs()
        .values()
        .any(|program| program.name.eq_ignore_ascii_case("Main")));
    assert!(metadata
        .functions()
        .values()
        .any(|function| function.name.eq_ignore_ascii_case("AddOne")));
}

#[test]
fn harness_api_contract_labeled_parse_error_uses_explicit_path() {
    let error = CompileSession::from_sources(vec![SourceFile::with_path(
        "broken/main.st",
        "PROGRAM Broken",
    )])
    .label_errors(true)
    .declarations()
    .unwrap_err()
    .to_string();
    assert!(error.contains("broken/main.st:"), "{error}");
}

#[test]
fn harness_api_contract_labeled_parse_error_uses_stable_virtual_index() {
    let error = CompileSession::from_sources(vec![
        SourceFile::new(helper_source()),
        SourceFile::new("PROGRAM Broken"),
    ])
    .declarations()
    .unwrap_err()
    .to_string();
    assert!(error.contains("file 1:"), "{error}");
}

#[test]
fn harness_api_contract_unlabeled_error_omits_source_prefix() {
    let error = CompileSession::from_source("PROGRAM Broken")
        .declarations()
        .unwrap_err()
        .to_string();
    assert!(!error.contains("file 0:"), "{error}");
}

#[test]
fn harness_api_contract_extra_program_instance_is_explicit_and_case_insensitive() {
    let default_error = CompileSession::from_source(unbound_program_source())
        .declarations()
        .unwrap_err()
        .to_string();
    assert!(default_error.contains("unbound PROGRAM declaration"));

    let declarations = CompileSession::from_source(unbound_program_source())
        .with_extra_program_instances(["main", "MAIN"])
        .declarations()
        .unwrap();
    assert!(declarations
        .storage()
        .globals()
        .keys()
        .any(|name| name.eq_ignore_ascii_case("Main")));
}

#[test]
fn harness_api_contract_unknown_extra_program_is_rejected_by_name() {
    let error = CompileSession::from_source(unbound_program_source())
        .with_extra_program_instances(["Missing"])
        .declarations()
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("extra PROGRAM instance 'Missing' has no matching declaration"),
        "{error}"
    );
}
