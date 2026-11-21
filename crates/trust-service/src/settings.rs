//! A service's settings (specification 48 RU-62, Appendix B): its name, the runtime's control
//! endpoint, its service token and its heartbeat's period, read from `[services.<name>]` of the
//! project's `trust.toml`, or given by the service's own code. A section without `endpoint` takes
//! the endpoint that the project's running runtime published (specification 11 §6.8.0a).

use std::fmt;
use std::path::Path;
use std::time::Duration;

use trust_runtime_client::project_files::{endpoint_file_path, read_endpoint_file};

use crate::ServiceError;

/// What a service needs to reach its runtime.
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceSettings {
    /// The service's name, as `trust.toml` declares it in `[services.<name>]`.
    pub name: String,
    /// The runtime's control endpoint (specification 11 §6.8): `tcp://127.0.0.1:47800`,
    /// `unix:///run/trust.sock`.
    pub endpoint: String,
    /// The service token the service presents (specification 11 §6.9.6).
    pub token: String,
    /// The period at which the service sends its heartbeat (RU-63).
    pub heartbeat: Duration,
}

impl fmt::Debug for ServiceSettings {
    /// Every setting, with the token reduced to its last four characters, so that a log never
    /// holds a secret.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let start = self
            .token
            .char_indices()
            .rev()
            .nth(3)
            .map_or(0, |(index, _)| index);
        let tail = &self.token[start..];
        f.debug_struct("ServiceSettings")
            .field("name", &self.name)
            .field("endpoint", &self.endpoint)
            .field("token", &format!("…{tail}"))
            .field("heartbeat", &self.heartbeat)
            .finish()
    }
}

impl ServiceSettings {
    /// The settings of the service `name` in the `trust.toml` of the project in `project`: its
    /// `endpoint`, the service token in its `credentials` file, a path relative to the project,
    /// and its `heartbeat`, `500ms` or `2s`. Without `endpoint` the service connects to the
    /// endpoint that the project's running runtime published in its endpoint file, as
    /// `trust-runtime ctl` finds it (specification 11 §6.8.0a).
    ///
    /// # Errors
    ///
    /// [`ServiceError::Settings`] naming what is missing or does not read: the file, the service,
    /// its `credentials` or `heartbeat`, the token's file, an `endpoint` that is no text, or,
    /// without one, the endpoint file of a runtime that is not running or does not read.
    pub fn from_project(project: &Path, name: &str) -> Result<Self, ServiceError> {
        let file = project.join("trust.toml");
        let text = std::fs::read_to_string(&file)
            .map_err(|error| settings(format!("{}: {error}", file.display())))?;
        let document: toml::Table = toml::from_str(&text)
            .map_err(|error| settings(format!("{}: {error}", file.display())))?;
        let service = document
            .get("services")
            .and_then(|services| services.get(name))
            .and_then(toml::Value::as_table)
            .ok_or_else(|| settings(format!("trust.toml declares no [services.{name}]")))?;
        let text_of = |key: &str| {
            service
                .get(key)
                .and_then(toml::Value::as_str)
                .ok_or_else(|| settings(format!("[services.{name}] has no {key}")))
        };
        let endpoint = match service.get("endpoint") {
            None => published_endpoint(project, name)?,
            Some(endpoint) => endpoint.as_str().map(str::to_string).ok_or_else(|| {
                settings(format!(
                    "[services.{name}] endpoint {endpoint} is no text such as \
                     \"tcp://127.0.0.1:47800\""
                ))
            })?,
        };
        let credentials = project.join(text_of("credentials")?);
        let token = std::fs::read_to_string(&credentials)
            .map_err(|error| {
                settings(format!(
                    "the service token in {}: {error}; create it with `trust-runtime ctl \
                     service-token {name} --out <file>`",
                    credentials.display()
                ))
            })?
            .trim()
            .to_string();
        let heartbeat = text_of("heartbeat")?;
        let heartbeat = duration(heartbeat).ok_or_else(|| {
            settings(format!(
                "[services.{name}] heartbeat {heartbeat:?} is no duration such as \"500ms\" or \
                 \"2s\""
            ))
        })?;
        Ok(Self {
            name: name.to_string(),
            endpoint,
            token,
            heartbeat,
        })
    }
}

fn settings(message: String) -> ServiceError {
    ServiceError::Settings(message)
}

/// The endpoint that the project's running runtime published in its endpoint file (specification
/// 11 §6.8.0a), for a service whose section names none. Every running runtime publishes the
/// endpoint it bound there, a free port chosen at its start included.
fn published_endpoint(project: &Path, name: &str) -> Result<String, ServiceError> {
    match read_endpoint_file(project) {
        Ok(Some(file)) => Ok(file.endpoint),
        Ok(None) => Err(settings(format!(
            "[services.{name}] names no endpoint, and no runtime of the project has published one: \
             no endpoint file at {}",
            endpoint_file_path(project).display()
        ))),
        Err(error) => Err(settings(format!(
            "[services.{name}] names no endpoint, and the endpoint file of the project's runtime \
             does not read: {error}"
        ))),
    }
}

/// A period of `trust.toml` above zero: a whole number of `ns`, `us`, `ms`, `s` or `min`.
pub(crate) fn duration(text: &str) -> Option<Duration> {
    let digits = text.trim().find(|c: char| !c.is_ascii_digit())?;
    let (number, unit) = text.trim().split_at(digits);
    let number: u64 = number.parse().ok()?;
    let period = match unit {
        "ns" => Duration::from_nanos(number),
        "us" => Duration::from_micros(number),
        "ms" => Duration::from_millis(number),
        "s" => Duration::from_secs(number),
        "min" => Duration::from_secs(number.checked_mul(60)?),
        _ => return None,
    };
    (!period.is_zero()).then_some(period)
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
