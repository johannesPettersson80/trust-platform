//! The runtime a study is bound to: how the extension hands over its connection, and the facts its
//! `status` operation reports (runtime protocol annex, E1.3).

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::recording::EvidenceSource;

/// Where the bound runtime listens and its control token, as the extension's runtime lifecycle
/// knows them. The token is a credential: it is never journaled (E11.2), printed or stored.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeConnection {
    /// `tcp://host:port` or `unix:///absolute/path`.
    pub endpoint: String,
    /// The control token, when the runtime requires one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// The runtime this endpoint belongs to, as the extension's selection names it (A2.7). A live request whose loop is bound to
    /// another runtime is refused before it starts. The command line names no selection and leaves it empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_id: Option<String>,
}

impl fmt::Debug for RuntimeConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeConnection")
            .field("endpoint", &self.endpoint)
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .field("runtime_id", &self.runtime_id)
            .finish()
    }
}

/// The facts of E1.3 that Control Studio uses; the answer has further fields it does not read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeStatus {
    /// The session epoch (E1.1).
    pub session_epoch: u64,
    /// Whether simulation is enabled.
    pub simulation_enabled: bool,
    /// Whether every I/O driver is simulated and no ADS or OPC UA client is configured.
    pub simulation_only_io: bool,
    /// The configured speed, 1 to 100.
    pub simulation_time_scale: u32,
    /// The achieved speed; null until measured.
    #[serde(default)]
    pub simulation_achieved_time_scale: Option<f64>,
    /// The speed words the runtime computes (specification 35, B9.2): "" at 1x, "×N" above 1x, and "×A of ×N" when the achieved
    /// speed is behind. An answer without the field names a simulation without a speed.
    #[serde(default)]
    pub simulation_speed_text: String,
    /// The loaded program's revision.
    pub program_revision: String,
    /// The scheduled tasks.
    pub tasks: Vec<TaskFact>,
    /// The resource cycle interval.
    pub cycle_interval_ms: u64,
}

/// One scheduled task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskFact {
    /// The task's name.
    pub name: String,
    /// Its interval; 0 for an event task.
    pub interval_ms: u64,
}

impl RuntimeStatus {
    /// Where values observed on this runtime come from (A2.4, A9): a simulated plant only when
    /// simulation is enabled and every attached I/O is simulated.
    #[must_use]
    pub fn source(&self) -> EvidenceSource {
        if self.simulation_enabled && self.simulation_only_io {
            EvidenceSource::SimulatedPlant
        } else {
            EvidenceSource::PhysicalPlant
        }
    }

    /// The task named `name`, matched without regard to case (annex E4.2).
    #[must_use]
    pub fn task(&self, name: &str) -> Option<&TaskFact> {
        self.tasks
            .iter()
            .find(|task| task.name.eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn status(simulation_enabled: bool, simulation_only_io: bool) -> RuntimeStatus {
        serde_json::from_value(json!({
            "state": "running", "session_epoch": 7, "simulation_enabled": simulation_enabled,
            "simulation_only_io": simulation_only_io, "simulation_time_scale": 1, "simulation_achieved_time_scale": null,
            "program_revision": "sha256:program", "tasks": [{ "name": "MainTask", "interval_ms": 100 }], "cycle_interval_ms": 10
        }))
        .expect("the runtime's status answer decodes")
    }

    #[test]
    fn a_target_is_simulated_only_when_simulation_is_enabled_and_all_io_is_simulated() {
        assert_eq!(status(true, true).source(), EvidenceSource::SimulatedPlant);
        assert_eq!(
            status(true, false).source(),
            EvidenceSource::PhysicalPlant,
            "an ADS or OPC UA client is not simulated I/O"
        );
        assert_eq!(status(false, true).source(), EvidenceSource::PhysicalPlant);
    }

    #[test]
    fn a_task_is_found_without_regard_to_case() {
        assert_eq!(
            status(true, true)
                .task("maintask")
                .map(|task| task.interval_ms),
            Some(100)
        );
        assert!(status(true, true).task("FastTask").is_none());
    }

    #[test]
    fn the_token_never_appears_when_a_connection_is_printed() {
        let connection = RuntimeConnection {
            endpoint: "tcp://127.0.0.1:9000".into(),
            token: Some("secret-token".into()),
            runtime_id: None,
        };
        let printed = format!("{connection:?}");
        assert!(
            printed.contains("127.0.0.1:9000") && !printed.contains("secret-token"),
            "{printed}"
        );
    }
}
