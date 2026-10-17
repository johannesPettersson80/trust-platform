//! Unit tests of endpoint resolution, binding and connection failures (spec 11, sections 6.8.0a
//! and 6.8.1).

use std::path::Path;

use super::file::PublishedEndpoint;
use super::test_support::{tcp, Scratch};
use super::{
    binding_for, control_binding, project_control_endpoint_text, resolve_control_target,
    resolve_with, ControlBinding, ControlTarget, ControlTargetError, EndpointSource, ENDPOINT_DIR,
};
use crate::control::ControlEndpoint;
use crate::error::RuntimeError;

/// Resolves with an injected platform default and the token kept in the endpoint file, as on a
/// Unix-like system.
fn resolve_with_default(
    project: Option<&Path>,
    explicit: Option<ControlEndpoint>,
    token: Option<&str>,
    default: impl FnOnce(&Path) -> Result<ControlEndpoint, RuntimeError>,
) -> Result<ControlTarget, ControlTargetError> {
    resolve_with(project, explicit, token, default, None)
}

// Resolution order (spec 11, section 6.8.0a).

#[test]
fn explicit_endpoint_wins_over_the_endpoint_file_the_configured_endpoint_and_the_default() {
    let project = Scratch::new("explicit");
    project
        .write_runtime_toml("endpoint = \"tcp://127.0.0.1:9100\"\nauth_token = \"config-token\"");
    project.publish_file("tcp://127.0.0.1:9200", 1, None);

    let target = resolve_with_default(
        Some(project.path()),
        Some(tcp("127.0.0.1:9300")),
        Some("explicit-token"),
        |_| Ok(tcp("127.0.0.1:9400")),
    )
    .expect("explicit endpoint resolves");

    assert_eq!(target.source, EndpointSource::Explicit);
    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9300");
    assert_eq!(target.auth_token.as_deref(), Some("explicit-token"));
}

#[test]
fn endpoint_file_wins_over_the_configured_endpoint_and_the_default() {
    let project = Scratch::new("file-first");
    project
        .write_runtime_toml("endpoint = \"tcp://127.0.0.1:9100\"\nauth_token = \"config-token\"");
    project.publish_file("tcp://127.0.0.1:9200", 1, None);

    let target = resolve_with_default(Some(project.path()), None, None, |_| {
        Ok(tcp("127.0.0.1:9400"))
    })
    .expect("endpoint file resolves");

    assert_eq!(target.source, EndpointSource::EndpointFile);
    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9200");
    // The file names no token, so the configured token is the credential.
    assert_eq!(target.auth_token.as_deref(), Some("config-token"));
    assert_eq!(target.endpoint_file_skipped, None);
}

#[test]
fn configured_endpoint_wins_over_the_platform_default() {
    let project = Scratch::new("configured");
    project
        .write_runtime_toml("endpoint = \"tcp://127.0.0.1:9100\"\nauth_token = \"config-token\"");

    let target = resolve_with_default(Some(project.path()), None, None, |_| {
        Ok(tcp("127.0.0.1:9400"))
    })
    .expect("configured endpoint resolves");

    assert_eq!(target.source, EndpointSource::Configured);
    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9100");
    assert_eq!(target.auth_token.as_deref(), Some("config-token"));
}

#[test]
fn the_platform_default_is_the_last_resort() {
    let project = Scratch::new("default");
    project.write_runtime_toml("");

    let target = resolve_with_default(Some(project.path()), None, None, |_| {
        Ok(tcp("127.0.0.1:9400"))
    })
    .expect("default resolves");

    assert_eq!(target.source, EndpointSource::Default);
    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9400");
    assert_eq!(target.auth_token, None);
}

#[test]
fn a_windows_default_without_an_endpoint_file_reports_that_the_runtime_is_not_running() {
    let project = Scratch::new("unbound");
    project.write_runtime_toml("");

    let error = resolve_with_default(Some(project.path()), None, None, |_| Ok(tcp("127.0.0.1:0")))
        .expect_err("a default without a port names no runtime");

    assert_eq!(error.code(), "not_running");
    match error {
        ControlTargetError::NotRunning(message) => {
            assert!(message.starts_with("runtime is not running"), "{message}");
            assert!(message.contains("control.json"), "{message}");
        }
        other => panic!("expected not_running, got {other:?}"),
    }
}

