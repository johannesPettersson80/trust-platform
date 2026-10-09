//! Specification 39, §12.2–12.4 and §12.8: users, session times and the write policy.

use std::time::Duration;

use super::config::{HmiAccessConfig, HmiSection};
use super::password::{hash_password, pbkdf2_sha256, PasswordHash};
use crate::hmi::member_paths::MemberPath;
use crate::security::AccessRole;

const SALT: &[u8] = b"0123456789abcdef";

fn section(text: &str) -> HmiSection {
    toml::from_str(text).expect("the section parses")
}

fn path(text: &str) -> MemberPath {
    MemberPath::parse(text).expect("a path")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn without_a_section_nobody_may_write_and_sessions_have_their_defaults() {
    let config = HmiAccessConfig::from_section(HmiSection::default()).expect("valid");
    assert_eq!(config.idle, Duration::from_secs(15 * 60));
    assert_eq!(config.session, Duration::from_secs(12 * 60 * 60));
    assert!(config.users.is_empty());
    assert!(config.write.is_empty());
}

#[test]
fn users_and_write_rules_are_read() {
    let hash = hash_password("secret", 10_000, SALT);
    let text = format!(
        r#"
idle_minutes = 5
session_hours = 8

[[user]]
name = "anna"
role = "operator"
password = "{hash}"

[[write]]
path = "Loop.Setpoint"
role = "operator"
min = 0.0
max = 120.0

[[write]]
path = "Loop.Tuning.*"
role = "engineer"
"#
    );
    let config = HmiAccessConfig::from_section(section(&text)).expect("valid");
    assert_eq!(config.idle, Duration::from_secs(5 * 60));
    assert_eq!(config.session, Duration::from_secs(8 * 60 * 60));
    assert_eq!(config.users.len(), 1);
    assert_eq!(config.users[0].name, "anna");
    assert_eq!(config.users[0].role, AccessRole::Operator);
    assert!(config.users[0].password.verify("secret"));
    let setpoint = &config.write[0];
    assert_eq!(setpoint.role, AccessRole::Operator);
    assert_eq!((setpoint.min, setpoint.max), (Some(0.0), Some(120.0)));
    assert!(setpoint.covers(&path("Loop.Setpoint")));
    assert!(
        setpoint.covers(&path("loop.SETPOINT")),
        "identifiers ignore case"
    );
    assert!(!setpoint.covers(&path("Loop.Setpoint.Bias")));
    let tuning = &config.write[1];
    assert_eq!(tuning.role, AccessRole::Engineer);
    assert!(tuning.covers(&path("Loop.Tuning.Gain")));
    assert!(tuning.covers(&path("Loop.Tuning.Limits[2].High")));
    assert!(
        !tuning.covers(&path("Loop.Tuning")),
        "the members below, not the path itself"
    );
    assert!(!tuning.covers(&path("Loop.TuningX.Gain")));
}

#[test]
fn a_bad_section_is_refused_naming_the_key() {
    let hash = hash_password("secret", 10_000, SALT);
    let refused = |text: &str, words: &str| {
        let error = HmiAccessConfig::from_section(section(text)).expect_err(text);
        assert!(error.contains(words), "{text}: {error}");
    };
    refused("idle_minutes = 0", "idle_minutes");
    refused("session_hours = 0", "session_hours");
    refused(
        &format!("[[user]]\nname = \" \"\nrole = \"operator\"\npassword = \"{hash}\""),
        "name",
    );
    refused(
        "[[user]]\nname = \"anna\"\nrole = \"operator\"\npassword = \"secret\"",
        "password",
    );
    refused(
        &format!("[[user]]\nname = \"anna\"\nrole = \"chief\"\npassword = \"{hash}\""),
        "role",
    );
    refused(
        &format!("[[user]]\nname = \"anna\"\nrole = \"operator\"\npassword = \"{hash}\"\n[[user]]\nname = \"ANNA\"\nrole = \"viewer\"\npassword = \"{hash}\""),
        "twice",
    );
    refused(
        "[[write]]\npath = \"Loop..Setpoint\"\nrole = \"operator\"",
        "path",
    );
    refused(
        "[[write]]\npath = \"Loop.Setpoint\"\nrole = \"viewer\"",
        "role",
    );
    refused(
        "[[write]]\npath = \"Loop.Setpoint\"\nrole = \"operator\"\nmin = 5.0\nmax = 1.0",
        "min",
    );
}

#[test]
fn a_password_is_kept_only_as_a_salted_hash_and_verifies() {
    let hash = hash_password("secret", 10_000, SALT);
    assert!(hash.starts_with("pbkdf2-sha256:10000:"), "{hash}");
    assert!(!hash.contains("secret"));
    let stored = PasswordHash::parse(&hash).expect("valid");
    assert!(stored.verify("secret"));
    assert!(!stored.verify("Secret"));
    assert!(!stored.verify(""));
    assert_ne!(
        hash,
        hash_password("secret", 10_000, b"another salt...."),
        "the salt matters"
    );
    assert!(
        PasswordHash::parse("pbkdf2-sha256:1000:c2FsdA:aGFzaA").is_err(),
        "too few iterations"
    );
}

#[test]
fn pbkdf2_matches_the_published_test_vector() {
    // RFC 7914, section 11: PBKDF2-HMAC-SHA256, P = "passwd", S = "salt", c = 1, dkLen = 64.
    let derived = pbkdf2_sha256(b"passwd", b"salt", 1, 64);
    assert_eq!(
        hex(&derived),
        "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc49ca9cccf179b645991664b39d77ef317c71b845b1e30bd509112041d3a19783"
    );
}
