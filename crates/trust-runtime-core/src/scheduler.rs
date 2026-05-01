//! Portable scheduler model records.

/// Resource execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourceState {
    /// Resource has been constructed but not started.
    #[default]
    Boot,
    /// Resource is waiting for a start gate or external run signal.
    Ready,
    /// Resource is actively executing cycles.
    Running,
    /// Resource is paused and will not execute cycles until resumed.
    Paused,
    /// Resource stopped because execution faulted.
    Faulted,
    /// Resource stopped cleanly.
    Stopped,
}

#[cfg(test)]
mod tests {
    use super::ResourceState;

    #[test]
    fn resource_state_preserves_default_and_lifecycle_order_contract() {
        assert_eq!(ResourceState::default(), ResourceState::Boot);
        assert_ne!(ResourceState::Ready, ResourceState::Running);
        assert_ne!(ResourceState::Paused, ResourceState::Faulted);
        assert_ne!(ResourceState::Faulted, ResourceState::Stopped);
    }
}
