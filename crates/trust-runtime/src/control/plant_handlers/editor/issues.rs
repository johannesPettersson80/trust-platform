//! A validation issue of the plant editor (specification 47 §13.1, §20.1): a stable code, the exact
//! field path, the message the editor shows at the field, the summary line it lists, and where in
//! the editor the field is, so that the editor can open its section and focus it.

use serde_json::{json, Value};

/// Where the editor shows an issue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Target {
    /// The plant ID or display name (`"id"`, `"displayName"`).
    Identity(&'static str),
    /// A command (`"command"`) or feedback (`"feedback"`) port, by its index.
    Port(&'static str, usize),
    /// A dynamics field by its key (`gain`, `initial_state`, `time_constant_s`).
    Dynamics(String),
    /// One value of the starting state, by its index.
    StartingState(usize),
    /// A state-space input delay, or the delay vector as a whole.
    InputDelay(Option<usize>),
    /// A state-space matrix cell (`a`, `b`, `c`, `d`, `initial_state`).
    Matrix(&'static str, usize, usize),
    /// A piecewise-linear point's input or output.
    Piecewise(usize, &'static str),
    /// An actuator field.
    Actuator(String),
    /// A signal-conversion field (`step_ms`, `initial_input`, ...).
    Signal(&'static str),
    /// An imperfection effect's field, by the effect's row.
    Effect(usize, String),
    /// A scenario event's field, by the event's row.
    Event(usize, &'static str),
    /// The confirmation of the temperature example's units.
    TemperatureUnits,
    /// The complete plant configuration, as the runtime's validation judges it.
    Configuration,
}

/// One validation issue of the draft.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Issue {
    /// The stable code (specification 47 §20.1).
    pub(super) code: &'static str,
    /// The exact field path in the plant candidate (`parameters.gain`, `uncertainties[2].mode`).
    pub(super) path: String,
    /// The message the editor shows at the field.
    pub(super) message: String,
    /// The line the editor's validation summary lists: the field, its value and the message.
    pub(super) summary: String,
    pub(super) target: Target,
}

impl Issue {
    pub(super) fn new(
        code: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
        target: Target,
    ) -> Self {
        let message = message.into();
        Self {
            code,
            path: path.into(),
            summary: message.clone(),
            message,
            target,
        }
    }

    /// The same issue with its summary line.
    pub(super) fn summarized(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    /// The issue as `plant.edit` answers it, for the plant `plant_id`.
    pub(super) fn to_json(&self, plant_id: &str) -> Value {
        let target = match &self.target {
            Target::Identity(field) => json!({"kind": "identity", "field": field}),
            Target::Port(direction, index) => {
                json!({"kind": "port", "direction": direction, "index": index})
            }
            Target::Dynamics(key) => json!({"kind": "dynamics", "key": key}),
            Target::StartingState(index) => {
                json!({"kind": "dynamics", "key": "initial_state", "index": index})
            }
            Target::InputDelay(column) => json!({"kind": "input_delay", "column": column}),
            Target::Matrix(matrix, row, column) => {
                json!({"kind": "matrix", "matrix": matrix, "row": row, "column": column})
            }
            Target::Piecewise(row, field) => {
                json!({"kind": "piecewise", "row": row, "field": field})
            }
            Target::Actuator(key) => json!({"kind": "actuator", "key": key}),
            Target::Signal(key) => json!({"kind": "signal", "key": key}),
            Target::Effect(row, field) => json!({"kind": "effect", "row": row, "field": field}),
            Target::Event(row, field) => json!({"kind": "event", "row": row, "field": field}),
            Target::TemperatureUnits => json!({"kind": "temperature_units"}),
            Target::Configuration => json!({"kind": "configuration"}),
        };
        let mut issue = json!({
            "code": self.code,
            "severity": "error",
            "path": self.path,
            "message": self.message,
            "summary": self.summary,
            "target": target,
        });
        if !plant_id.is_empty() {
            issue["plant_id"] = json!(plant_id);
        }
        issue
    }
}