#[test]
fn a_windows_default_with_an_endpoint_file_resolves_to_the_published_port() {
    let project = Scratch::new("bound");
    project.write_runtime_toml("");
    project.publish_file("tcp://127.0.0.1:5555", 1, Some("generated"));

    let target = resolve_with_default(Some(project.path()), None, None, |_| Ok(tcp("127.0.0.1:0")))
        .expect("the published port resolves");

    assert_eq!(target.source, EndpointSource::EndpointFile);
    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:5555");
    assert_eq!(target.auth_token.as_deref(), Some("generated"));
}

#[test]
fn no_project_and_no_endpoint_is_no_target() {
    let error = resolve_control_target(None, None, None).expect_err("nothing to connect to");

    assert_eq!(error, ControlTargetError::NoTarget);
    assert_eq!(error.to_string(), "--endpoint or --project required");
    assert_eq!(error.code(), "no_project");
}

#[test]
fn an_explicit_endpoint_without_a_project_carries_only_the_token_it_is_given() {
    let given = resolve_control_target(None, Some(tcp("127.0.0.1:9")), Some("given"))
        .expect("explicit endpoint");
    assert_eq!(given.source, EndpointSource::Explicit);
    assert_eq!(given.auth_token.as_deref(), Some("given"));

    let bare =
        resolve_control_target(None, Some(tcp("127.0.0.1:9")), None).expect("explicit endpoint");
    assert_eq!(bare.auth_token, None);
}

// Credential precedence (spec 11, section 6.8.1).

#[test]
fn an_explicit_token_beats_the_endpoint_file_and_the_configured_token() {
    let project = Scratch::new("token-explicit");
    project
        .write_runtime_toml("endpoint = \"tcp://127.0.0.1:9100\"\nauth_token = \"config-token\"");
    project.publish_file("tcp://127.0.0.1:9200", 1, Some("file-token"));

    let target = resolve_with_default(Some(project.path()), None, Some("explicit-token"), |_| {
        Ok(tcp("127.0.0.1:9400"))
    })
    .expect("resolves");

    assert_eq!(target.source, EndpointSource::EndpointFile);
    assert_eq!(target.auth_token.as_deref(), Some("explicit-token"));
}

#[test]
fn the_endpoint_file_token_beats_the_configured_token() {
    let project = Scratch::new("token-file");
    project
        .write_runtime_toml("endpoint = \"tcp://127.0.0.1:9100\"\nauth_token = \"config-token\"");
    project.publish_file("tcp://127.0.0.1:9200", 1, Some("file-token"));

    let target = resolve_with_default(Some(project.path()), None, None, |_| {
        Ok(tcp("127.0.0.1:9400"))
    })
    .expect("resolves");

    assert_eq!(target.auth_token.as_deref(), Some("file-token"));
}

#[test]
fn an_explicit_endpoint_takes_the_project_token_because_the_credential_is_independent() {
    let project = Scratch::new("token-override");
    project
        .write_runtime_toml("endpoint = \"tcp://127.0.0.1:9100\"\nauth_token = \"config-token\"");
    project.publish_file("tcp://127.0.0.1:9200", 1, Some("file-token"));

    let target = resolve_with_default(
        Some(project.path()),
        Some(tcp("127.0.0.1:9300")),
        None,
        |_| Ok(tcp("127.0.0.1:9400")),
    )
    .expect("resolves");

    assert_eq!(target.source, EndpointSource::Explicit);
    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9300");
    assert_eq!(target.auth_token.as_deref(), Some("file-token"));
}

#[test]
fn the_configured_token_is_used_when_the_endpoint_file_names_none() {
    let project = Scratch::new("token-configured");
    project
        .write_runtime_toml("endpoint = \"tcp://127.0.0.1:9100\"\nauth_token = \"config-token\"");
    project.publish_file("tcp://127.0.0.1:9200", 1, None);

    let target = resolve_with_default(Some(project.path()), None, None, |_| {
        Ok(tcp("127.0.0.1:9400"))
    })
    .expect("resolves");

    assert_eq!(target.auth_token.as_deref(), Some("config-token"));
}

