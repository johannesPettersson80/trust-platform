//! The sentences of the sensor-lag comparison (specification 35, B3.9 and A11.1; numerical annex N2.4 and N3.2): the
//! warnings a fit attaches to its result, the refusals of the fit and of the structure choice. The rules decide; these
//! constants say what was decided, in plain words, and name no domain (the plant may be any process).

/// The warning of every sensor-lag fit: the recording cannot tell the two lags apart by process and sensor.
pub const HYPOTHESIS_WARNING: &str = "Additional lag is a hypothesis; the recording alone cannot assign the two time constants to the process and the sensor.";
/// The warning when the best dead time, first time constant or second time constant is on its grid edge.
pub const SEARCH_BOUNDARY_WARNING: &str =
    "The sensor-lag fit reaches a search boundary; inspect or extend the justified range.";
/// The warning when the best baseline or gain is on its grid edge.
pub const GRID_BOUNDARY_WARNING: &str = "The fitted baseline or gain reaches its grid boundary; review window uncertainty and grid resolution.";
/// The warning when no grid point is measurement consistent.
pub const NO_CONSISTENT_POINT_WARNING: &str = "No sensor-lag grid point satisfies the declared measurement bound; model adequacy does not replace measurement consistency.";
/// The warning when the timestamp shift is not zero.
pub const TIMESTAMP_SHIFT_WARNING: &str =
    "The timestamp shift is a sensitivity study, not nominal model evidence.";

/// The refusal when the search would evaluate more shapes, samples or baselines than its limits allow.
pub const SEARCH_TOO_LARGE: &str = "Sensor-lag search is too large; narrow the justified bounds or increase the declared grid spacing.";
/// The refusal when a lattice of the search has too many points.
pub const GRID_TOO_LARGE: &str =
    "Step-response identification grid is too large; refine the requested bounds or spacing.";
/// The refusal when no baseline and gain lie within the qualified windows.
pub const NO_CANDIDATE: &str =
    "The baseline/gain grid has no candidate within the qualified windows; refine its spacing.";

/// The refusal when the first-order reference was fitted with other windows or measurement assumptions.
pub const REFERENCE_CHANGED: &str = "Recheck the first-order reference before comparing with changed windows or measurement assumptions.";
/// The refusal when the comparison has no estimation preview to start from.
pub const NEEDS_PREVIEW: &str = "A structure comparison needs an estimation preview.";
/// The refusal when a sensor lag is chosen although the first-order model is adequate.
pub const FIRST_ORDER_ADEQUATE: &str =
    "The first-order model is adequate; the extra state is not supported.";
/// The refusal when the chosen structure does not meet the declared requirements.
pub const STRUCTURE_NOT_MET: &str =
    "That structure does not meet the declared model-error and improvement requirements.";
/// The refusal when a structure is chosen before the comparison has been calculated.
pub const CALCULATE_FIRST: &str = "Calculate a model-structure comparison before choosing a model.";
/// The refusal when an independent check is asked for before the engineer has chosen a structure (N2.10: the check
/// follows the choice).
pub const CHOOSE_BEFORE_VALIDATION: &str = "Choose a model structure before the independent check.";
