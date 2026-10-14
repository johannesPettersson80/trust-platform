//! Tests of the authoring command's argument (specification 43, §3 and §4): the project folder, its storage,
//! the open documents, the panel selections and the simulator sessions the command reads.
//!
//! Port of `editors/vscode/src/test/suite/hmi-agent-projects.test.ts` at the argument level: a project is
//! chosen by its address, an address that is not a local folder (a remote or virtual workspace) is an
//! unsupported context and never a path, and a malformed argument is refused. The journeys of that suite
//! run in the editor; the authoring steps they perform are ported in `service_tests.rs`.

use std::path::PathBuf;

use serde_json::{json, Value};

use super::super::documents::OpenDocument;
use super::super::error::{INVALID_ARGUMENT, UNSUPPORTED_CONTEXT};
use super::*;

/// The storage folder the tests name: an absolute path that reading the argument does not create.
fn temp_storage() -> String {
    std::env::temp_dir()
        .join("trust-agent-argument-storage")
        .display()
        .to_string()
}

/// One command argument with the given project address, storage and extra fields.
fn argument(root_uri: &str, storage: &str, extra: Value) -> Value {
    let mut argument = json!({
        "workspaceId": "workspace-a",
        "root_uri": root_uri,
        "storage": storage,
        "request": { "contractVersion": 1, "workspaceId": "workspace-a", "input": {} },
    });
    if let (Value::Object(fields), Value::Object(extra)) = (&mut argument, extra) {
        fields.extend(extra);
    }
    argument
}

/// A file address names the project folder; the storage is an absolute folder that is kept as given
/// (specification 43, §3 and §4).
#[test]
fn a_file_address_is_the_project_folder_and_the_storage_is_kept_as_given() {
    let read = Argument::read(vec![argument(
        "file:///srv/projects/tank",
        &temp_storage(),
        json!({}),
    )])
    .expect("a local project");
    assert_eq!(read.workspace_id, "workspace-a");
    assert_eq!(read.root_uri, "file:///srv/projects/tank");
    assert_eq!(read.root, PathBuf::from("/srv/projects/tank"));
    assert_eq!(read.storage, PathBuf::from(temp_storage()));
    assert_eq!(read.remote, json!({}));
    assert!(read.selections.is_empty());
    assert!(read.open.is_empty());
    assert!(read.simulator_sessions.is_empty());
    assert_eq!(read.request["input"], json!({}));
}

/// Port of `hmi-agent-projects.test.ts` (the project is not a local folder): a remote, virtual or untitled
/// address is an unsupported context. It belongs to its own extension host and is never resolved as a
/// local path (specification 43, §3).
#[test]
fn a_remote_or_virtual_project_is_an_unsupported_context() {
    for address in [
        "vscode-remote://ssh-remote%2Bbuilder/home/project",
        "vscode-vfs://github/owner/repository",
        "untitled:Untitled-1",
    ] {
        let error = Argument::read(vec![argument(address, &temp_storage(), json!({}))])
            .expect_err("only a local folder is a project");
        assert_eq!(error.code, UNSUPPORTED_CONTEXT, "{address}");
    }
}

/// An address that is not a URI names no project, so it is an invalid argument (specification 43, §4).
#[test]
fn an_address_that_is_not_a_uri_is_an_invalid_argument() {
    let error = Argument::read(vec![argument(
        "not a project address",
        &temp_storage(),
        json!({}),
    )])
    .expect_err("not a URI");
    assert_eq!(error.code, INVALID_ARGUMENT);
}

/// The storage folder must be absolute: a relative one would depend on the working directory (specification
/// 43, §4).
#[test]
fn the_storage_must_be_an_absolute_folder() {
    let error = Argument::read(vec![argument(
        "file:///srv/projects/tank",
        "private/drafts",
        json!({}),
    )])
    .expect_err("a relative storage folder");
    assert_eq!(error.code, INVALID_ARGUMENT);
    assert!(error.message.contains("absolute"), "{}", error.message);
}

/// A blank workspace identifier is refused: a request names the workspace it belongs to (specification 43, §4).
#[test]
fn a_blank_workspace_identifier_is_refused() {
    let error = Argument::read(vec![argument(
        "file:///srv/projects/tank",
        &temp_storage(),
        json!({ "workspaceId": "   " }),
    )])
    .expect_err("a blank workspace");
    assert_eq!(error.code, INVALID_ARGUMENT);
}

