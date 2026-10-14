//! The sentences of the loop contract that the engine answers (specification 35, C12).

/// The reason a stage gives for a loop without a controller law (C12): Tune, Validate, Apply, Restore and Keep refuse with it, and
/// the stage status answers Not applicable with it (`stages::stage_availability`).
pub const NO_CONTROLLER_LAW: &str = "This loop has no controller law, so there is nothing to tune.";