#[test]
fn an_explicit_endpoint_and_token_do_not_read_the_project() {
    let project = Scratch::new("unreadable");
    std::fs::write(project.path().join("runtime.toml"), "not = [valid")
        .expect("write malformed runtime config");

    let target = resolve_with_default(
        Some(project.path()),
        Some(tcp("127.0.0.1:9300")),
        Some("explicit-token"),
        |_| panic!("no default is needed for an explicit endpoint"),
    )
    .expect("an explicit endpoint and token need no read of the project");

    assert_eq!(target.auth_token.as_deref(), Some("explicit-token"));
}

#[test]
fn a_folder_without_a_runtime_toml_configures_nothing() {
    let folder = Scratch::new("no-runtime-toml");

    let target = resolve_with_default(
        Some(folder.path()),
        Some(tcp("127.0.0.1:9300")),
        None,
        |_| panic!("an explicit endpoint needs no default"),
    )
    .expect("an explicit endpoint works from a folder that is not a project");

    assert_eq!(target.source, EndpointSource::Explicit);
    assert_eq!(target.auth_token, None);
}

#[test]
fn a_folder_without_a_runtime_toml_uses_the_platform_default() {
    let folder = Scratch::new("no-runtime-default");

    let target = resolve_with_default(Some(folder.path()), None, None, |_| {
        Ok(tcp("127.0.0.1:9400"))
    })
    .expect("the default resolves");

    assert_eq!(target.source, EndpointSource::Default);
}

#[test]
fn each_source_has_the_name_that_the_endpoint_report_uses() {
    assert_eq!(EndpointSource::Explicit.as_str(), "explicit");
    assert_eq!(EndpointSource::EndpointFile.as_str(), "endpoint_file");
    assert_eq!(EndpointSource::Configured.as_str(), "configured");
    assert_eq!(EndpointSource::Default.as_str(), "default");
}

// Failures and their stable codes (spec 11, section 6.8.0a).

#[test]
fn a_malformed_runtime_toml_is_invalid_config_when_it_is_needed() {
    let folder = Scratch::new("bad-runtime-toml");
    std::fs::write(folder.path().join("runtime.toml"), "not = [valid")
        .expect("write malformed runtime config");

    let error = resolve_with_default(Some(folder.path()), None, None, |_| {
        Ok(tcp("127.0.0.1:9400"))
    })
    .expect_err("a malformed runtime.toml must not be ignored");

    assert!(
        matches!(error, ControlTargetError::InvalidConfig(_)),
        "{error:?}"
    );
    assert_eq!(error.code(), "invalid_config");
}

#[test]
fn a_malformed_endpoint_file_or_one_of_another_schema_is_endpoint_file_invalid() {
    let project = Scratch::new("file-invalid-code");
    project
        .write_runtime_toml("endpoint = \"tcp://127.0.0.1:9100\"\nauth_token = \"config-token\"");
    std::fs::create_dir_all(project.path().join(ENDPOINT_DIR)).expect("endpoint directory");
    let path = project.path().join(ENDPOINT_DIR).join("control.json");

    for text in [
        "not json",
        r#"{"schema_version":2,"endpoint":"tcp://127.0.0.1:1","pid":1}"#,
        r#"{"schema_version":1,"endpoint":"pipe://nowhere","pid":1}"#,
    ] {
        std::fs::write(&path, text).expect("write endpoint file");

        let error = resolve_with_default(Some(project.path()), None, None, |_| {
            Ok(tcp("127.0.0.1:9400"))
        })
        .expect_err("an invalid endpoint file is an error, not a fallback");

        assert_eq!(error.code(), "endpoint_file_invalid", "{text}: {error}");
        assert!(error.to_string().contains("control.json"), "{error}");
    }
}

#[test]
fn a_missing_project_folder_is_no_project() {
    let parent = Scratch::new("missing-project");
    let missing = parent.path().join("absent");

    let error = resolve_with_default(Some(&missing), None, None, |_| {
        panic!("a missing folder has no default")
    })
    .expect_err("a missing project folder resolves nothing");

    assert_eq!(error.code(), "no_project");
    assert!(error.to_string().contains("absent"), "{error}");
}

