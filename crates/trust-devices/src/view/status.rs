//! What a runtime's answer to `status` says, as far as the panel shows it (specification 11,
//! §6.8.1): the program's state, its up time, cycle times and fault count, the access role and
//! rights of the connection, whether debugging is on, the truST version and platform that newer
//! runtimes report (decision O5), and what `shutdown` does to it (decision O4). A value that is
//! missing or malformed is unknown.

use serde_json::Value;
use trust_runtime::security::AccessRole;

/// The state of a runtime's program, as `status` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProgramState {
    /// `running`.
    Running,
    /// `paused`.
    Paused,
    /// `faulted`.
    Faulted,
    /// `stopped`.
    Stopped,
    /// `ready`: waiting for its start.
    Ready,
    /// `boot`: being set up.
    Boot,
}

impl ProgramState {
    fn read(word: &str) -> Option<Self> {
        match word.trim().to_ascii_lowercase().as_str() {
            "running" => Some(Self::Running),
            "paused" => Some(Self::Paused),
            "faulted" => Some(Self::Faulted),
            "stopped" => Some(Self::Stopped),
            "ready" => Some(Self::Ready),
            "boot" => Some(Self::Boot),
            _ => None,
        }
    }
}

/// What `shutdown` does to a runtime, as the `program_control` of its `status` names it
/// (decision O4). This is a lifecycle fact that every runtime reports, not a board flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ProgramControl {
    /// `keeps_control`: the runtime still answers after `shutdown`, so `restart` starts its
    /// program again, as the truST firmware of a chip does (specification 34 STATE-08).
    KeepsControl,
    /// `process`: `shutdown` ends the process, as it ends trust-runtime on a computer. A runtime
    /// that does not report the field counts as one.
    #[default]
    Process,
}

/// The facts of one `status` answer.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct RuntimeStatus {
    /// The program's state.
    pub(crate) state: Option<ProgramState>,
    /// The time since the program started, in milliseconds.
    pub(crate) uptime_ms: Option<u64>,
    /// The last cycle time, in milliseconds.
    pub(crate) cycle_last_ms: Option<f64>,
    /// The longest cycle time, in milliseconds.
    pub(crate) cycle_max_ms: Option<f64>,
    /// The faults since the program started.
    pub(crate) faults: Option<u64>,
    /// The role of the connection that asked.
    pub(crate) role: Option<AccessRole>,
    /// Whether that role may write values.
    pub(crate) may_write: bool,
    /// Whether that role may force values.
    pub(crate) may_force: bool,
    /// Whether the debug surface is on.
    pub(crate) debug_enabled: Option<bool>,
    /// The truST version.
    pub(crate) version: Option<String>,
    /// The platform, such as `linux-x86_64` or `stm32f407`.
    pub(crate) platform: Option<String>,
    /// What `shutdown` does to the runtime.
    pub(crate) program_control: ProgramControl,
}

impl RuntimeStatus {
    /// Reads a `status` result.
    pub(crate) fn read(value: &Value) -> Self {
        let text = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
        };
        let metrics = value.get("metrics");
        let cycle = metrics.and_then(|metrics| metrics.get("cycle_ms"));
        let milliseconds = |key: &str| {
            cycle
                .and_then(|cycle| cycle.get(key))
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && *value >= 0.0)
        };
        let access = value.get("access");
        let right = |key: &str| {
            access
                .and_then(|access| access.get("io"))
                .and_then(|io| io.get(key))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        Self {
            state: value
                .get("state")
                .and_then(Value::as_str)
                .and_then(ProgramState::read),
            uptime_ms: value.get("uptime_ms").and_then(Value::as_u64),
            cycle_last_ms: milliseconds("last"),
            cycle_max_ms: milliseconds("max"),
            faults: metrics
                .and_then(|metrics| metrics.get("faults"))
                .and_then(Value::as_u64),
            role: access
                .and_then(|access| access.get("role"))
                .and_then(Value::as_str)
                .and_then(AccessRole::parse),
            may_write: right("write"),
            may_force: right("force"),
            debug_enabled: value.get("debug_enabled").and_then(Value::as_bool),
            version: text("version"),
            platform: text("platform"),
            program_control: if text("program_control").as_deref() == Some("keeps_control") {
                ProgramControl::KeepsControl
            } else {
                ProgramControl::Process
            },
        }
    }
}
