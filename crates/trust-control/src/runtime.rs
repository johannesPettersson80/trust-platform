//! The runtime a study is bound to: how the extension hands over its connection, and the facts its
//! `status` operation reports (runtime protocol annex, E1.3).

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};
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
    /// The resource's state as `status` answers it: `running`, `paused` (a pause, or the debugger
    /// holding the scan), `faulted` or `stopped`; empty from a runtime that does not say.
    #[serde(default)]
    pub state: String,
    /// The debugger's hold of the scan, while it holds it (specification 13, "Pause").
    #[serde(default)]
    pub debug_hold: Option<DebugHold>,
}

/// The debugger's hold of the scan as `status` answers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DebugHold {
    /// What holds the scan: `breakpoint`, `step`, `pause` or `entry` (specification 13, "Stopped
    /// Events").
    pub reason: String,
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

    /// The refusal of `operation`, named as a sentence begins it, which needs the program to run,
    /// while the runtime is paused or the debugger holds its scan (specification 35, A2.3): it says
    /// so and how to go on, instead of the operation waiting for a scan that does not come; `None`
    /// while it runs.
    #[must_use]
    pub fn paused_refusal(&self, operation: &str) -> Option<Failure> {
        if self.state != "paused" {
            return None;
        }
        let message = match self.debug_hold.as_ref().map(|hold| hold.reason.as_str()) {
            Some(reason) => {
                let place = match reason {
                    "breakpoint" => "at a breakpoint",
                    "step" => "after a step",
                    "entry" => "at the program's start",
                    _ => "at a pause",
                };
                format!(
                    "The runtime is paused: the debugger holds its scan {place}. {operation} needs the program to run: continue in the debugger, then start it again."
                )
            }
            None => format!(
                "The runtime is paused. {operation} needs the program to run: resume the runtime, then start it again."
            ),
        };
        Some(Failure::new(&message, Cause::Runtime, NextAction::None))
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

    /// Specification 35 A2.3: an operation that needs the program to run is refused while the
    /// runtime's `status` answers `paused`, by a pause or by the debugger holding the scan, which the
    /// refusal names with how to go on; a running runtime is not refused. The sentences are worked
    /// out from A2.3's wording. Catches a paused runtime let through to a wait that times out, a
    /// debugger hold described as a pause, and a running runtime refused.
    #[test]
    fn a_paused_runtime_refuses_an_operation_that_needs_its_scan() {
        let with = |state: &str, hold: Option<&str>| -> RuntimeStatus {
            let mut report = json!({
                "state": state, "session_epoch": 7, "simulation_enabled": true,
                "simulation_only_io": true, "simulation_time_scale": 1,
                "program_revision": "sha256:program", "tasks": [], "cycle_interval_ms": 10
            });
            if let Some(reason) = hold {
                report["debug_hold"] = json!({ "reason": reason });
            }
            serde_json::from_value(report).expect("the runtime's status answer decodes")
        };
        // The whole text: its first sentence is the failure's message, the whole its detail.
        let message = |status: RuntimeStatus| {
            status
                .paused_refusal("Identify")
                .map(|failure| failure.detail.unwrap_or(failure.message))
        };
        assert_eq!(message(with("running", None)), None);
        assert_eq!(
            message(with("paused", Some("breakpoint"))).as_deref(),
            Some("The runtime is paused: the debugger holds its scan at a breakpoint. Identify needs the program to run: continue in the debugger, then start it again.")
        );
        assert_eq!(
            message(with("paused", Some("step"))).as_deref(),
            Some("The runtime is paused: the debugger holds its scan after a step. Identify needs the program to run: continue in the debugger, then start it again.")
        );
        assert_eq!(
            message(with("paused", None)).as_deref(),
            Some("The runtime is paused. Identify needs the program to run: resume the runtime, then start it again.")
        );
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
