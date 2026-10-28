//! The settings every runtime has, and the part of `config.get` and `config.set` that every
//! runtime answers alike (specification 11, the runtime configuration contract; specification 34
//! TARGET-07): the log level, the cycle interval, the watchdog, the fault policy, retain and
//! control. A computer adds the keys of its own services, its web server, discovery, mesh,
//! runtime cloud, real-time scheduling, OPC UA, simulation and observability; a device without an
//! operating system runs none of them and has only these keys.

pub mod values;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::{Map, Value};
use smol_str::SmolStr;
use trust_runtime_core::deployment::{Configuration, ControlMode, HangAction, LogLevel};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::Duration;
use trust_runtime_core::watchdog::{FaultPolicy, RetainMode, WatchdogAction, WatchdogPolicy};

use crate::resource::commands::SchedulerPolicies;
use crate::status::StatusSettings;
use values::{
    config_type_error, config_value_error, expect_bool, expect_duration_millis,
    expect_non_empty_string,
};

/// The `config.set` keys whose values the scheduler holds as copies: the watchdog, the fault
/// policy and the retain save interval. They change at the program's next boundary, all at once.
pub const SCHEDULER_POLICY_KEYS: [&str; 5] = [
    "watchdog.enabled",
    "watchdog.timeout_ms",
    "watchdog.action",
    "fault.policy",
    "retain.save_interval_ms",
];

/// The settings every runtime has: a computer's from `runtime.toml`, a chip's from the deployment
/// record of its program (specification 34 §5.1.1, section 7). The control token is kept apart:
/// `config.get` reports only whether there is one and its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSettings {
    /// `runtime.log.level`.
    pub log_level: LogLevel,
    /// `resource.cycle_interval_ms`, which `config.set` does not change.
    pub cycle_interval: Duration,
    /// `[runtime.watchdog]`.
    pub watchdog: WatchdogPolicy,
    /// `runtime.watchdog.hang_action`.
    pub hang_action: HangAction,
    /// `[runtime.fault] policy`.
    pub fault_policy: FaultPolicy,
    /// `[runtime.retain] mode`.
    pub retain_mode: RetainMode,
    /// `[runtime.retain] save_interval_ms`, none where retained values are not saved at an
    /// interval.
    pub retain_save_interval: Option<Duration>,
    /// `runtime.control.mode`.
    pub control_mode: ControlMode,
    /// `runtime.control.debug_enabled`.
    pub debug_enabled: bool,
}

impl RuntimeSettings {
    /// The settings of a deployment record's configuration section (specification 34 §5.1.1,
    /// section 7), which a chip's program runs with.
    #[must_use]
    pub fn from_record(config: &Configuration) -> Self {
        Self {
            log_level: config.log_level,
            cycle_interval: Duration::from_nanos(
                i64::try_from(config.cycle_interval_nanos).unwrap_or(i64::MAX),
            ),
            watchdog: config.watchdog,
            hang_action: config.watchdog_hang_action,
            fault_policy: config.fault_policy,
            retain_mode: config.retain_mode,
            retain_save_interval: Some(config.retain_save_interval),
            control_mode: config.control_mode,
            debug_enabled: config.control_debug_enabled,
        }
    }

    /// The scheduler's copies of these settings.
    #[must_use]
    pub fn policies(&self) -> SchedulerPolicies {
        SchedulerPolicies {
            watchdog: self.watchdog,
            fault_policy: self.fault_policy,
            retain_save_interval: self.retain_save_interval,
        }
    }
}

/// Writes `config.get`'s keys of `settings` into `out`, each value in the spelling the
/// configuration takes. `token_length` is the length of the control token, none without one; the
/// token itself is never written. `status` gives the execution backend and its source, as
/// `status` reports them.
pub fn project(
    settings: &RuntimeSettings,
    token_length: Option<usize>,
    status: &StatusSettings,
    out: &mut Map<String, Value>,
) {
    let entries: [(&str, Value); 15] = [
        ("log.level", settings.log_level.as_str().into()),
        ("watchdog.enabled", settings.watchdog.enabled.into()),
        (
            "watchdog.timeout_ms",
            settings.watchdog.timeout.as_millis().into(),
        ),
        ("watchdog.action", settings.watchdog.action.as_str().into()),
        ("watchdog.hang_action", settings.hang_action.as_str().into()),
        (
            "resource.cycle_interval_ms",
            settings.cycle_interval.as_millis().into(),
        ),
        ("runtime.execution_backend", status.execution_backend.into()),
        (
            "runtime.execution_backend_source",
            status.execution_backend_source.into(),
        ),
        ("fault.policy", settings.fault_policy.as_str().into()),
        ("retain.mode", settings.retain_mode.as_str().into()),
        (
            "retain.save_interval_ms",
            settings
                .retain_save_interval
                .map(Duration::as_millis)
                .into(),
        ),
        ("control.auth_token_set", token_length.is_some().into()),
        ("control.auth_token_length", token_length.into()),
        ("control.debug_enabled", settings.debug_enabled.into()),
        ("control.mode", settings.control_mode.as_str().into()),
    ];
    for (key, value) in entries {
        out.insert(key.to_string(), value);
    }
}

