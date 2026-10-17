//! The control endpoint a project's runtime listens on, and the endpoint a client connects to
//! (spec 11, sections 6.8.0a and 6.8.1).
//!
//! A runtime without `[runtime.control] endpoint` listens on the platform default and publishes
//! where it is in `<project>/.trust-runtime/control.json`. A client resolves an endpoint in this
//! order: an explicit endpoint, the endpoint file, the configured endpoint, and the platform
//! default. Every client uses this one module, so the order and the meaning of a failed connection
//! are defined in one place.

mod defaults;
mod file;
mod gitignore;
mod token_file;

#[cfg(test)]
mod defaults_tests;
#[cfg(test)]
mod publication_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

use std::fmt;
use std::io;
use std::path::Path;

use smol_str::SmolStr;

use crate::config::RuntimeConfig;
use crate::error::RuntimeError;

use super::ControlEndpoint;

pub use defaults::{generate_control_token, platform_default_endpoint};
pub use file::{
    remove_endpoint_file_of, withdraw_auth_token, PublishedEndpoint, WithdrawnToken, ENDPOINT_DIR,
    ENDPOINT_FILE,
};
pub use gitignore::{
    ensure_endpoint_dir_ignored, ENDPOINT_DIR_GITIGNORE, ENDPOINT_DIR_GITIGNORE_LINE,
};

use defaults::default_binding_endpoint;
use file::{
    endpoint_file_path, published_token, read_endpoint_file, token_directory, EndpointFileError,
};

/// Where a client found the endpoint it resolved (spec 11, section 6.8.0a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointSource {
    /// The caller named the endpoint with `--endpoint` or an explicit `endpoint` argument.
    Explicit,
    /// The endpoint file that the running runtime published.
    EndpointFile,
    /// The `[runtime.control] endpoint` of the project's `runtime.toml`.
    Configured,
    /// The platform default for the project.
    Default,
}

impl EndpointSource {
    /// The name that `trust-runtime ctl --project <dir> endpoint --json` reports for a source.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::EndpointFile => "endpoint_file",
            Self::Configured => "configured",
            Self::Default => "default",
        }
    }

    /// Where the endpoint came from, in words for a message.
    fn origin(self) -> &'static str {
        match self {
            Self::Explicit => "the explicit endpoint",
            Self::EndpointFile => "the endpoint file",
            Self::Configured => "the configured endpoint",
            Self::Default => "the platform default",
        }
    }
}

/// The endpoint and the credential that a client connects with.
#[derive(Clone)]
pub struct ControlTarget {
    /// The endpoint to connect to.
    pub endpoint: ControlEndpoint,
    /// The endpoint as text: `tcp://` and an address, or `unix://` and a path.
    pub endpoint_text: String,
    /// Where the endpoint came from.
    pub source: EndpointSource,
    /// The token to send with each request, or `None` when the endpoint needs none.
    pub auth_token: Option<String>,
    /// Why the project's endpoint file was skipped, when it exists but could not be read.
    pub endpoint_file_skipped: Option<String>,
}

impl fmt::Debug for ControlTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlTarget")
            .field("endpoint_text", &self.endpoint_text)
            .field("source", &self.source)
            .field(
                "auth_token",
                &self.auth_token.as_ref().map(|_| "<redacted>"),
            )
            .field("endpoint_file_skipped", &self.endpoint_file_skipped)
            .finish()
    }
}

impl ControlTarget {
    fn new(endpoint: ControlEndpoint, source: EndpointSource, auth_token: Option<String>) -> Self {
        Self {
            endpoint_text: endpoint.to_string(),
            endpoint,
            source,
            auth_token,
            endpoint_file_skipped: None,
        }
    }

