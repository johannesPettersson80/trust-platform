//! The connection file (specification 43, §3.1): the server opens only a private file that names
//! the window's loopback URL, a 64-digit capability and a workspace.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use serde_json::json;

use super::super::connection::{Connection, Refusal};
use super::{CAPABILITY, WORKSPACE};

/// The loopback URL of the examples.
const URL: &str = "http://127.0.0.1:41234/mcp";

/// A connection file's text.
fn file(url: &str, capability: &str, workspace: &str) -> String {
    json!({"url": url, "capability": capability, "workspaceId": workspace}).to_string()
}

/// Catches: the window's own loopback URL being refused, or the port, capability or workspace
/// being lost on the way.
#[test]
fn the_loopback_connection_names_its_address_capability_and_workspace() {
    let connection = Connection::parse(&file(URL, CAPABILITY, WORKSPACE)).expect("a connection");
    assert_eq!(
        connection.address,
        SocketAddr::from((Ipv4Addr::LOCALHOST, 41234))
    );
    assert_eq!(connection.capability, CAPABILITY);
    assert_eq!(connection.workspace_id, WORKSPACE);
    let default_port =
        Connection::parse(&file("http://127.0.0.1/mcp", CAPABILITY, WORKSPACE)).expect("port 80");
    assert_eq!(
        default_port.address,
        SocketAddr::from((Ipv4Addr::LOCALHOST, 80))
    );
}

/// Catches: the capability being sent to another host, scheme or path, or a URL with credentials,
/// a query or a fragment being accepted.
#[test]
fn only_a_plain_loopback_mcp_url_is_accepted() {
    for url in [
        "https://127.0.0.1:41234/mcp",
        "http://localhost:41234/mcp",
        "http://127.0.0.2:41234/mcp",
        "http://[::1]:41234/mcp",
        "http://192.168.1.2:41234/mcp",
        "http://127.0.0.1:41234/",
        "http://127.0.0.1:41234/mcp/call",
        "http://user@127.0.0.1:41234/mcp",
        "http://user:secret@127.0.0.1:41234/mcp",
        "http://127.0.0.1:41234/mcp?next=1",
        "http://127.0.0.1:41234/mcp?",
        "http://127.0.0.1:41234/mcp#top",
        "http://127.0.0.1:41234/mcp#",
        "127.0.0.1:41234/mcp",
        "",
    ] {
        assert_eq!(
            Connection::parse(&file(url, CAPABILITY, WORKSPACE)),
            Err(Refusal::NotLoopback),
            "{url}"
        );
    }
}

/// Catches: a short, long, uppercase or non-hexadecimal capability being accepted.
#[test]
fn the_capability_is_64_lowercase_hexadecimal_digits() {
    let long = format!("{CAPABILITY}0");
    let upper = CAPABILITY.to_uppercase();
    let not_hex = CAPABILITY.replace('a', "g");
    for capability in [&CAPABILITY[1..], &long, &upper, &not_hex, ""] {
        assert_eq!(
            Connection::parse(&file(URL, capability, WORKSPACE)),
            Err(Refusal::Capability),
            "{capability}"
        );
    }
}

/// Catches: a connection without a workspace, or a file that is not a connection, being used.
#[test]
fn a_connection_names_its_workspace() {
    assert_eq!(
        Connection::parse(&file(URL, CAPABILITY, "")),
        Err(Refusal::Workspace)
    );
    let without = json!({"url": URL, "capability": CAPABILITY}).to_string();
    assert_eq!(Connection::parse(&without), Err(Refusal::Malformed));
    assert_eq!(Connection::parse("not json"), Err(Refusal::Malformed));
}

/// A file path of this test that no other test uses.
fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "trust-hmi-agent-{}-{name}.json",
        std::process::id()
    ))
}

/// Catches: a capability file that group or other users can read being used, or the owner's
/// execute bit being taken for access by others (Unix).
#[cfg(unix)]
#[test]
fn the_connection_file_is_private_to_its_owner() {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    let path = temporary("private");
    std::fs::write(&path, file(URL, CAPABILITY, WORKSPACE)).expect("write the file");
    for (mode, private) in [
        (0o600, true),
        (0o700, true),
        (0o640, false),
        (0o604, false),
        (0o660, false),
    ] {
        std::fs::set_permissions(&path, Permissions::from_mode(mode)).expect("set the mode");
        let read = Connection::read(&path).map(|connection| connection.workspace_id);
        let expected = if private {
            Ok(WORKSPACE.to_owned())
        } else {
            Err(Refusal::NotPrivate)
        };
        assert_eq!(read, expected, "mode {mode:o}");
    }
    let _ = std::fs::remove_file(&path);
}

/// Catches: a missing file being read as an empty connection.
#[test]
fn a_missing_file_is_unreadable() {
    assert_eq!(
        Connection::read(&temporary("missing")),
        Err(Refusal::Unreadable)
    );
}