/// What a `config.set` does with the control token.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TokenChange {
    /// The request leaves the token as it is.
    #[default]
    Keep,
    /// The request sets this token.
    Set(SmolStr),
    /// The request clears the token.
    Clear,
}

impl TokenChange {
    /// Whether a token is set after the change, where `before` says whether one was set before it.
    #[must_use]
    pub fn leaves_token(&self, before: bool) -> bool {
        match self {
            Self::Keep => before,
            Self::Set(_) => true,
            Self::Clear => false,
        }
    }

    /// Changes `token` as the request does.
    pub fn apply_to(self, token: &mut Option<SmolStr>) {
        match self {
            Self::Keep => {}
            Self::Set(new) => *token = Some(new),
            Self::Clear => *token = None,
        }
    }
}

/// A `config.set` request staged on a copy of the settings: nothing has changed until the host
/// commits it, all of it, after every check passed (specification 11, the runtime configuration
/// contract).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSettings {
    /// The settings as the request leaves them.
    pub settings: RuntimeSettings,
    /// What the request does with the control token.
    pub token: TokenChange,
    /// Each key the request set, in the order it set them.
    pub updated: Vec<&'static str>,
    /// The keys whose service takes the new value only when it starts again.
    pub restart_required: Vec<&'static str>,
}

impl StagedSettings {
    /// A request that has changed nothing of `settings` yet.
    #[must_use]
    pub fn new(settings: RuntimeSettings) -> Self {
        Self {
            settings,
            token: TokenChange::Keep,
            updated: Vec::new(),
            restart_required: Vec::new(),
        }
    }

    /// Lists `name` as set, and as needing a restart when its service takes it only when it starts
    /// again (`restart`).
    fn record(&mut self, name: &'static str, restart: bool) {
        self.updated.push(name);
        if restart {
            self.restart_required.push(name);
        }
    }

    /// The scheduler's copies as the request leaves them, when it set any of them
    /// ([`SCHEDULER_POLICY_KEYS`]); none when it set none, so that it sends the scheduler nothing.
    #[must_use]
    pub fn changed_policies(&self) -> Option<SchedulerPolicies> {
        self.updated
            .iter()
            .any(|key| SCHEDULER_POLICY_KEYS.contains(key))
            .then(|| self.settings.policies())
    }
}

/// Stages every key of `params` on `staged`, in the order of `config.set`: the control token
/// first, then each key in the order of the object. `host_key` stages a key of the host's own
/// services and returns none for a key the host does not have, which is refused as unknown. A
/// token cleared where the control endpoint requires one (`token_required`) is refused.
///
/// # Errors
///
/// The refusal of the first key that is unknown, startup-only or has a value its key does not
/// take; `staged` is then to be dropped.
pub fn stage_request(
    params: &Map<String, Value>,
    staged: &mut StagedSettings,
    token_required: bool,
    mut host_key: impl FnMut(&str, &Value, &mut StagedSettings) -> Option<Result<(), String>>,
) -> Result<(), String> {
    if let Some(value) = params.get("control.auth_token") {
        stage_token(value, staged, token_required)?;
    }
    for (key, value) in params {
        if key == "control.auth_token" {
            continue;
        }
        match stage_key(key, value, staged).or_else(|| host_key(key, value, staged)) {
            Some(result) => result?,
            None => return Err(format!("unknown config key '{key}'")),
        }
    }
    Ok(())
}

