//! Unit tests of the endpoint file, the per-user token file, and withdrawing a token (spec 11,
//! section 6.8.0a).

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::file::{
    endpoint_file_path, read_endpoint_file, remove_endpoint_file_of, withdraw_auth_token,
    withdraw_auth_token_with, write_endpoint_file, EndpointFile, EndpointFileError,
    PublishedEndpoint, ENDPOINT_FILE_SCHEMA_VERSION,
};
use super::test_support::{read_file, tcp, Scratch};
use super::token_file::{read_token_file, token_file_path, TokenFile};
use super::ENDPOINT_DIR;

/// The token file that a publication with the token home `token_home` wrote for `project`.
fn token_file_of(project: &Scratch, token_home: &Scratch) -> Option<TokenFile> {
    let path = token_file_path(token_home.path(), project.path()).expect("token file path");
    read_token_file(&path)
}

#[test]
fn an_endpoint_file_round_trips_and_omits_a_token_it_did_not_generate() {
    let project = Scratch::new("file-round-trip");
    let file = EndpointFile {
        schema_version: ENDPOINT_FILE_SCHEMA_VERSION,
        endpoint: "tcp://127.0.0.1:5555".to_string(),
        pid: 42,
        auth_token: None,
    };
    write_endpoint_file(project.path(), &file).expect("write endpoint file");

    assert_eq!(read_file(project.path()), file);
    let text = std::fs::read_to_string(endpoint_file_path(project.path())).expect("raw file");
    assert_eq!(
        text,
        r#"{"schema_version":1,"endpoint":"tcp://127.0.0.1:5555","pid":42}"#
    );
}