    /// The message for a connection to this target that failed with `error` (spec 11, section
    /// 6.8.0a). When the endpoint came from the endpoint file or is the platform default and
    /// nothing listens there, the project's runtime is not running. Nothing listens when the
    /// connection is refused or times out, or the socket does not exist: these endpoints are
    /// loopback ports and local sockets, where a listening runtime answers at once (Windows
    /// retries a refused loopback connection until a short connect timeout ends). Any other
    /// failure, such as a socket that belongs to another user, is reported as it is. A skipped
    /// endpoint file is named in both cases.
    #[must_use]
    pub fn connect_error(&self, error: &io::Error) -> String {
        let nothing_listens = matches!(
            error.kind(),
            io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound | io::ErrorKind::TimedOut
        );
        let project_runtime = matches!(
            self.source,
            EndpointSource::EndpointFile | EndpointSource::Default
        );
        let mut message = if nothing_listens && project_runtime {
            format!(
                "runtime is not running: nothing listens at {} from {} ({error})",
                self.endpoint_text,
                self.source.origin()
            )
        } else {
            format!("connect control endpoint {}: {error}", self.endpoint_text)
        };
        if let Some(reason) = &self.endpoint_file_skipped {
            message.push_str("; the endpoint file was skipped: ");
            message.push_str(reason);
        }
        message
    }
}

/// Why a client could not resolve a control target. Each case has the stable code that
/// `trust-runtime ctl --project <dir> endpoint --json` reports (spec 11, section 6.8.0a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlTargetError {
    /// Neither an endpoint nor a project was given.
    NoTarget,
    /// The project folder does not exist.
    NoProject(String),
    /// The project's `runtime.toml` cannot be loaded, or its endpoint does not parse.
    InvalidConfig(String),
    /// The endpoint file does not parse, has another schema version, or names an endpoint that
    /// does not parse.
    EndpointFileInvalid(String),
    /// The endpoint file exists but cannot be read, and nothing else names an endpoint: the
    /// Windows default has no port until the runtime publishes it.
    EndpointFileUnreadable(String),
    /// The platform default has no port until a runtime binds it, and no endpoint file names one.
    NotRunning(String),
    /// The platform default cannot be used: its fallback socket would sit in a runtime directory
    /// that is not private, or its path does not fit.
    DefaultUnavailable(String),
}

impl ControlTargetError {
    /// The stable code of this failure.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoTarget | Self::NoProject(_) => "no_project",
            Self::InvalidConfig(_) => "invalid_config",
            Self::EndpointFileInvalid(_) => "endpoint_file_invalid",
            Self::EndpointFileUnreadable(_) => "endpoint_file_unreadable",
            Self::NotRunning(_) => "not_running",
            Self::DefaultUnavailable(_) => "default_unavailable",
        }
    }
}

impl fmt::Display for ControlTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTarget => formatter.write_str("--endpoint or --project required"),
            Self::NoProject(message)
            | Self::InvalidConfig(message)
            | Self::EndpointFileInvalid(message)
            | Self::EndpointFileUnreadable(message)
            | Self::NotRunning(message)
            | Self::DefaultUnavailable(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ControlTargetError {}

/// What a runtime binds, and the token its control server requires (spec 11, section 6.8.0a).
#[derive(Clone)]
pub struct ControlBinding {
    /// The endpoint to bind. A TCP port of 0 is replaced by the bound port once the bind succeeds.
    pub endpoint: ControlEndpoint,
    /// The configured token, or the token generated for this start.
    pub auth_token: Option<SmolStr>,
    /// Whether `auth_token` was generated for this start, so the endpoint file must offer it.
    pub generated_token: bool,
}

impl fmt::Debug for ControlBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlBinding")
            .field("endpoint", &self.endpoint)
            .field(
                "auth_token",
                &self.auth_token.as_ref().map(|_| "<redacted>"),
            )
            .field("generated_token", &self.generated_token)
            .finish()
    }
}

/// Chooses what a project's runtime binds. A configured endpoint is used as written. When
/// `[runtime.control] endpoint` is omitted, the platform default is used, and a default TCP
/// endpoint without a configured token gets a generated one. On Unix-like systems a project whose
/// `.trust-runtime` folder cannot be created gets the fallback socket, so the runtime still starts.
///
/// # Errors
///
/// Returns an error when the configured endpoint does not parse, the default cannot be computed,
/// or the operating system's random source fails.
pub fn control_binding(
    project_root: &Path,
    configured_endpoint: Option<&str>,
    configured_token: Option<&str>,
) -> Result<ControlBinding, RuntimeError> {
    match configured_endpoint {
        Some(text) => binding_for(ControlEndpoint::parse(text)?, configured_token, false),
        None => binding_for(
            default_binding_endpoint(project_root)?,
            configured_token,
            true,
        ),
    }
}

