use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::Value;

const COMPLETED_OPERATION_LIMIT: usize = 256;

#[derive(Debug, Clone)]
struct CompletedOperation {
    project_id: String,
    operation_id: String,
    input_fingerprint: String,
    result: Value,
}

#[derive(Debug)]
pub(crate) enum PlantOperationOutcome {
    Applied(Value),
    Replayed(Value),
    Conflict,
}

/// Coordinates revision-guarded plant mutations and bounded retry replay.
#[derive(Debug, Default)]
pub struct PlantControlService {
    completed: Mutex<VecDeque<CompletedOperation>>,
}

impl PlantControlService {
    pub(crate) fn execute_once<E>(
        &self,
        project_id: &str,
        operation_id: &str,
        input_fingerprint: &str,
        apply: impl FnOnce() -> Result<Value, E>,
    ) -> Result<PlantOperationOutcome, E> {
        // Keep the lock through the write so two concurrent saves cannot both
        // pass the same revision check and overwrite each other.
        let mut completed = self
            .completed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = completed
            .iter()
            .find(|entry| entry.project_id == project_id && entry.operation_id == operation_id)
        {
            return Ok(if existing.input_fingerprint == input_fingerprint {
                PlantOperationOutcome::Replayed(existing.result.clone())
            } else {
                PlantOperationOutcome::Conflict
            });
        }

        let result = apply()?;
        if completed.len() == COMPLETED_OPERATION_LIMIT {
            completed.pop_front();
        }
        completed.push_back(CompletedOperation {
            project_id: project_id.to_string(),
            operation_id: operation_id.to_string(),
            input_fingerprint: input_fingerprint.to_string(),
            result: result.clone(),
        });
        Ok(PlantOperationOutcome::Applied(result))
    }
}