/// Stages `control.auth_token`: a non-empty string, or null to clear it where the endpoint does
/// not require one.
fn stage_token(
    value: &Value,
    staged: &mut StagedSettings,
    token_required: bool,
) -> Result<(), String> {
    staged.token = if value.is_null() {
        if token_required {
            return Err("auth token required for tcp endpoints".to_string());
        }
        TokenChange::Clear
    } else if let Some(token) = value.as_str() {
        let token = token.trim();
        if token.is_empty() {
            return Err(config_value_error(
                "control.auth_token",
                "must not be empty",
            ));
        }
        TokenChange::Set(SmolStr::new(token))
    } else {
        return Err(config_type_error("control.auth_token", "string or null"));
    };
    staged.updated.push("control.auth_token");
    Ok(())
}

/// The value of an arm of [`stage_key`] or [`stage_policy_key`], or the arm's refusal.
macro_rules! staged {
    ($expr:expr) => {
        match $expr {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        }
    };
}

/// Stages one key that every runtime has; none for any other key.
fn stage_key(key: &str, value: &Value, staged: &mut StagedSettings) -> Option<Result<(), String>> {
    let settings = &mut staged.settings;
    let (name, restart): (&'static str, bool) = match key {
        "log.level" => {
            let level = staged!(expect_non_empty_string(key, value));
            let Some(level) = LogLevel::parse(level) else {
                return Some(Err(config_value_error(
                    key,
                    "must be error, warn, warning, info, debug or trace",
                )));
            };
            settings.log_level = level;
            ("log.level", false)
        }
        "control.debug_enabled" => {
            settings.debug_enabled = staged!(expect_bool(key, value));
            ("control.debug_enabled", false)
        }
        "control.mode" => {
            let mode = staged!(expect_non_empty_string(key, value));
            let Some(mode) = ControlMode::parse(mode) else {
                return Some(Err(config_value_error(
                    "control.mode",
                    "expected 'production' or 'debug'",
                )));
            };
            settings.control_mode = mode;
            ("control.mode", true)
        }
        "runtime.execution_backend" | "runtime.execution_backend_source" => {
            return Some(Err(format!(
                "{key} is startup-only; change it via runtime.toml/service posture and restart"
            )));
        }
        _ => return stage_policy_key(key, value, staged),
    };
    staged.record(name, restart);
    Some(Ok(()))
}

/// Stages one key of the watchdog, the fault policy or retain; none for any other key.
fn stage_policy_key(
    key: &str,
    value: &Value,
    staged: &mut StagedSettings,
) -> Option<Result<(), String>> {
    let settings = &mut staged.settings;
    let (name, restart): (&'static str, bool) = match key {
        "watchdog.enabled" => {
            settings.watchdog.enabled = staged!(expect_bool(key, value));
            ("watchdog.enabled", false)
        }
        "watchdog.timeout_ms" => {
            settings.watchdog.timeout = staged!(expect_duration_millis(key, value));
            ("watchdog.timeout_ms", false)
        }
        "watchdog.action" => {
            settings.watchdog.action = staged!(parsed(key, value, WatchdogAction::parse));
            ("watchdog.action", false)
        }
        "watchdog.hang_action" => {
            settings.hang_action = staged!(parsed(key, value, |text| {
                HangAction::parse(text).ok_or_else(|| {
                    RuntimeError::InvalidConfig(
                        format!("invalid watchdog hang action '{text}'").into(),
                    )
                })
            }));
            ("watchdog.hang_action", true)
        }
        "fault.policy" => {
            settings.fault_policy = staged!(parsed(key, value, FaultPolicy::parse));
            ("fault.policy", false)
        }
        "retain.save_interval_ms" => {
            settings.retain_save_interval = Some(staged!(expect_duration_millis(key, value)));
            ("retain.save_interval_ms", false)
        }
        "retain.mode" => {
            settings.retain_mode = staged!(parsed(key, value, RetainMode::parse));
            ("retain.mode", true)
        }
        _ => return None,
    };
    staged.record(name, restart);
    Some(Ok(()))
}

/// The value of `key` that `parse` reads from its non-empty string, or the refusal that carries
/// the parser's error.
fn parsed<T>(
    key: &str,
    value: &Value,
    parse: impl FnOnce(&str) -> Result<T, RuntimeError>,
) -> Result<T, String> {
    let text = expect_non_empty_string(key, value)?;
    parse(text).map_err(|error| config_value_error(key, &error.to_string()))
}

#[cfg(test)]
mod tests;
