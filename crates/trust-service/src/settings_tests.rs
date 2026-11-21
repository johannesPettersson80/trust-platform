//! A service's settings from `trust.toml` (specification 48 RU-62, Appendix B), each refusal naming
//! what is missing. Every expected value is worked out from the specification.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{duration, ServiceSettings};
use crate::ServiceError;

/// A project folder holding `trust.toml` with `text` and, unless `None`, the service token
/// `secrets/recipe.token` with `token`.
fn project(text: &str, token: Option<&str>) -> PathBuf {
    let root = trust_test_support::unique_temp_path("service-settings");
    std::fs::create_dir_all(root.join("secrets")).expect("the project folder");
    std::fs::write(root.join("trust.toml"), text).expect("trust.toml");
    if let Some(token) = token {
        std::fs::write(root.join("secrets/recipe.token"), token).expect("the token");
    }
    root
}

/// Appendix B's service, with an endpoint, a credentials file and a heartbeat.
const RECIPE: &str = "[services.recipe]\npackage = \"recipe-service\"\nheartbeat = \"500ms\"\n\
endpoint = \"tcp://127.0.0.1:47800\"\ncredentials = \"secrets/recipe.token\"\n";

/// Publishes `text` as the endpoint file of the project's runtime (specification 11 §6.8.0a).
fn publish(root: &Path, text: &str) {
    let folder = root.join(".trust-runtime");
    std::fs::create_dir_all(&folder).expect("the endpoint folder");
    std::fs::write(folder.join("control.json"), text).expect("the endpoint file");
}

/// The endpoint file of a runtime that bound the free port 50123 and generated a token of its own,
/// as a runtime on its default endpoint publishes it.
const PUBLISHED: &str =
    r#"{"schema_version":1,"endpoint":"tcp://127.0.0.1:50123","pid":4242,"auth_token":"RUNTIME"}"#;

fn refusal(root: &Path, name: &str) -> String {
    match ServiceSettings::from_project(root, name) {
        Err(ServiceError::Settings(message)) => message,
        other => panic!("a settings refusal: {other:?}"),
    }
}

#[test]
fn a_services_settings_come_from_its_trust_toml_section() {
    // The endpoint as written, the token without the file's line end, the heartbeat of 500 ms.
    // Catches a token sent with its newline, which no runtime takes, and a period in other units.
    let root = project(RECIPE, Some("ABCDEFGH\n"));
    assert_eq!(
        ServiceSettings::from_project(&root, "recipe"),
        Ok(ServiceSettings {
            name: "recipe".into(),
            endpoint: "tcp://127.0.0.1:47800".into(),
            token: "ABCDEFGH".into(),
            heartbeat: Duration::from_millis(500),
        })
    );
}

#[test]
fn a_missing_setting_is_named_with_what_to_do() {
    // A service trust.toml does not declare, a section without an endpoint while the project's
    // runtime is not running, and a token file that does not exist, whose refusal says how to
    // create it. Catches a service that starts without its credentials and fails later with a
    // refusal it cannot explain.
    let root = project(RECIPE, Some("ABCDEFGH"));
    assert_eq!(
        refusal(&root, "camera"),
        "trust.toml declares no [services.camera]"
    );
    let no_endpoint = project(
        &RECIPE.replace("endpoint = \"tcp://127.0.0.1:47800\"\n", ""),
        Some("A"),
    );
    assert_eq!(
        refusal(&no_endpoint, "recipe"),
        format!(
            "[services.recipe] names no endpoint, and no runtime of the project has published one: \
             no endpoint file at {}",
            no_endpoint
                .join(".trust-runtime")
                .join("control.json")
                .display()
        )
    );
    let no_token = project(RECIPE, None);
    let message = refusal(&no_token, "recipe");
    assert!(
        message.contains("secrets/recipe.token")
            && message.contains("trust-runtime ctl service-token recipe --out <file>"),
        "{message}"
    );
}

