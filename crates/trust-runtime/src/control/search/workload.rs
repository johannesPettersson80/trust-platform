use super::{
    contracts::Request,
    run_contracts::RunRequest,
    snapshot::Snapshot,
    worker::{RunInput, TrialInput, WorkerInput},
};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Purpose {
    Search,
    Run,
}
#[derive(Clone)]
pub(super) enum Workload {
    Search(Request),
    Run(RunRequest),
}
impl Workload {
    pub fn client_id(&self) -> Option<&str> {
        match self {
            Self::Run(request) => request.request_id.as_deref(),
            Self::Search(_) => None,
        }
    }
    pub fn fingerprint(&self) -> String {
        match self {
            Self::Run(request) => super::snapshot::digest(request),
            Self::Search(request) => super::snapshot::digest(request),
        }
    }
    pub fn purpose(&self) -> Purpose {
        match self {
            Self::Search(_) => Purpose::Search,
            Self::Run(_) => Purpose::Run,
        }
    }
    pub fn candidates(&self) -> Result<Vec<BTreeMap<String, f64>>, String> {
        match self {
            Self::Search(request) => request.validate(),
            Self::Run(request) => Ok(vec![request.validate()?]),
        }
    }
    pub fn expected_revision(&self) -> &str {
        match self {
            Self::Search(request) => &request.expected_project_revision,
            Self::Run(request) => &request.expected_project_revision,
        }
    }
    pub fn wall_budget_ms(&self) -> u64 {
        match self {
            Self::Search(request) => request.wall_budget_ms,
            Self::Run(request) => request.wall_budget_ms,
        }
    }
    pub fn per_trial_wall_ms(&self) -> u64 {
        match self {
            Self::Search(request) => request.per_trial_wall_ms,
            Self::Run(request) => request.wall_budget_ms,
        }
    }
    pub fn input(
        &self,
        snapshot: Snapshot,
        index: usize,
        parameters: BTreeMap<String, f64>,
    ) -> WorkerInput {
        match self {
            Self::Search(request) => WorkerInput::Search(TrialInput {
                snapshot,
                request: request.clone(),
                index,
                parameters,
            }),
            Self::Run(run) => WorkerInput::Run(RunInput {
                snapshot,
                run: run.clone(),
            }),
        }
    }
}
