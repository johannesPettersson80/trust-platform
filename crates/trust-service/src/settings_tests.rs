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
timeout = \"2s\"\nendpoint = \"tcp://127.0.0.1:47800\"\ncredentials = \"secrets/recipe.token\"\n";

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
    // A service trust.toml does not declare, a section without an endpoint, and a token file that
    // does not exist, whose refusal says how to create it. Catches a service that starts without
    // its credentials and fails later with a refusal it cannot explain.
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
        "[services.recipe] has no endpoint"
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
