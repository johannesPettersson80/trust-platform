//! The runtime display's users (specification 39, §12.8 item 1).

use serde_json::json;

use super::answer;

const RUNTIME: &str =
    "[bundle]\nversion = 1\n\n[resource]\nname = \"Res\"\n\n[runtime.web]\nenabled = true\n";
/// A hash `trust-runtime hmi-password` makes (`pbkdf2-sha256:<iterations>:<salt>:<hash>`).
const HASH: &str =
    "pbkdf2-sha256:600000:c2FsdHNhbHRzYWx0c2FsdA:aGFzaGhhc2hoYXNoaGFzaGhhc2hoYXNoaGFzaGhhc2g";

fn ask(argument: serde_json::Value) -> serde_json::Value {
    answer(argument).unwrap_or_else(|error| json!({ "ok": false, "error": error }))
}

#[test]
fn a_user_is_appended_with_the_hash_and_removed_again_to_the_file_it_was() {
    // §12.8: the rest of runtime.toml stays as it is; the password itself is never written.
    let added = ask(json!({
        "action": "add", "runtimeToml": RUNTIME, "name": " anna ", "role": "operator", "hash": HASH
    }));
    let text = added["text"].as_str().expect("the new text").to_owned();
    assert_eq!(
        text,
        format!(
            "{RUNTIME}\n[[runtime.web.hmi.user]]\nname = \"anna\"\nrole = \"operator\"\npassword = \"{HASH}\"\n"
        ),
        "appended, the name trimmed"
    );
    assert_eq!(
        ask(json!({ "action": "list", "runtimeToml": text })),
        json!({ "ok": true, "users": [{ "name": "anna", "role": "operator" }] })
    );
    assert_eq!(
        ask(json!({ "action": "remove", "runtimeToml": text, "name": "ANNA" })),
        json!({ "ok": true, "text": RUNTIME }),
        "removed whatever the name's case, back to the file it was"
    );
}

#[test]
fn a_name_is_needed_and_taken_whatever_its_case() {
    let text = format!(
        "{RUNTIME}\n[[runtime.web.hmi.user]]\nname = \"anna\"\nrole = \"operator\"\npassword = \"{HASH}\"\n"
    );
    let problem = |name: &str| {
        ask(json!({ "action": "checkName", "runtimeToml": text, "name": name }))["problem"].clone()
    };
    assert_eq!(problem("   "), json!("Give the user a name"));
    assert_eq!(problem("Anna"), json!("A user of this name exists already"));
    assert_eq!(problem("ben"), json!(null));
    assert_eq!(
        ask(
            json!({ "action": "add", "runtimeToml": text, "name": "ANNA", "role": "engineer", "hash": HASH })
        ),
        json!({ "ok": false, "error": "A user of this name exists already" }),
        "add checks the name too, so a second Anna never reaches the file"
    );
}

#[test]
fn a_password_needs_eight_characters() {
    // §12.8 item 1: "a password of at least 8 characters".
    let problem = |characters: usize| {
        ask(json!({ "action": "checkPassword", "characters": characters }))["problem"].clone()
    };
    assert_eq!(problem(7), json!("Use at least 8 characters"));
    assert_eq!(problem(8), json!(null));
}

#[test]
fn only_a_stored_hash_and_a_known_role_are_written() {
    // A password typed into the hash field would be written in clear text.
    assert_eq!(
        ask(
            json!({ "action": "add", "runtimeToml": RUNTIME, "name": "ben", "role": "operator", "hash": "correct horse" })
        ),
        json!({ "ok": false, "error": "The password's hash is not one trust-runtime hmi-password makes." })
    );
    let refused = ask(
        json!({ "action": "add", "runtimeToml": RUNTIME, "name": "ben", "role": "admin", "hash": HASH }),
    );
    assert_eq!(
        refused["ok"],
        json!(false),
        "admin is no role a user is given in the editor"
    );
}

#[test]
fn users_listed_inline_are_left_to_the_user() {
    let inline =
        "[runtime.web.hmi]\nuser = [{ name = \"anna\", role = \"operator\", password = \"x\" }]\n";
    assert_eq!(
        ask(
            json!({ "action": "add", "runtimeToml": inline, "name": "ben", "role": "operator", "hash": HASH })
        ),
        json!({ "ok": false, "error": "runtime.toml lists its users inline under [runtime.web.hmi]; add the user there by hand." })
    );
}