#[test]
fn a_default_that_cannot_be_placed_is_default_unavailable() {
    let project = Scratch::new("default-unavailable");

    let error = resolve_with_default(Some(project.path()), None, None, |_| {
        Err(RuntimeError::ControlError(
            "runtime directory /tmp/x cannot hold a control socket".into(),
        ))
    })
    .expect_err("the default cannot be used");

    assert_eq!(error.code(), "default_unavailable");
    assert_eq!(
        error.to_string(),
        "runtime directory /tmp/x cannot hold a control socket"
    );
}

// An endpoint file that cannot be read is skipped (spec 11, section 6.8.0a).

/// A project whose endpoint file exists but cannot be read by this process, or `None` when this
/// process reads it anyway (a privileged user), so the test cannot run.
#[cfg(unix)]
fn project_with_unreadable_endpoint_file(label: &str, control_lines: &str) -> Option<Scratch> {
    let project = Scratch::new(label);
    project.write_runtime_toml(control_lines);
    project.publish_file("tcp://127.0.0.1:9200", 1, Some("file-token"));
    let path = project.path().join(ENDPOINT_DIR).join("control.json");
    super::test_support::set_mode(&path, 0o000);
    if super::test_support::readable_despite_mode(&path) {
        eprintln!("skipped: this process reads files whatever their mode");
        return None;
    }
    Some(project)
}

#[cfg(unix)]
#[test]
fn an_unreadable_endpoint_file_is_skipped_for_the_configured_endpoint_and_token() {
    let Some(project) = project_with_unreadable_endpoint_file(
        "skip-configured",
        "endpoint = \"tcp://127.0.0.1:9100\"\nauth_token = \"config-token\"",
    ) else {
        return;
    };

    let target = resolve_with_default(Some(project.path()), None, None, |_| {
        Ok(tcp("127.0.0.1:9400"))
    })
    .expect("the configured endpoint resolves");

    assert_eq!(target.source, EndpointSource::Configured);
    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9100");
    assert_eq!(target.auth_token.as_deref(), Some("config-token"));
    let reason = target.endpoint_file_skipped.expect("the skip has a reason");
    assert!(reason.contains("control.json"), "{reason}");
}

#[cfg(unix)]
#[test]
fn an_unreadable_endpoint_file_is_skipped_when_only_the_token_is_missing() {
    let Some(project) = project_with_unreadable_endpoint_file(
        "skip-token",
        "endpoint = \"tcp://127.0.0.1:9100\"\nauth_token = \"config-token\"",
    ) else {
        return;
    };

    let target = resolve_with_default(
        Some(project.path()),
        Some(tcp("127.0.0.1:9300")),
        None,
        |_| panic!("an explicit endpoint needs no default"),
    )
    .expect("an explicit endpoint resolves");

    assert_eq!(target.source, EndpointSource::Explicit);
    assert_eq!(target.auth_token.as_deref(), Some("config-token"));
    assert!(target.endpoint_file_skipped.is_some());
}

#[cfg(unix)]
#[test]
fn an_unreadable_endpoint_file_is_skipped_for_the_platform_default() {
    let Some(project) = project_with_unreadable_endpoint_file("skip-default", "") else {
        return;
    };

    let target = resolve_with_default(Some(project.path()), None, None, |_| {
        Ok(tcp("127.0.0.1:9400"))
    })
    .expect("the default resolves");

    assert_eq!(target.source, EndpointSource::Default);
    assert!(target.endpoint_file_skipped.is_some());
}

#[cfg(unix)]
#[test]
fn an_unreadable_endpoint_file_with_the_windows_default_is_endpoint_file_unreadable() {
    let Some(project) = project_with_unreadable_endpoint_file("skip-unbound", "") else {
        return;
    };

    let error = resolve_with_default(Some(project.path()), None, None, |_| Ok(tcp("127.0.0.1:0")))
        .expect_err("nothing names a port");

    assert_eq!(error.code(), "endpoint_file_unreadable");
    assert!(error.to_string().contains("control.json"), "{error}");
}

// The token of a Windows runtime lives in a per-user token file (spec 11, section 6.8.0a).