/// The token policy of a binding. A configured token is used as it is. Otherwise a default TCP
/// endpoint gets a generated token, so that it never listens without one.
fn binding_for(
    endpoint: ControlEndpoint,
    configured_token: Option<&str>,
    is_default: bool,
) -> Result<ControlBinding, RuntimeError> {
    if let Some(token) = configured_token {
        return Ok(ControlBinding {
            endpoint,
            auth_token: Some(SmolStr::new(token)),
            generated_token: false,
        });
    }
    if is_default && matches!(endpoint, ControlEndpoint::Tcp(_)) {
        let token = generate_control_token()?;
        return Ok(ControlBinding {
            endpoint,
            auth_token: Some(SmolStr::new(token)),
            generated_token: true,
        });
    }
    Ok(ControlBinding {
        endpoint,
        auth_token: None,
        generated_token: false,
    })
}

/// Resolves the endpoint and the credential that a client uses for a project (spec 11, sections
/// 6.8.0a and 6.8.1).
///
/// `explicit` is an endpoint the caller named. `token` is the caller's token, already merged from
/// `--token` or an explicit argument with `TRUST_CTL_TOKEN` (see [`control_token_from_env`]).
/// Without a project only an explicit endpoint resolves, and it carries no project token. With a
/// project, the endpoint file and `runtime.toml` fill in what is missing. The project is not read
/// when both the endpoint and the token are already known. An endpoint file that exists but
/// cannot be read is skipped, and the target names the reason.
///
/// # Errors
///
/// Returns the [`ControlTargetError`] of the first step that fails.
pub fn resolve_control_target(
    project: Option<&Path>,
    explicit: Option<ControlEndpoint>,
    token: Option<&str>,
) -> Result<ControlTarget, ControlTargetError> {
    let token_dir = token_directory().ok().flatten();
    resolve_with(
        project,
        explicit,
        token,
        platform_default_endpoint,
        token_dir.as_deref(),
    )
}

/// The resolution itself, with the platform default and the per-user token folder supplied, so
/// that the Windows rules can be tested on any platform.
fn resolve_with(
    project: Option<&Path>,
    explicit: Option<ControlEndpoint>,
    token: Option<&str>,
    default: impl FnOnce(&Path) -> Result<ControlEndpoint, RuntimeError>,
    token_dir: Option<&Path>,
) -> Result<ControlTarget, ControlTargetError> {
    let Some(root) = project else {
        let endpoint = explicit.ok_or(ControlTargetError::NoTarget)?;
        let auth_token = token.map(str::to_string);
        return Ok(ControlTarget::new(
            endpoint,
            EndpointSource::Explicit,
            auth_token,
        ));
    };
    let mut skipped = None;
    let file = if explicit.is_none() || token.is_none() {
        match read_endpoint_file(root) {
            Ok(file) => file,
            Err(EndpointFileError::Unreadable(reason)) => {
                skipped = Some(reason);
                None
            }
            Err(EndpointFileError::Invalid(reason)) => {
                return Err(ControlTargetError::EndpointFileInvalid(reason))
            }
        }
    } else {
        None
    };
    let file_token = file
        .as_ref()
        .and_then(|file| published_token(file, root, token_dir));
    let needs_configured =
        (explicit.is_none() && file.is_none()) || (token.is_none() && file_token.is_none());
    let configured = if needs_configured {
        ConfiguredControl::load(root)
            .map_err(|error| ControlTargetError::InvalidConfig(error_text(&error)))?
    } else {
        ConfiguredControl::default()
    };
    let auth_token = token
        .map(str::to_string)
        .or(file_token)
        .or_else(|| configured.auth_token.clone());
    let (endpoint, source) = if let Some(endpoint) = explicit {
        (endpoint, EndpointSource::Explicit)
    } else if let Some(file) = file {
        let endpoint = ControlEndpoint::parse(&file.endpoint).map_err(|error| {
            ControlTargetError::EndpointFileInvalid(format!(
                "{} names endpoint {}: {}",
                endpoint_file_path(root).display(),
                file.endpoint,
                error_text(&error)
            ))
        })?;
        (endpoint, EndpointSource::EndpointFile)
    } else if let Some(text) = configured.endpoint {
        let endpoint = ControlEndpoint::parse(&text)
            .map_err(|error| ControlTargetError::InvalidConfig(error_text(&error)))?;
        (endpoint, EndpointSource::Configured)
    } else {
        if !root.is_dir() {
            return Err(ControlTargetError::NoProject(format!(
                "project folder {} does not exist",
                root.display()
            )));
        }
        let endpoint = default(root)
            .map_err(|error| ControlTargetError::DefaultUnavailable(error_text(&error)))?;
        if is_unbound_default(&endpoint) {
            return Err(match skipped {
                Some(reason) => ControlTargetError::EndpointFileUnreadable(format!(
                    "the endpoint file cannot be read, and the platform default has no port until the runtime publishes it: {reason}"
                )),
                None => ControlTargetError::NotRunning(not_running_message(root)),
            });
        }
        (endpoint, EndpointSource::Default)
    };
    let mut target = ControlTarget::new(endpoint, source, auth_token);
    target.endpoint_file_skipped = skipped;
    Ok(target)
}

