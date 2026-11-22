//! `[runtime.cloud] site` (specification 11, "Runtime Cloud sites"): the site the configuration
//! gives the Runtime Cloud and the discovery announcement, and the values it refuses.

use super::parser::parse_runtime_toml_from_text;
use super::RuntimeConfig;

/// The smallest `runtime.toml` the parser accepts.
const BASE: &str = r#"
[bundle]
version = 1

[resource]
name = "runtime_b"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#;

fn parse(extra: &str) -> Result<RuntimeConfig, crate::error::RuntimeError> {
    parse_runtime_toml_from_text(&format!("{BASE}\n{extra}\n"), "runtime.toml")
}

/// Catches a site that is read but not announced, or announced but not given to the cloud: both
/// take the configured site.
#[test]
fn the_configured_site_goes_to_the_cloud_and_to_the_announcement() {
    let config = parse("[runtime.cloud]\nsite = \"site-b\"").expect("a site is accepted");
    assert_eq!(config.runtime_cloud_site.as_deref(), Some("site-b"));
    assert_eq!(config.discovery.site.as_deref(), Some("site-b"));
}

/// Catches `default-site` treated as a site of its own, which would give the runtime the ID
/// `default-site/runtime_b` and change every default-site runtime's ID.
#[test]
fn no_site_and_default_site_are_the_default_site() {
    for extra in [
        "",
        "[runtime.cloud]\nprofile = \"dev\"",
        "[runtime.cloud]\nsite = \"default-site\"",
    ] {
        let config = parse(extra).expect("the default site is accepted");
        assert_eq!(config.runtime_cloud_site, None, "{extra:?}");
        assert_eq!(config.discovery.site, None, "{extra:?}");
    }
}

/// Catches a site the cloud ID and the keyspace cannot carry: a `/` would make the cloud ID name
/// another site, and an empty or spaced value no site at all.
#[test]
fn a_site_that_is_no_identifier_is_refused_naming_the_setting() {
    for site in ["", "site/b", "site b", "-site", "sité"] {
        let error =
            parse(&format!("[runtime.cloud]\nsite = \"{site}\"")).expect_err("the site is refused");
        let text = error.to_string();
        assert!(
            text.contains("runtime.cloud.site must be 1 to 63 ASCII letters"),
            "{site:?}: {text}"
        );
    }
}
