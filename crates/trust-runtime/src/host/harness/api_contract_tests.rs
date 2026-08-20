use super::*;

use crate::bytecode::{BytecodeModule, SectionId};

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

fn assert_same_executable_sections(left: &BytecodeModule, right: &BytecodeModule) {
    assert_eq!(left.version, right.version);
    assert_eq!(left.flags, right.flags);
    for section in [
        SectionId::StringTable,
        SectionId::TypeTable,
        SectionId::ConstPool,
        SectionId::RefTable,
        SectionId::PouIndex,
        SectionId::PouBodies,
        SectionId::ResourceMeta,
        SectionId::IoMap,
        SectionId::VarMeta,
        SectionId::RetainInit,
    ] {
        assert_eq!(left.section(section), right.section(section), "{section:?}");
    }
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
fn harness_api_contract_runtime_build_uses_complete_source_set() {
    let runtime = CompileSession::from_sources(vec![
        SourceFile::new(helper_source()),
        SourceFile::new(counter_source()),
    ])
    .build_runtime()
    .unwrap();
    assert!(runtime
        .programs()
        .values()
        .any(|program| program.name.eq_ignore_ascii_case("Main")));
    assert!(runtime
        .functions()
        .values()
        .any(|function| function.name.eq_ignore_ascii_case("AddOne")));
}

#[test]
fn harness_api_contract_module_and_bytes_builds_are_equivalent() {
    let session = CompileSession::from_source(counter_source());
    let module = session.build_bytecode_module().unwrap();
    let bytes = session.build_bytecode_bytes().unwrap();
    assert!(!bytes.is_empty());
    assert_eq!(BytecodeModule::decode(&bytes).unwrap(), module);
}

#[test]
fn harness_api_contract_single_source_module_helpers_are_equivalent() {
    let direct = bytecode_module_from_source(counter_source()).unwrap();
    let with_path = bytecode_module_from_source_with_path(counter_source(), "src/main.st").unwrap();
    assert_same_executable_sections(&direct, &with_path);
    assert_ne!(
        direct.section(SectionId::DebugStringTable),
        with_path.section(SectionId::DebugStringTable)
    );
}

#[test]
fn harness_api_contract_multi_source_module_helpers_are_equivalent() {
    let sources = [helper_source(), counter_source()];
    let paths = ["src/helper.st", "src/main.st"];
    let direct = bytecode_module_from_sources(&sources).unwrap();
    let with_paths = bytecode_module_from_sources_with_paths(&sources, &paths).unwrap();
    assert_same_executable_sections(&direct, &with_paths);
    assert_ne!(
        direct.section(SectionId::DebugStringTable),
        with_paths.section(SectionId::DebugStringTable)
    );
}

#[test]
fn harness_api_contract_single_source_byte_helpers_decode() {
    for bytes in [
        bytecode_bytes_from_source(counter_source()).unwrap(),
        bytecode_bytes_from_source_with_path(counter_source(), "src/main.st").unwrap(),
    ] {
        assert!(BytecodeModule::decode(&bytes).is_ok());
    }
}

#[test]
fn harness_api_contract_multi_source_byte_helpers_decode() {
    let sources = [helper_source(), counter_source()];
    let paths = ["src/helper.st", "src/main.st"];
    for bytes in [
        bytecode_bytes_from_sources(&sources).unwrap(),
        bytecode_bytes_from_sources_with_paths(&sources, &paths).unwrap(),
    ] {
        assert!(BytecodeModule::decode(&bytes).is_ok());
    }
}

#[test]
fn harness_api_contract_source_path_length_mismatch_is_rejected_before_build() {
    let sources = [counter_source()];
    let empty_paths: [&str; 0] = [];
    let extra_paths = ["one.st", "two.st"];
    for error in [
        bytecode_module_from_sources_with_paths(&sources, &empty_paths)
            .unwrap_err()
            .to_string(),
        bytecode_module_from_sources_with_paths(&sources, &extra_paths)
            .unwrap_err()
            .to_string(),
        bytecode_bytes_from_sources_with_paths(&sources, &empty_paths)
            .unwrap_err()
            .to_string(),
        bytecode_bytes_from_sources_with_paths(&sources, &extra_paths)
            .unwrap_err()
            .to_string(),
    ] {
        assert_eq!(error, "sources/paths length mismatch");
    }
}

#[test]
fn harness_api_contract_labeled_parse_error_uses_explicit_path() {
    let error = CompileSession::from_sources(vec![SourceFile::with_path(
        "broken/main.st",
        "PROGRAM Broken",
    )])
    .label_errors(true)
    .build_runtime()
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
    .build_runtime()
    .unwrap_err()
    .to_string();
    assert!(error.contains("file 1:"), "{error}");
}

#[test]
fn harness_api_contract_unlabeled_error_omits_source_prefix() {
    let error = CompileSession::from_source("PROGRAM Broken")
        .build_runtime()
        .unwrap_err()
        .to_string();
    assert!(!error.contains("file 0:"), "{error}");
}

#[test]
fn harness_api_contract_extra_program_instance_is_explicit_and_case_insensitive() {
    let default_error = CompileSession::from_source(unbound_program_source())
        .build_runtime()
        .unwrap_err()
        .to_string();
    assert!(default_error.contains("unbound PROGRAM declaration"));

    let runtime = CompileSession::from_source(unbound_program_source())
        .with_extra_program_instances(["main", "MAIN"])
        .build_runtime()
        .unwrap();
    assert!(runtime
        .storage()
        .globals()
        .keys()
        .any(|name| name.eq_ignore_ascii_case("Main")));
}

#[test]
fn harness_api_contract_unknown_extra_program_is_rejected_by_name() {
    let error = CompileSession::from_source(unbound_program_source())
        .with_extra_program_instances(["Missing"])
        .build_runtime()
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("extra PROGRAM instance 'Missing' has no matching declaration"),
        "{error}"
    );
}
