//! Purpose-specific requests become one actual-ST execution plan.
use super::{
    contracts::{Bound, Objective, Request},
    execution_settings::ExecutionSettings,
    run_contracts::RunRequest,
    snapshot::{digest, Snapshot},
};
use sha2::Digest;
use std::collections::BTreeMap;
#[derive(Clone)]
pub(super) struct Evaluation {
    pub requirements: Vec<Bound>,
    pub objective: Objective,
}
pub(super) struct ExecutionPlan {
    pub snapshot: Snapshot,
    pub simulation: String,
    pub model_revision: String,
    pub model_provenance: Option<String>,
    pub simulation_config_revision: Option<String>,
    pub settings: ExecutionSettings,
    pub index: usize,
    pub parameters: BTreeMap<String, f64>,
    pub parameter_types: BTreeMap<String, String>,
    pub parameter_schema_revision: String,
    pub evaluation: Option<Evaluation>,
    pub feedback_relay: Option<super::feedback_relay::FeedbackRelay>,
}
impl ExecutionPlan {
    pub fn search(
        snapshot: Snapshot,
        request: &Request,
        index: usize,
        parameters: BTreeMap<String, f64>,
    ) -> Result<Self, String> {
        if request.validate()?.get(index) != Some(&parameters) {
            return Err("worker candidate does not match reviewed grid".into());
        }
        if snapshot.project_revision != request.expected_project_revision {
            return Err("worker project revision mismatch".into());
        }
        Ok(Self {
            simulation: snapshot.simulation.clone(),
            model_revision: digest(&snapshot.simulation),
            model_provenance: None,
            simulation_config_revision: None,
            snapshot,
            settings: request.settings(),
            index,
            parameters,
            parameter_types: BTreeMap::new(),
            parameter_schema_revision: digest(
                request
                    .grid
                    .iter()
                    .map(|axis| (&axis.symbol, &axis.unit, axis.minimum, axis.maximum))
                    .collect::<Vec<_>>(),
            ),
            feedback_relay: None,
            evaluation: Some(Evaluation {
                requirements: request.requirements.clone(),
                objective: request.objective.clone(),
            }),
        })
    }
    pub fn run(snapshot: Snapshot, request: &RunRequest) -> Result<Self, String> {
        let parameters = request.validate()?;
        if snapshot.project_revision != request.expected_project_revision {
            return Err("worker project revision mismatch".into());
        }
        let (simulation, model_revision, model_provenance, simulation_config_revision) =
            match &request.simulation_override {
                Some(selected) => (
                    selected.simulation_toml.clone(),
                    selected.model_revision.clone(),
                    Some(selected.model_provenance.clone()),
                    Some(format!(
                        "sha256:{:x}",
                        sha2::Sha256::digest(selected.simulation_toml.as_bytes())
                    )),
                ),
                None => (
                    snapshot.simulation.clone(),
                    digest(&snapshot.simulation),
                    None,
                    None,
                ),
            };
        Ok(Self {
            simulation,
            model_revision,
            model_provenance,
            simulation_config_revision,
            snapshot,
            settings: request.settings(),
            index: 0,
            parameters,
            parameter_types: request
                .parameter_overrides
                .iter()
                .map(|p| (p.symbol.clone(), p.iec_type.clone()))
                .collect(),
            parameter_schema_revision: digest(
                request
                    .parameter_overrides
                    .iter()
                    .map(|p| (&p.symbol, &p.unit, p.minimum, p.maximum))
                    .collect::<Vec<_>>(),
            ),
            evaluation: None,
            feedback_relay: request.feedback_relay.clone(),
        })
    }
}
