//! Pre-move scaffold markers.

/// Current scaffold state for `trust-runtime-core`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreScaffoldStage {
    /// The crate exists and participates in workspace checks, but no runtime
    /// behavior has moved into it yet.
    PreMove,
}

/// Return the current runtime-core scaffold stage.
#[must_use]
pub const fn scaffold_stage() -> CoreScaffoldStage {
    CoreScaffoldStage::PreMove
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_stage_is_pre_move() {
        assert_eq!(scaffold_stage(), CoreScaffoldStage::PreMove);
    }
}