#[test]
fn with_a_per_user_token_folder_the_file_token_comes_from_the_token_file() {
    let project = Scratch::new("token-home-project");
    let token_home = Scratch::new("token-home");
    project.write_runtime_toml("auth_token = \"config-token\"");
    let published = PublishedEndpoint::publish_with(
        project.path(),
        &tcp("127.0.0.1:5555"),
        Some("generated-token"),
        Some(token_home.path()),
    )
    .expect("publish");

    let target = resolve_with(
        Some(project.path()),
        None,
        None,
        |_| Ok(tcp("127.0.0.1:0")),
        Some(token_home.path()),
    )
    .expect("the published endpoint resolves");

    assert_eq!(target.source, EndpointSource::EndpointFile);
    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:5555");
    assert_eq!(target.auth_token.as_deref(), Some("generated-token"));
    drop(published);
}

#[test]
fn a_token_file_of_another_start_is_not_offered() {
    let project = Scratch::new("token-home-stale-project");
    let token_home = Scratch::new("token-home-stale");
    project.write_runtime_toml("auth_token = \"config-token\"");
    let published = PublishedEndpoint::publish_with(
        project.path(),
        &tcp("127.0.0.1:5555"),
        Some("generated-token"),
        Some(token_home.path()),
    )
    .expect("publish");
    // A newer start of another process published another port without generating a token.
    project.publish_file(
        "tcp://127.0.0.1:6666",
        std::process::id().wrapping_add(1),
        None,
    );

    let target = resolve_with(
        Some(project.path()),
        None,
        None,
        |_| Ok(tcp("127.0.0.1:0")),
        Some(token_home.path()),
    )
    .expect("the newer endpoint resolves");

    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:6666");
    assert_eq!(target.auth_token.as_deref(), Some("config-token"));
    drop(published);
}

// A failed connection (spec 11, section 6.8.0a).

fn target_from(source: EndpointSource, skipped: Option<&str>) -> ControlTarget {
    let mut target = ControlTarget::new(tcp("127.0.0.1:9"), source, None);
    target.endpoint_file_skipped = skipped.map(str::to_string);
    target
}

#[test]
fn nothing_listening_at_the_endpoint_file_or_the_default_means_not_running() {
    for source in [EndpointSource::EndpointFile, EndpointSource::Default] {
        for kind in [
            std::io::ErrorKind::ConnectionRefused,
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::TimedOut,
        ] {
            let message = target_from(source, None).connect_error(&std::io::Error::from(kind));
            assert!(
                message.starts_with("runtime is not running"),
                "{source:?} {kind:?}: {message}"
            );
            assert!(message.contains("tcp://127.0.0.1:9"), "{message}");
        }
    }
}