/// The message of a [`RuntimeError`] without its category prefix.
fn error_text(error: &RuntimeError) -> String {
    match error {
        RuntimeError::ControlError(message)
        | RuntimeError::InvalidConfig(message)
        | RuntimeError::InvalidBundle(message) => message.to_string(),
        other => other.to_string(),
    }
}

/// The `[runtime.control]` values of a project's runtime.toml that a client reads.
#[derive(Default)]
struct ConfiguredControl {
    endpoint: Option<String>,
    auth_token: Option<String>,
}

impl ConfiguredControl {
    /// Reads `[runtime.control]`. A folder without a runtime.toml configures nothing, so an
    /// explicit endpoint still works from a workspace folder that is not a project. A runtime.toml
    /// that exists must parse.
    fn load(root: &Path) -> Result<Self, RuntimeError> {
        let path = root.join("runtime.toml");
        if !path.is_file() {
            return Ok(Self::default());
        }
        let config = RuntimeConfig::load(path)?;
        Ok(Self {
            endpoint: config.control_endpoint.map(|endpoint| endpoint.to_string()),
            auth_token: config.control_auth_token.map(|token| token.to_string()),
        })
    }
}

/// The Windows default, `tcp://127.0.0.1:0`, names no port: the operating system chooses one
/// when a runtime binds.
fn is_unbound_default(endpoint: &ControlEndpoint) -> bool {
    matches!(endpoint, ControlEndpoint::Tcp(address) if address.port() == 0)
}

fn not_running_message(root: &Path) -> String {
    format!(
        "runtime is not running: no endpoint file at {}",
        endpoint_file_path(root).display()
    )
}

/// The endpoint text that a project's clients would connect to: the endpoint file's endpoint,
/// else the configured endpoint, else the platform default. It is `None` when no endpoint is
/// known, which is the Windows default before a runtime runs. A file that cannot be read counts
/// as absent, because this is for display and not for connecting.
#[must_use]
pub fn project_control_endpoint_text(
    project_root: &Path,
    configured: Option<&str>,
) -> Option<String> {
    if let Ok(Some(file)) = read_endpoint_file(project_root) {
        return Some(file.endpoint);
    }
    if let Some(text) = configured {
        return Some(text.to_string());
    }
    let endpoint = platform_default_endpoint(project_root).ok()?;
    (!is_unbound_default(&endpoint)).then(|| endpoint.to_string())
}

/// The token in the `TRUST_CTL_TOKEN` environment variable, when it is set.
#[must_use]
pub fn control_token_from_env() -> Option<String> {
    std::env::var("TRUST_CTL_TOKEN").ok()
}

impl fmt::Display for ControlEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tcp(address) => write!(formatter, "tcp://{address}"),
            #[cfg(unix)]
            Self::Unix(path) => write!(formatter, "unix://{}", path.display()),
        }
    }
}
