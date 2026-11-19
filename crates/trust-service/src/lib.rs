//! The SDK of truST services (specification 48 §13, RU-61 to RU-64): a service is an ordinary Rust
//! program beside a runtime that reaches it only over the control link (specification 11 §6.8),
//! with a service token of its own (specification 11 §6.9.6). [`Service`] reads the program's
//! variables by their paths, one completed scan's values at a time, writes the variables its
//! project allows, at the next cycle's start, and sends the heartbeat by which the program judges
//! whether the service is alive.
//!
//! ```no_run
//! use trust_service::Service;
//!
//! # fn main() -> Result<(), trust_service::ServiceError> {
//! // [services.recipe] of the project's trust.toml: endpoint, credentials and heartbeat.
//! let service = Service::connect("recipe")?;
//! let _heartbeat = service.start_heartbeat()?;
//! let snapshot = service.read(&["P1.level"])?;
//! let level: f64 = snapshot.get("P1.level")?;
//! service.write(&[("Recipe.target", (level + 1.0).into())])?;
//! # Ok(())
//! # }
//! ```

mod heartbeat;
mod settings;
mod snapshot;

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_runtime_client::{ControlClient, ControlEndpoint, ControlError};

pub use heartbeat::Heartbeat;
pub use settings::ServiceSettings;
pub use snapshot::{FromVariable, Quality, Snapshot, Variable};

/// How long a connection waits for each read and write.
const TIMEOUT: Duration = Duration::from_secs(5);

/// Why a service's request has no result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ServiceError {
    /// The service's settings are missing or do not read.
    #[error("{0}")]
    Settings(String),
    /// The connection failed or the runtime refused the request, with the runtime's text.
    #[error(transparent)]
    Control(#[from] ControlError),
    /// The runtime's reply is not the one specification 11 §6.8.1 gives the request.
    #[error(
        "the runtime's reply to {request} is not as specification 11 §6.8.1 gives it: {reply}"
    )]
    Reply {
        /// The request.
        request: &'static str,
        /// The reply's text.
        reply: String,
    },
    /// The read did not name the path.
    #[error("{0} is not among the variables the read named")]
    NotRead(String),
    /// The heartbeat's thread did not start.
    #[error("the heartbeat's thread did not start: {0}")]
    Thread(String),
    /// The variable's value is no value of the Rust type asked for.
    #[error("{path} holds a {type_name}, which is no {wanted}")]
    Type {
        /// The variable's path.
        path: String,
        /// Its declared type.
        type_name: String,
        /// The Rust type asked for.
        wanted: &'static str,
    },
}

/// A service's connection to its runtime. It answers one request at a time; a service that works
/// on several threads shares one `Service` or connects one per thread.
pub struct Service {
    settings: ServiceSettings,
    client: Mutex<ControlClient>,
}

impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Service")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

impl Service {
    /// Connects the service `name` with the settings of `[services.<name>]` in the project's
    /// `trust.toml`: in the folder `TRUST_PROJECT` names, else the current folder.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Settings`] for settings that are missing or do not read, and
    /// [`ServiceError::Control`] for an endpoint that cannot be reached.
    pub fn connect(name: &str) -> Result<Self, ServiceError> {
        let project = std::env::var_os("TRUST_PROJECT").map_or_else(
            || std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            PathBuf::from,
        );
        Self::connect_with(ServiceSettings::from_project(&project, name)?)
    }

    /// Connects with `settings`.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Settings`] for an endpoint that is no control endpoint, and
    /// [`ServiceError::Control`] for one that cannot be reached.
    pub fn connect_with(settings: ServiceSettings) -> Result<Self, ServiceError> {
        let client = open(&settings)?;
        Ok(Self {
            settings,
            client: Mutex::new(client),
        })
    }

    /// The service's settings.
    #[must_use]
    pub fn settings(&self) -> &ServiceSettings {
        &self.settings
    }