#[test]
fn a_refused_explicit_or_configured_endpoint_is_reported_as_it_is() {
    for source in [EndpointSource::Explicit, EndpointSource::Configured] {
        let message = target_from(source, None)
            .connect_error(&std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
        assert!(
            message.starts_with("connect control endpoint tcp://127.0.0.1:9"),
            "{source:?}: {message}"
        );
    }
}

#[test]
fn another_connection_failure_at_the_endpoint_file_is_reported_as_it_is() {
    let message = target_from(EndpointSource::EndpointFile, None)
        .connect_error(&std::io::Error::from(std::io::ErrorKind::PermissionDenied));

    assert!(
        message.starts_with("connect control endpoint tcp://127.0.0.1:9"),
        "{message}"
    );
}

#[test]
fn a_skipped_endpoint_file_is_named_in_the_connection_failure() {
    let message = target_from(EndpointSource::Default, Some("read x/control.json: denied"))
        .connect_error(&std::io::Error::from(std::io::ErrorKind::PermissionDenied));

    assert!(
        message.ends_with("; the endpoint file was skipped: read x/control.json: denied"),
        "{message}"
    );
}

// Tokens never appear in debug output (spec 11, section 6.8.0a).

#[test]
fn a_target_and_a_binding_redact_their_token_in_debug_output() {
    let mut target = target_from(EndpointSource::EndpointFile, None);
    target.auth_token = Some("secret-token-value".to_string());
    let binding = ControlBinding {
        endpoint: tcp("127.0.0.1:9"),
        auth_token: Some("secret-token-value".into()),
        generated_token: true,
    };

    for text in [format!("{target:?}"), format!("{binding:?}")] {
        assert!(!text.contains("secret-token-value"), "{text}");
        assert!(text.contains("<redacted>"), "{text}");
    }
}

// The project's endpoint text, for display (spec 11, section 6.8.0a).

#[test]
fn the_project_endpoint_text_prefers_the_endpoint_file_then_the_configured_endpoint() {
    let project = Scratch::new("text-configured");
    assert_eq!(
        project_control_endpoint_text(project.path(), Some("tcp://127.0.0.1:9100")).as_deref(),
        Some("tcp://127.0.0.1:9100")
    );

    project.publish_file("tcp://127.0.0.1:9200", 1, None);
    assert_eq!(
        project_control_endpoint_text(project.path(), Some("tcp://127.0.0.1:9100")).as_deref(),
        Some("tcp://127.0.0.1:9200")
    );
}

#[cfg(unix)]
#[test]
fn the_unix_default_is_known_before_any_runtime_runs() {
    let project = Scratch::new("text-default");

    let text = project_control_endpoint_text(project.path(), None)
        .expect("the Unix default has a socket path");

    assert!(text.starts_with("unix://"), "{text}");
    assert!(text.ends_with(".sock"), "{text}");
}

#[cfg(not(unix))]
#[test]
fn the_windows_default_is_unknown_until_a_runtime_publishes_its_port() {
    let project = Scratch::new("text-unbound");

    assert_eq!(project_control_endpoint_text(project.path(), None), None);
}

// Binding (spec 11, section 6.8.0a).

#[test]
fn a_configured_endpoint_binds_as_written_with_its_configured_token() {
    let project = Scratch::new("binding-configured");

    let binding = control_binding(
        project.path(),
        Some("tcp://127.0.0.1:9500"),
        Some("configured-token"),
    )
    .expect("configured binding");

    assert!(matches!(
        binding.endpoint,
        ControlEndpoint::Tcp(address) if address.port() == 9500
    ));
    assert_eq!(binding.auth_token.as_deref(), Some("configured-token"));
    assert!(!binding.generated_token);
}

#[test]
fn a_configured_tcp_endpoint_without_a_token_gets_no_generated_token() {
    let binding = binding_for(tcp("127.0.0.1:9500"), None, false).expect("binding");

    assert_eq!(binding.auth_token, None);
    assert!(!binding.generated_token);
}

#[test]
fn a_default_tcp_endpoint_without_a_configured_token_gets_a_generated_one() {
    let binding = binding_for(tcp("127.0.0.1:0"), None, true).expect("binding");

    let token = binding.auth_token.as_deref().expect("a generated token");
    assert_eq!(token.len(), 43);
    assert!(binding.generated_token);
}

#[test]
fn a_default_tcp_endpoint_keeps_a_configured_token_and_generates_nothing() {
    let binding = binding_for(tcp("127.0.0.1:0"), Some("configured-token"), true).expect("binding");

    assert_eq!(binding.auth_token.as_deref(), Some("configured-token"));
    assert!(!binding.generated_token);
}

#[cfg(not(unix))]
#[test]
fn the_windows_default_binds_loopback_port_zero_with_a_generated_token() {
    let project = Scratch::new("binding-windows");

    let binding = control_binding(project.path(), None, None).expect("default binding");

    assert!(matches!(
        binding.endpoint,
        ControlEndpoint::Tcp(address) if address.port() == 0
    ));
    assert_eq!(binding.auth_token.as_deref().map(str::len), Some(43));
    assert!(binding.generated_token);
}

#[cfg(unix)]
#[test]
fn the_unix_default_gets_no_generated_token_and_a_private_directory() {
    use std::os::unix::fs::PermissionsExt;

    let project = Scratch::new("binding-default");

    let binding = control_binding(project.path(), None, None).expect("default binding");

    assert!(matches!(binding.endpoint, ControlEndpoint::Unix(_)));
    assert_eq!(binding.auth_token, None);
    assert!(!binding.generated_token);
    let directory = project.path().join(ENDPOINT_DIR);
    assert!(directory.is_dir());
    let mode = std::fs::metadata(&directory)
        .expect("directory metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
}
