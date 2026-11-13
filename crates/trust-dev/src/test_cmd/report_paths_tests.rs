//! The paths the test reports name (specification 22, "Test reporting and CI output").
use super::*;

/// Specification 22: list mode reports a test's project-relative path, here the list of the agent API's runner, which
/// writes its own JSON. Worked by hand: the test lies in `src/tests.st`, so the list names `src/tests.st` (`src\tests.st`
/// on Windows), and the project is the folder's whole path without `\\?\`. Catches the whole path the list printed.
#[test]
fn the_agent_list_names_files_relative_to_the_project_folder() {
    let project = trust_test_support::unique_temp_path("trust-dev-t3-list");
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(
        project.join("src/tests.st"),
        "TEST_PROGRAM Listed\nEND_TEST_PROGRAM\n",
    )
    .expect("write the test");
    let listed = run_test_json(Some(project.clone()), None, true, 1).expect("the list");
    let _ = std::fs::remove_dir_all(&project);

    let relative = std::path::Path::new("src")
        .join("tests.st")
        .display()
        .to_string();
    assert_eq!(listed["tests"][0]["file"], relative.as_str(), "{listed}");
    let project_text = listed["project"].as_str().expect("the project");
    assert!(!project_text.starts_with(r"\\?\"), "{project_text}");
}
