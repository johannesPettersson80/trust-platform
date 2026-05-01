//! Portable retain and restart policy records.

/// Retentive behavior for variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RetainPolicy {
    /// Retentive across warm restarts.
    Retain,
    /// Always reinitialized on restart.
    NonRetain,
    /// No explicit qualifier; treat as non-retentive on warm restart.
    #[default]
    Unspecified,
    /// Persistent across warm restarts.
    Persistent,
}

/// Restart mode for a resource/configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartMode {
    /// Cold restart: reinitialize all variables.
    Cold,
    /// Warm restart: retain RETAIN/PERSISTENT variables.
    Warm,
}

#[cfg(test)]
mod tests {
    use super::{RestartMode, RetainPolicy};

    #[test]
    fn retain_policy_preserves_default_and_warm_restart_contract() {
        assert_eq!(RetainPolicy::default(), RetainPolicy::Unspecified);
        assert_ne!(RetainPolicy::Retain, RetainPolicy::NonRetain);
        assert_ne!(RetainPolicy::Persistent, RetainPolicy::Unspecified);
        assert_ne!(RestartMode::Cold, RestartMode::Warm);
    }
}
