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
        PersistenceError::Commit(message) => safe_commit_context(message).map_or_else(
            || "selected OpenOT persistence backend operation failed".to_string(),
            |context| format!("selected OpenOT persistence backend operation failed: {context}"),
        ),
        PersistenceError::CapacityExhausted(_) => {
            "OpenOT persistence durable capacity is exhausted; free space or increase the configured bound"
                .to_string()
        }
    }
}

fn safe_commit_context(message: &str) -> Option<&str> {
    let context = message
        .split_once(':')
        .map_or(message, |(context, _)| context);
    let known_backend = [
        "SQLite",
        "PostgreSQL",
        "TimescaleDB",
        "MySQL",
        "SQL Server",
        "InfluxDB 3",
        "OpenOT",
    ]
    .iter()
    .any(|backend| context.starts_with(backend));
    (known_backend
        && !context.is_empty()
        && context.len() <= 120
        && context
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || " -_".contains(character)))
    .then_some(context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_error_keeps_safe_action_but_removes_sensitive_detail() {
        let projected = redacted_error(&PersistenceError::Commit(
            "PostgreSQL connect with required TLS: password=plant-secret /private/ca.pem".into(),
        ));

        assert!(projected.contains("PostgreSQL connect with required TLS"));
        assert!(!projected.contains("plant-secret"));
        assert!(!projected.contains("/private"));
    }
}
