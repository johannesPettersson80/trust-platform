//! The test build's file labels (specification 22; specification 44 §5.3).
use super::*;

/// Specification 22 and specification 44 §5.3: the test image labels each file by its path relative to the project
/// folder, as `trust-runtime build` does, so a compile error of the tests names `src/tests.st` and no path of this
/// computer. Catches the whole path that the test build gave every file.
#[test]
fn a_compile_error_of_the_tests_names_its_file_relative_to_the_project() {
    let project = trust_test_support::unique_temp_path("trust-dev-g101");
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(
        project.join("src/tests.st"),
        "TEST_PROGRAM Broken\nVAR\n    x : INT;\nEND_VAR\nx := 'text';\nEND_TEST_PROGRAM\n",
    )
    .expect("write the test");
    let sources =
        load_sources(&resolve_sources_root(&project, None).expect("sources")).expect("load");
    let discovered = discover_tests(&sources);
    let error = execute_native_tests(&project, &discovered, &discovered, 5)
        .expect_err("the build refuses the tests")
        .to_string();
    let _ = std::fs::remove_dir_all(&project);

    assert!(error.contains("src/tests.st"), "{error}");
    let whole = project.display().to_string();
    assert!(!error.contains(&whole), "{error}");
}