#[test]
fn an_endpoint_file_carries_a_token_the_runtime_generated() {
    let project = Scratch::new("file-token");
    project.publish_file("tcp://127.0.0.1:5555", 42, Some("generated"));

    let text = std::fs::read_to_string(endpoint_file_path(project.path())).expect("raw file");
    assert!(text.contains(r#""auth_token":"generated""#), "{text}");
    assert_eq!(
        read_file(project.path()).auth_token.as_deref(),
        Some("generated")
    );
}

#[test]
fn a_write_leaves_only_the_endpoint_file_in_its_directory() {
    let project = Scratch::new("file-atomic");
    project.publish_file("tcp://127.0.0.1:5555", 42, None);

    let mut names: Vec<String> = std::fs::read_dir(project.path().join(ENDPOINT_DIR))
        .expect("endpoint directory")
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(names, vec!["control.json".to_string()]);
}

#[test]
fn a_missing_endpoint_file_reads_as_none() {
    let project = Scratch::new("file-missing");

    assert_eq!(read_endpoint_file(project.path()), Ok(None));
}

#[test]
fn an_endpoint_file_of_another_schema_or_a_malformed_one_is_invalid() {
    let project = Scratch::new("file-invalid");
    std::fs::create_dir_all(project.path().join(ENDPOINT_DIR)).expect("endpoint directory");
    let path = endpoint_file_path(project.path());

    std::fs::write(
        &path,
        r#"{"schema_version":2,"endpoint":"tcp://127.0.0.1:1","pid":1}"#,
    )
    .expect("write future schema");
    assert!(matches!(
        read_endpoint_file(project.path()),
        Err(EndpointFileError::Invalid(_))
    ));

    std::fs::write(&path, "not json").expect("write malformed file");
    assert!(matches!(
        read_endpoint_file(project.path()),
        Err(EndpointFileError::Invalid(_))
    ));
}

#[cfg(unix)]
#[test]
fn an_endpoint_file_that_cannot_be_read_is_unreadable_not_invalid() {
    let project = Scratch::new("file-unreadable");
    project.publish_file("tcp://127.0.0.1:5555", 1, None);
    let path = endpoint_file_path(project.path());
    super::test_support::set_mode(&path, 0o000);
    if super::test_support::readable_despite_mode(&path) {
        eprintln!("skipped: this process reads files whatever their mode");
        return;
    }

    assert!(matches!(
        read_endpoint_file(project.path()),
        Err(EndpointFileError::Unreadable(reason)) if reason.contains("control.json")
    ));
}

#[cfg(unix)]
#[test]
fn the_endpoint_file_and_its_directory_are_owner_only() {
    let project = Scratch::new("file-modes");
    project.publish_file("tcp://127.0.0.1:5555", 1, None);

    let directory_mode = std::fs::metadata(project.path().join(ENDPOINT_DIR))
        .expect("directory metadata")
        .permissions()
        .mode();
    let file_mode = std::fs::metadata(endpoint_file_path(project.path()))
        .expect("file metadata")
        .permissions()
        .mode();
    assert_eq!(directory_mode & 0o777, 0o700);
    assert_eq!(file_mode & 0o777, 0o600);
}

#[cfg(unix)]
#[test]
fn publishing_into_a_read_only_project_fails_and_leaves_nothing_behind() {
    let project = Scratch::new("publish-read-only");
    super::test_support::set_mode(project.path(), 0o555);
    if std::fs::write(project.path().join("probe"), "x").is_ok() {
        eprintln!("skipped: this process writes into read-only folders");
        return;
    }

    let error = PublishedEndpoint::publish(project.path(), &tcp("127.0.0.1:5555"), None)
        .expect_err("a read-only project folder cannot take the endpoint file");

    assert!(error.to_string().contains(".trust-runtime"), "{error}");
    assert!(!project.path().join(ENDPOINT_DIR).exists());
}

#[test]
fn dropping_a_published_endpoint_removes_its_file() {
    let project = Scratch::new("publish-drop");
    let published =
        PublishedEndpoint::publish(project.path(), &tcp("127.0.0.1:5555"), None).expect("publish");
    assert!(endpoint_file_path(project.path()).is_file());

    drop(published);

    assert!(!endpoint_file_path(project.path()).exists());
}

#[test]
fn dropping_a_published_endpoint_keeps_a_file_that_names_another_process() {
    let project = Scratch::new("publish-keep");
    let published =
        PublishedEndpoint::publish(project.path(), &tcp("127.0.0.1:5555"), None).expect("publish");
    project.publish_file(
        "tcp://127.0.0.1:6666",
        std::process::id().wrapping_add(1),
        None,
    );

    drop(published);

    assert!(endpoint_file_path(project.path()).is_file());
    assert_eq!(read_file(project.path()).endpoint, "tcp://127.0.0.1:6666");
}

// The per-user token file of a Windows runtime (spec 11, section 6.8.0a).

#[test]
fn with_a_per_user_token_folder_the_generated_token_stays_out_of_the_project() {
    let project = Scratch::new("token-home-publish");
    let token_home = Scratch::new("token-home-publish-dir");

    let published = PublishedEndpoint::publish_with(
        project.path(),
        &tcp("127.0.0.1:5555"),
        Some("generated-token"),
        Some(token_home.path()),
    )
    .expect("publish");

    let file = read_file(project.path());
    assert_eq!(file.endpoint, "tcp://127.0.0.1:5555");
    assert_eq!(file.auth_token, None);
    let raw = std::fs::read_to_string(endpoint_file_path(project.path())).expect("raw file");
    assert!(!raw.contains("generated-token"), "{raw}");
    let token = token_file_of(&project, &token_home).expect("token file");
    assert_eq!(token.pid, std::process::id());
    assert_eq!(token.endpoint, "tcp://127.0.0.1:5555");
    assert_eq!(token.into_token(), "generated-token");

    drop(published);

    assert!(!endpoint_file_path(project.path()).exists());
    assert!(token_file_of(&project, &token_home).is_none());
}

#[test]
fn the_token_file_is_named_after_the_hash_of_the_canonical_project_path() {
    let project = Scratch::new("token-home-name");
    let token_home = Scratch::new("token-home-name-dir");

    let path = token_file_path(token_home.path(), project.path()).expect("token file path");

    let canonical = std::fs::canonicalize(project.path()).expect("canonical project");
    let expected = format!("{}.json", super::defaults::project_path_hash(&canonical));
    assert_eq!(path, token_home.path().join(expected));
}

#[test]
fn a_token_file_redacts_its_token_in_debug_output() {
    let file = TokenFile::new(
        "tcp://127.0.0.1:5555".to_string(),
        7,
        "secret-token-value".to_string(),
    );
    let endpoint_file = EndpointFile {
        schema_version: ENDPOINT_FILE_SCHEMA_VERSION,
        endpoint: "tcp://127.0.0.1:5555".to_string(),
        pid: 7,
        auth_token: Some("secret-token-value".to_string()),
    };

    for text in [format!("{file:?}"), format!("{endpoint_file:?}")] {
        assert!(!text.contains("secret-token-value"), "{text}");
        assert!(text.contains("<redacted>"), "{text}");
    }
}

// Withdrawing the token after `config.set` (spec 11, section 6.8.0a).

#[test]
fn withdrawing_the_token_keeps_the_endpoint_and_the_pid_and_restoring_puts_it_back() {
    let project = Scratch::new("withdraw-token");
    // The token home is given, so the token is in the endpoint file on every platform.
    let published = PublishedEndpoint::publish_with(
        project.path(),
        &tcp("127.0.0.1:5555"),
        Some("generated-token"),
        None,
    )
    .expect("publish");
    let before = read_file(project.path());

    let withdrawn = withdraw_auth_token_with(project.path(), None)
        .expect("withdraw")
        .expect("the token was withdrawn");

    let after = read_file(project.path());
    assert_eq!(after.endpoint, before.endpoint);
    assert_eq!(after.pid, before.pid);
    assert_eq!(after.auth_token, None);
    let text = std::fs::read_to_string(endpoint_file_path(project.path())).expect("read file");
    assert!(!text.contains("auth_token"));
    assert!(!format!("{withdrawn:?}").contains("generated-token"));

    withdrawn.restore().expect("restore");

    assert_eq!(read_file(project.path()), before);
    drop(published);
}

#[test]
fn withdrawing_a_token_from_the_token_file_removes_it_and_restoring_writes_it_again() {
    let project = Scratch::new("withdraw-token-home");
    let token_home = Scratch::new("withdraw-token-home-dir");
    let published = PublishedEndpoint::publish_with(
        project.path(),
        &tcp("127.0.0.1:5555"),
        Some("generated-token"),
        Some(token_home.path()),
    )
    .expect("publish");

    let withdrawn = withdraw_auth_token_with(project.path(), Some(token_home.path()))
        .expect("withdraw")
        .expect("the token was withdrawn");

    assert!(token_file_of(&project, &token_home).is_none());
    assert_eq!(read_file(project.path()).endpoint, "tcp://127.0.0.1:5555");

    withdrawn.restore().expect("restore");

    let token = token_file_of(&project, &token_home).expect("token file restored");
    assert_eq!(token.into_token(), "generated-token");
    drop(published);
}

#[test]
fn withdrawing_the_token_leaves_a_file_that_names_another_process_alone() {
    let project = Scratch::new("withdraw-other");
    project.publish_file(
        "tcp://127.0.0.1:6666",
        std::process::id().wrapping_add(1),
        Some("other-token"),
    );

    let withdrawn = withdraw_auth_token(project.path()).expect("withdraw");

    assert!(withdrawn.is_none());
    assert_eq!(
        read_file(project.path()).auth_token.as_deref(),
        Some("other-token")
    );
}

#[test]
fn withdrawing_the_token_without_an_endpoint_file_creates_none() {
    let project = Scratch::new("withdraw-missing");

    let withdrawn = withdraw_auth_token(project.path()).expect("withdraw");

    assert!(withdrawn.is_none());
    assert!(!endpoint_file_path(project.path()).exists());
}

#[test]
fn withdrawing_ignores_an_endpoint_file_that_does_not_parse() {
    let project = Scratch::new("withdraw-malformed");
    std::fs::create_dir_all(project.path().join(ENDPOINT_DIR)).expect("endpoint directory");
    let path = endpoint_file_path(project.path());
    std::fs::write(&path, "not json").expect("write malformed file");

    let withdrawn = withdraw_auth_token(project.path())
        .expect("a file this process cannot own is no error for config.set");

    assert!(withdrawn.is_none());
    assert_eq!(
        std::fs::read_to_string(&path).expect("read file"),
        "not json"
    );
}

#[cfg(unix)]
#[test]
fn withdrawing_ignores_an_endpoint_file_that_cannot_be_read() {
    let project = Scratch::new("withdraw-unreadable");
    project.publish_file("tcp://127.0.0.1:6666", std::process::id(), Some("token"));
    let path = endpoint_file_path(project.path());
    super::test_support::set_mode(&path, 0o000);
    if super::test_support::readable_despite_mode(&path) {
        eprintln!("skipped: this process reads files whatever their mode");
        return;
    }

    let withdrawn = withdraw_auth_token(project.path())
        .expect("a file this process cannot read is no error for config.set");

    assert!(withdrawn.is_none());
}

// Removing the endpoint file of a stopped process (spec 11, section 6.8.0a).

#[test]
fn removing_the_endpoint_file_of_a_process_removes_only_that_process_file() {
    let project = Scratch::new("remove-of");
    let other = std::process::id().wrapping_add(1);
    project.publish_file("tcp://127.0.0.1:6666", other, None);

    assert!(!remove_endpoint_file_of(
        project.path(),
        other.wrapping_add(1)
    ));
    assert!(endpoint_file_path(project.path()).is_file());

    assert!(remove_endpoint_file_of(project.path(), other));
    assert!(!endpoint_file_path(project.path()).exists());
}
