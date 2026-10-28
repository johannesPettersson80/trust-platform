//! `trust-runtime ctl --project <dir> endpoint [--json]` (spec 11, section 6.8.0a): prints the
//! project's control endpoint and the token a client would use with it. Nothing is sent to the
//! runtime, so this works while the runtime is stopped.
//!
//! With `--json`, every failure prints `{"error":{"code","message"}}` with one of the stable codes
//! of [`ControlTargetError::code`], `invalid_argument` or `no_project`, and the command exits
//! non-zero.

use std::path::PathBuf;

use serde_json::{json, Value as JsonValue};
use trust_runtime::control::endpoint::{
    control_token_from_env, resolve_control_target, ControlTarget,
};

/// Why the endpoint could not be reported: a stable code and a message for a person.
#[derive(Debug, PartialEq, Eq)]
struct EndpointFailure {
    code: &'static str,
    message: String,
}

/// Runs `ctl endpoint`. A failure is printed as JSON when asked, and returned, so the process
/// exits non-zero either way.
pub(super) fn print_control_endpoint(
    bundle: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let token = token.or_else(control_token_from_env);
    match resolve_endpoint(bundle, endpoint.as_deref(), token.as_deref()) {
        Ok(target) => {
            if json {
                println!("{}", endpoint_report(&target));
            } else {
                println!("{}", target.endpoint_text);
                if let Some(reason) = &target.endpoint_file_skipped {
                    eprintln!("warning: the endpoint file was skipped: {reason}");
                }
            }
            Ok(())
        }
        Err(failure) => {
            if json {
                println!("{}", error_report(&failure));
            }
            anyhow::bail!("{}", failure.message)
        }
    }
}

/// Resolves the project's target for the report. `--endpoint` is refused, because the command
/// reports the project's own endpoint.
fn resolve_endpoint(
    bundle: Option<PathBuf>,
    endpoint: Option<&str>,
    token: Option<&str>,
) -> Result<ControlTarget, EndpointFailure> {
    if endpoint.is_some() {
        return Err(EndpointFailure {
            code: "invalid_argument",
            message: "ctl endpoint reports the project's endpoint and does not take --endpoint"
                .to_string(),
        });
    }
    let Some(project) = bundle else {
        return Err(EndpointFailure {
            code: "no_project",
            message: "--project required".to_string(),
        });
    };
    resolve_control_target(Some(project.as_path()), None, token).map_err(|error| EndpointFailure {
        code: error.code(),
        message: error.to_string(),
    })
}

/// The `ctl endpoint --json` object for a resolved target.
fn endpoint_report(target: &ControlTarget) -> JsonValue {
    json!({
        "endpoint": target.endpoint_text,
        "auth_token": target.auth_token,
        "source": target.source.as_str(),
        "endpoint_file_skipped": target.endpoint_file_skipped,
    })
}

/// The `ctl endpoint --json` error object.
fn error_report(failure: &EndpointFailure) -> JsonValue {
    json!({"error": {"code": failure.code, "message": failure.message}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_runtime::control::endpoint::EndpointSource;
    use trust_runtime::control::ControlEndpoint;

    fn target(source: EndpointSource, token: Option<&str>, skipped: Option<&str>) -> ControlTarget {
        let addr: std::net::SocketAddr = "127.0.0.1:9".parse().expect("address");
        ControlTarget {
            endpoint: ControlEndpoint::Tcp(addr),
            endpoint_text: "tcp://127.0.0.1:9".to_string(),
            source,
            auth_token: token.map(str::to_string),
            endpoint_file_skipped: skipped.map(str::to_string),
        }
    }

    /// A scratch folder, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default();
            let path = std::env::temp_dir().join(format!(
                "trust-ctl-endpoint-{label}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("create scratch folder");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn endpoint_report_names_each_resolved_source() {
        for (source, name) in [
            (EndpointSource::EndpointFile, "endpoint_file"),
            (EndpointSource::Configured, "configured"),
            (EndpointSource::Default, "default"),
        ] {
            assert_eq!(
                endpoint_report(&target(source, None, None)),
                json!({
                    "endpoint": "tcp://127.0.0.1:9",
                    "auth_token": null,
                    "source": name,
                    "endpoint_file_skipped": null,
                })
            );
        }
    }

    #[test]
    fn endpoint_report_carries_the_token_a_client_would_send_and_a_skipped_file() {
        let report = endpoint_report(&target(
            EndpointSource::Configured,
            Some("configured"),
            Some("read control.json: Permission denied"),
        ));

        assert_eq!(report["auth_token"], json!("configured"));
        assert_eq!(
            report["endpoint_file_skipped"],
            json!("read control.json: Permission denied")
        );
    }

    #[test]
    fn every_failure_is_the_documented_error_object() {
        assert_eq!(
            error_report(&EndpointFailure {
                code: "not_running",
                message: "runtime is not running: no endpoint file".to_string(),
            }),
            json!({
                "error": {
                    "code": "not_running",
                    "message": "runtime is not running: no endpoint file",
                }
            })
        );
    }

    #[test]
    fn an_endpoint_argument_is_invalid_and_a_missing_project_is_no_project() {
        let with_endpoint =
            resolve_endpoint(Some(PathBuf::from(".")), Some("tcp://127.0.0.1:9"), None)
                .expect_err("--endpoint is refused");
        assert_eq!(with_endpoint.code, "invalid_argument");

        let without_project =
            resolve_endpoint(None, None, None).expect_err("--project is required");
        assert_eq!(without_project.code, "no_project");
    }

    #[test]
    fn a_resolution_failure_keeps_the_resolver_code_and_message() {
        let folder = Scratch::new("invalid-file");
        let dir = folder.0.join(".trust-runtime");
        std::fs::create_dir_all(&dir).expect("endpoint folder");
        std::fs::write(dir.join("control.json"), "not json").expect("write endpoint file");

        let failure = resolve_endpoint(Some(folder.0.clone()), None, None)
            .expect_err("a malformed endpoint file is reported");

        assert_eq!(failure.code, "endpoint_file_invalid");
        assert!(
            failure.message.contains("control.json"),
            "{}",
            failure.message
        );

        let missing = resolve_endpoint(Some(folder.0.join("absent")), None, None)
            .expect_err("a missing folder is reported");
        assert_eq!(missing.code, "no_project");
    }
}