#[test]
fn without_an_endpoint_a_service_takes_the_one_its_running_runtime_published() {
    // Specification 48 RU-62 with specification 11 §6.8.0a: a section without `endpoint` takes the
    // endpoint in the runtime's endpoint file, with the free port the runtime bound, and the
    // service still presents its own service token, never the token the runtime generated, which
    // would make it an Admin whose writes no `writes` limits. An endpoint the section names wins
    // over the file. Catches a service that cannot reach a runtime on its default endpoint, one
    // that takes the runtime's token, and a file that overrides what the user named.
    let no_endpoint = RECIPE.replace("endpoint = \"tcp://127.0.0.1:47800\"\n", "");
    let root = project(&no_endpoint, Some("ABCDEFGH\n"));
    publish(&root, PUBLISHED);
    assert_eq!(
        ServiceSettings::from_project(&root, "recipe"),
        Ok(ServiceSettings {
            name: "recipe".into(),
            endpoint: "tcp://127.0.0.1:50123".into(),
            token: "ABCDEFGH".into(),
            heartbeat: Duration::from_millis(500),
        })
    );
    let named = project(RECIPE, Some("ABCDEFGH"));
    publish(&named, PUBLISHED);
    assert_eq!(
        ServiceSettings::from_project(&named, "recipe").map(|settings| settings.endpoint),
        Ok("tcp://127.0.0.1:47800".into())
    );
}

#[test]
fn an_endpoint_that_is_no_text_or_a_published_file_that_does_not_read_is_named() {
    // An `endpoint` that is no text, and, without one, an endpoint file of a schema this build
    // does not read. Catches a service that connects to a guessed endpoint instead of saying why
    // it cannot connect.
    let number = project(&RECIPE.replace("\"tcp://127.0.0.1:47800\"", "5"), Some("A"));
    assert_eq!(
        refusal(&number, "recipe"),
        "[services.recipe] endpoint 5 is no text such as \"tcp://127.0.0.1:47800\""
    );
    let root = project(
        &RECIPE.replace("endpoint = \"tcp://127.0.0.1:47800\"\n", ""),
        Some("A"),
    );
    publish(
        &root,
        r#"{"schema_version":2,"endpoint":"tcp://127.0.0.1:50123","pid":4242}"#,
    );
    assert_eq!(
        refusal(&root, "recipe"),
        format!(
            "[services.recipe] names no endpoint, and the endpoint file of the project's runtime \
             does not read: {} has unsupported schema_version 2",
            root.join(".trust-runtime").join("control.json").display()
        )
    );
}

#[test]
fn a_heartbeat_is_a_whole_number_of_a_unit() {
    // ns, us, ms, s and min; zero, a fraction, a missing unit and an unknown one are no period.
    // Catches 500ms read as 500 s and a period of zero, which would send heartbeats without pause.
    assert_eq!(duration("500ms"), Some(Duration::from_millis(500)));
    assert_eq!(duration("2s"), Some(Duration::from_secs(2)));
    assert_eq!(duration("750us"), Some(Duration::from_micros(750)));
    assert_eq!(duration("100ns"), Some(Duration::from_nanos(100)));
    assert_eq!(duration("1min"), Some(Duration::from_secs(60)));
    for text in ["0ms", "1.5s", "500", "5h", "ms", ""] {
        assert_eq!(duration(text), None, "{text:?}");
    }
    let root = project(&RECIPE.replace("\"500ms\"", "\"fast\""), Some("A"));
    assert_eq!(
        refusal(&root, "recipe"),
        "[services.recipe] heartbeat \"fast\" is no duration such as \"500ms\" or \"2s\""
    );
}

#[test]
fn the_settings_debug_form_hides_the_token() {
    // Catches a token written to a log.
    let settings = ServiceSettings {
        name: "recipe".into(),
        endpoint: "tcp://127.0.0.1:47800".into(),
        token: "secret-token-ABCD".into(),
        heartbeat: Duration::from_secs(1),
    };
    let shown = format!("{settings:?}");
    assert!(
        shown.contains("…ABCD") && !shown.contains("secret-token"),
        "{shown}"
    );
}
