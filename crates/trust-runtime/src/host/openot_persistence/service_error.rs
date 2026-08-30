use super::PersistenceError;

pub(super) fn redacted_error(error: &PersistenceError) -> String {
    match error {
        PersistenceError::NotImplemented => "persistence behavior is unavailable".to_string(),
        PersistenceError::IdentityConflict(identity) => {
            format!("OpenOT document identity conflict at {identity}")
        }
        PersistenceError::CheckpointRegression { current, requested } => {
            format!("OpenOT checkpoint regression: current={current}, requested={requested}")
        }
        PersistenceError::InvalidConfig(_) => {
            "OpenOT persistence configuration is invalid; inspect the local runtime log".to_string()
        }
        PersistenceError::BackendUnavailable(message) => message.clone(),
        PersistenceError::Commit(_) => {
            "selected OpenOT persistence backend operation failed; inspect the local runtime log"
                .to_string()
        }
        PersistenceError::CapacityExhausted(_) => {
            "OpenOT persistence durable capacity is exhausted; free space or increase the configured bound"
                .to_string()
        }
    }
}