    /// One completed scan's values of the variables `paths` names (`variables.read`).
    ///
    /// # Errors
    ///
    /// [`ServiceError::Control`] with the runtime's refusal: a path that names no variable or no
    /// single value, no completed scan yet, a debugger's hold.
    pub fn read(&self, paths: &[&str]) -> Result<Snapshot, ServiceError> {
        let reply = self.request("variables.read", json!({ "paths": paths }))?;
        Snapshot::of_reply(&reply).ok_or_else(|| ServiceError::Reply {
            request: "variables.read",
            reply: reply.to_string(),
        })
    }

    /// Writes `writes`, each a path and a value in the JSON form of the variable's type
    /// (`variables.write`): the next cycle's start writes the set together, as the program's input.
    /// Returns how many values that cycle writes.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Control`] with the runtime's refusal, which writes nothing: a path the
    /// service's `writes` does not name, a value its variable's type does not take, a token without
    /// the Engineer role.
    pub fn write(&self, writes: &[(&str, Value)]) -> Result<usize, ServiceError> {
        let writes: Vec<Value> = writes
            .iter()
            .map(|(path, value)| json!({ "path": path, "value": value }))
            .collect();
        let reply = self.request("variables.write", json!({ "writes": writes }))?;
        reply
            .get("count")
            .and_then(Value::as_u64)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or_else(|| ServiceError::Reply {
                request: "variables.write",
                reply: reply.to_string(),
            })
    }

    /// Sends one heartbeat (`service.heartbeat`), and returns the value the next cycle's start
    /// writes into the service's heartbeat.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Control`] with the runtime's refusal: a resource that does not run, a
    /// program without the service's heartbeat.
    pub fn heartbeat(&self) -> Result<u32, ServiceError> {
        let reply = self.request("service.heartbeat", Value::Null)?;
        reply
            .get("value")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| ServiceError::Reply {
                request: "service.heartbeat",
                reply: reply.to_string(),
            })
    }

    /// Sends the service's heartbeat at its `heartbeat` period on a connection and a thread of
    /// their own, until the returned [`Heartbeat`] is dropped, so that the service's own work never
    /// delays it (RU-63).
    ///
    /// # Errors
    ///
    /// [`ServiceError::Control`] for an endpoint that cannot be reached.
    pub fn start_heartbeat(&self) -> Result<Heartbeat, ServiceError> {
        Heartbeat::start(Self::connect_with(self.settings.clone())?)
    }

    /// Reads the variables `paths` names at `period`: each item one completed scan's values, the
    /// first at once and each next one `period` after the one before.
    pub fn subscribe<'s>(&'s self, paths: &'s [&'s str], period: Duration) -> Subscription<'s> {
        Subscription {
            service: self,
            paths,
            period,
            next: None,
        }
    }

    fn request(&self, kind: &str, params: Value) -> Result<Value, ServiceError> {
        let mut client = self.client.lock().unwrap_or_else(PoisonError::into_inner);
        let params = (!params.is_null()).then_some(params);
        Ok(client.request(kind, params)?)
    }
}

/// The client of `settings`' endpoint, presenting the service's token.
fn open(settings: &ServiceSettings) -> Result<ControlClient, ServiceError> {
    let endpoint = ControlEndpoint::parse(&settings.endpoint).ok_or_else(|| {
        ServiceError::Settings(format!(
            "{} is no control endpoint such as tcp://127.0.0.1:47800",
            settings.endpoint
        ))
    })?;
    Ok(ControlClient::connect(
        &endpoint,
        Some(&settings.token),
        TIMEOUT,
    )?)
}

/// The reads of [`Service::subscribe`].
#[derive(Debug)]
pub struct Subscription<'s> {
    service: &'s Service,
    paths: &'s [&'s str],
    period: Duration,
    next: Option<Instant>,
}

impl Iterator for Subscription<'_> {
    type Item = Result<Snapshot, ServiceError>;

    /// Waits until the read is due, then reads; never ends.
    fn next(&mut self) -> Option<Self::Item> {
        let now = Instant::now();
        let due = self.next.unwrap_or(now);
        if due > now {
            std::thread::sleep(due - now);
        }
        self.next = Some(due + self.period);
        Some(self.service.read(self.paths))
    }
}