/// The command takes exactly one argument object (specification 43, §3).
#[test]
fn the_command_takes_exactly_one_argument_object() {
    let one = argument("file:///srv/projects/tank", &temp_storage(), json!({}));
    assert_eq!(
        Argument::read(Vec::new()).expect_err("no argument").code,
        INVALID_ARGUMENT
    );
    assert_eq!(
        Argument::read(vec![one.clone(), one])
            .expect_err("two arguments")
            .code,
        INVALID_ARGUMENT
    );
}

/// The remote runtime's variables are an object; an absent or null value is an empty object, and any other
/// kind is refused (specification 43, §3).
#[test]
fn the_remote_variables_are_an_object_and_default_to_empty() {
    let base = "file:///srv/projects/tank";
    let absent =
        Argument::read(vec![argument(base, &temp_storage(), json!({}))]).expect("a project");
    assert_eq!(absent.remote, json!({}));
    let null = Argument::read(vec![argument(
        base,
        &temp_storage(),
        json!({ "remote": null }),
    )])
    .expect("a project");
    assert_eq!(null.remote, json!({}));
    let number = Argument::read(vec![argument(
        base,
        &temp_storage(),
        json!({ "remote": 3 }),
    )])
    .expect_err("a number is not an object of variables");
    assert_eq!(number.code, INVALID_ARGUMENT);
}

/// A panel selection is decoded with its own fields and no others (specification 43, AT-01).
#[test]
fn panel_selections_decode_their_fields_and_refuse_others() {
    let base = "file:///srv/projects/tank";
    let read = Argument::read(vec![argument(
        base,
        &temp_storage(),
        json!({
            "selections": [{
                "file": "hmi/pages/process.toml",
                "ids": ["tank"],
                "documentVersion": 3,
                "active": true,
            }],
        }),
    )])
    .expect("a selection");
    let selection = &read.selections[0];
    assert_eq!(selection.file, "hmi/pages/process.toml");
    assert_eq!(selection.ids, vec!["tank".to_owned()]);
    assert_eq!(selection.document_version, 3);
    assert!(selection.active);
    let zoomed = Argument::read(vec![argument(
        base,
        &temp_storage(),
        json!({
            "selections": [{
                "file": "hmi/pages/process.toml",
                "ids": [],
                "documentVersion": 3,
                "active": true,
                "zoom": 2,
            }],
        }),
    )])
    .expect_err("a field the selection does not own");
    assert_eq!(zoomed.code, INVALID_ARGUMENT);
}

/// An open document carries its text, its version and whether it is dirty. A document without its dirty
/// state is refused (specification 43, §4: a dirty buffer is authoritative until it is saved or discarded).
#[test]
fn open_documents_carry_their_text_version_and_dirty_state() {
    let base = "file:///srv/projects/tank";
    let read = Argument::read(vec![argument(
        base,
        &temp_storage(),
        json!({
            "open": {
                "hmi/pages/process.toml": { "text": "title = \"Edited\"", "version": 4, "dirty": true },
            },
        }),
    )])
    .expect("an open document");
    assert_eq!(
        read.open["hmi/pages/process.toml"],
        OpenDocument {
            text: "title = \"Edited\"".to_owned(),
            version: 4,
            dirty: true,
        }
    );
    let partial = Argument::read(vec![argument(
        base,
        &temp_storage(),
        json!({
            "open": { "hmi/pages/process.toml": { "text": "title = \"Edited\"", "version": 4 } },
        }),
    )])
    .expect_err("a document without its dirty state");
    assert_eq!(partial.code, INVALID_ARGUMENT);
}

/// The simulator sessions the extension lists are kept as given, for the context answer (specification 43,
/// §2.4).
#[test]
fn simulator_sessions_are_kept_as_given() {
    let session = json!({ "sessionId": "session-1", "expiresAt": "2026-10-10T12:00:00Z" });
    let read = Argument::read(vec![argument(
        "file:///srv/projects/tank",
        &temp_storage(),
        json!({ "simulatorSessions": [session.clone()] }),
    )])
    .expect("sessions");
    assert_eq!(read.simulator_sessions, vec![session]);
}
