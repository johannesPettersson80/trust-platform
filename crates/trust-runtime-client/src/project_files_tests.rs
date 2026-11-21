//! The files a project's runtime publishes for its clients (specification 11, section 6.8.0a): the
//! endpoint file's place and its reading. Every expected value is worked out from the
//! specification's file format, `{"schema_version":1,"endpoint":"<bound endpoint>","pid":<process
//! id>}` with a generated token added as `"auth_token"` on Unix-like systems.

use std::path::{Path, PathBuf};

use crate::project_files::{endpoint_file_path, read_endpoint_file, EndpointFile, EndpointFileError};

/// A new, empty project folder.
fn project() -> PathBuf {
    let root = trust_test_support::unique_temp_path("project-files");
    std::fs::create_dir_all(&root).expect("the project folder");
    root
}

/// Writes `text` as the project's endpoint file and gives its path.
fn publish(root: &Path, text: &str) -> PathBuf {
    let path = root.join(".trust-runtime").join("control.json");
    std::fs::create_dir_all(path.parent().expect("its folder")).expect("the endpoint folder");
    std::fs::write(&path, text).expect("the endpoint file");
    path
}

#[test]
fn a_running_runtimes_endpoint_file_gives_its_endpoint_pid_and_token() {
    // The file in `<project>/.trust-runtime/control.json`, with a generated token as on Unix-like
    // systems. Catches a reader that looks elsewhere, drops the token, or prints it in a log.
    let root = project();
    publish(
        &root,
        r#"{"schema_version":1,"endpoint":"tcp://127.0.0.1:50123","pid":4242,"auth_token":"T0K3N"}"#,
    );
    let file = read_endpoint_file(&root).expect("it reads");
    assert_eq!(
        file,
        Some(EndpointFile {
            schema_version: 1,
            endpoint: "tcp://127.0.0.1:50123".into(),
            pid: 4242,
            auth_token: Some("T0K3N".into()),
        })
    );
    let shown = format!("{file:?}");
    assert!(
        !shown.contains("T0K3N") && shown.contains("<redacted>"),
        "{shown}"
    );
}

#[test]
fn a_project_without_an_endpoint_file_has_no_runtime_that_published_one() {
    // Catches a missing file read as an error, which would make every stopped runtime look broken.
    let root = project();
    assert_eq!(read_endpoint_file(&root), Ok(None));
    assert_eq!(
        endpoint_file_path(&root),
        root.join(".trust-runtime").join("control.json")
    );
}

#[test]
fn an_endpoint_file_of_another_schema_or_not_json_is_invalid() {
    // Only schema version 1 is read. Catches a file of a later format read as if it were the
    // first, and a half-written or foreign file taken as an endpoint.
    let root = project();
    let path = publish(
        &root,
        r#"{"schema_version":2,"endpoint":"tcp://127.0.0.1:50123","pid":4242}"#,
    );
    assert_eq!(
        read_endpoint_file(&root),
        Err(EndpointFileError::Invalid(format!(
            "{} has unsupported schema_version 2",
            path.display()
        )))
    );
    publish(&root, "not json");
    match read_endpoint_file(&root) {
        Err(EndpointFileError::Invalid(message)) => assert!(
            message.starts_with(&format!("parse {}: ", path.display())),
            "{message}"
        ),
        other => panic!("an invalid file: {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn a_program_stores_current_link_keeps_its_files_in_the_store_folder() {
    // Specification 11 §6.8.0a: a runtime started on a store's `current` link keeps its files in
    // the store folder, and a client given the link takes the same files. A folder that is only
    // named `current`, and a link with another name, keep their own. Catches a client that looks
    // in the bundle, where no file is, and a rule that goes by the name alone.
    use crate::project_files::runtime_files_folder;

    let store = project();
    let bundle = store.join("bundles").join("b1");
    std::fs::create_dir_all(&bundle).expect("the bundle");
    let current = store.join("current");
    std::os::unix::fs::symlink(&bundle, &current).expect("the current link");
    assert_eq!(runtime_files_folder(&current), store);
    assert_eq!(
        endpoint_file_path(&current),
        store.join(".trust-runtime").join("control.json")
    );
    publish(
        &store,
        r#"{"schema_version":1,"endpoint":"unix:///s.sock","pid":7}"#,
    );
    assert_eq!(
        read_endpoint_file(&current).map(|file| file.map(|file| file.endpoint)),
        Ok(Some("unix:///s.sock".into()))
    );

    let plain = project().join("current");
    std::fs::create_dir_all(&plain).expect("a folder named current");
    assert_eq!(runtime_files_folder(&plain), plain);
    let previous = store.join("previous");
    std::os::unix::fs::symlink(&bundle, &previous).expect("the previous link");
    assert_eq!(runtime_files_folder(&previous), previous);
}
