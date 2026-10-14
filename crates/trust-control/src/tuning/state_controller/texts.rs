//! The sentences of the state-feedback laws (specification 35, A11 and C4; numerical annex N5.14), and the words of the
//! refusals the design rules name. Each sentence names the values involved (A11.1). A rule decides; these functions say
//! what it decided.

use trust_control_math::linalg::NumericalRejectionReason;
use trust_control_math::lqg::{KalmanRefusal, LqgRefusal};
use trust_control_math::lqr::LqrRefusal;
use trust_control_math::state_feedback::PlacementRefusal;

use super::preferences::DraftField;

/// C4: the estimator of an LQG law cannot be changed into another family without its own adapter.
pub const ESTIMATOR_STRUCTURE_CHANGE: &str =
    "Changing estimator structure requires its own typed ST adapter.";
/// C4 and B4.7: a design needs an explicit coefficient binding, and a model without a transfer function or a delay.
pub const NEEDS_BINDING: &str = "State-controller design requires an explicit coefficient binding and a delay-free state-space model.";
/// C4 and N5.14: each desired pole is a strict unit-circle value, one per model state.
pub const STRICT_UNIT_CIRCLE_POLES: &str =
    "Choose one strict unit-circle pole for every model state.";
/// C4: independent reference tracking needs one controlled objective per input.
pub const ONE_OBJECTIVE_PER_INPUT: &str = "Independent reference tracking requires one controlled objective per available input; auxiliary measurements are retained separately.";
/// N5.14: the reference gain's equilibrium cannot be solved for the selected objectives.
pub const INSUFFICIENT_AUTHORITY: &str =
    "The selected output objectives have insufficient steady control authority.";
/// C0: the mapped coefficient layout does not match the model's coordinates.
pub const LAYOUT_MISMATCH: &str = "ST coefficient layout does not match model coordinates.";
/// C4: the state-controller coordinates (the declared law on a state-space model) are not available.
pub const COORDINATES_UNAVAILABLE: &str = "State-controller coordinates are unavailable.";
/// C4: the state-feedback gain's coordinates are not the model's.
pub const GAIN_COORDINATES: &str = "State-feedback gain coordinates do not match the model.";
/// C4: the closed-loop poles of the mapped coefficients cannot be computed.
pub const POLE_CALCULATION_FAILED: &str = "State-controller pole calculation failed.";
/// C4: the observer's gain coordinates are not the model's.
pub const OBSERVER_COORDINATES: &str = "Observer gain coordinates do not match the model.";
/// C4: the observer's poles cannot be computed.
pub const OBSERVER_POLE_FAILED: &str = "Observer pole calculation failed.";
/// C4 and N7.11: the mapped coefficients must give strict sampled stability.
pub const NOT_STRICTLY_STABLE: &str =
    "Mapped controller/observer coefficients do not establish strict sampled stability.";
/// N4.7: a discrete model is used only at the task period.
pub const SAMPLE_PERIODS_DIFFER: &str = "State controller and model sample periods differ.";
/// C4: the design weights and covariances are finite symmetric matrices in declared coordinates.
pub const WEIGHTS_NOT_MATRICES: &str =
    "Design weights/covariances must be finite symmetric matrices in declared coordinates.";
/// C4 and N5.14: the design weights and covariances are positive (semi)definite as their role requires.
pub const WEIGHTS_NOT_DEFINITE: &str =
    "Design weights/covariances must satisfy the required positive-(semi)definite condition.";
/// C4: the estimator's fixed model is the selected sampled model.
pub const ESTIMATOR_MODEL_DIFFERS: &str = "The selected model differs from the declared ST estimator model. Update its coefficient binding or select the matching model before LQG design.";
/// C4: the LQG adapter needs a measurement feedthrough correction when the model has one.
pub const LQG_FEEDTHROUGH: &str = "The LQG adapter needs a declared measurement feedthrough correction for a model with nonzero D.";
/// C4: an LQG design needs justified covariances.
pub const COVARIANCE_MISSING: &str =
    "Supply justified process and measurement covariance before LQG design.";
/// C4: the scope of a provisional state-controller design.
pub const DESIGN_SCOPE: &str = "Mapped sampled state-feedback coefficients in declared coordinates; actual-ST verification and a separately declared initialization contract govern application.";

/// N4.7(a): the exact discretization of the model refused, with the reason.
#[must_use]
pub fn exact_discretization(reason: &str) -> String {
    format!("Exact model discretization: {reason}.")
}

/// C4: the coefficient's unit differs from the unit of its model coordinates.
#[must_use]
pub fn unit_mismatch(symbol: &str) -> String {
    format!("Coefficient {symbol} unit does not match its model coordinates.")
}

/// C0: a calculated coefficient is outside the representable bounds of its parameter.
#[must_use]
pub fn outside_representable_bounds(symbol: &str) -> String {
    format!("Calculated coefficient {symbol} is outside its representable bounds.")
}

/// C4: pole placement refused, with the reason.
#[must_use]
pub fn state_feedback_refused(reason: &str) -> String {
    format!("State-feedback design: {reason}.")
}

/// C4: the LQR refused, with the reason.
#[must_use]
pub fn lqr_refused(reason: &str) -> String {
    format!("LQR design: {reason}.")
}

/// C4: the LQG design refused, with the reason.
#[must_use]
pub fn lqg_refused(reason: &str) -> String {
    format!("LQG design: {reason}.")
}

/// The reason word of a numerical refusal, as the design rules name it.
#[must_use]
pub fn numerical_reason(reason: NumericalRejectionReason) -> &'static str {
    match reason {
        NumericalRejectionReason::NonfiniteInput => "nonfinite-input",
        NumericalRejectionReason::InconsistentDimensions => "inconsistent-dimensions",
        NumericalRejectionReason::MatrixNotSquare => "matrix-not-square",
        NumericalRejectionReason::Singular => "singular",
        NumericalRejectionReason::IllConditioned => "ill-conditioned",
        NumericalRejectionReason::SolverFailed => "solver-failed",
    }
}

/// The reason word of a refused pole placement.
#[must_use]
pub fn placement_reason(refusal: PlacementRefusal) -> &'static str {
    match refusal {
        PlacementRefusal::InconsistentDimensions => "inconsistent-dimensions",
        PlacementRefusal::Uncontrollable => "uncontrollable",
        PlacementRefusal::ClosedLoopEigenvalues(reason) => numerical_reason(reason),
    }
}

/// The reason word of a refused LQR.
#[must_use]
pub fn lqr_reason(refusal: LqrRefusal) -> &'static str {
    match refusal {
        LqrRefusal::Numerical(reason) => numerical_reason(reason),
        LqrRefusal::InvalidFiniteHorizon => "invalid-finite-horizon-design",
        LqrRefusal::SingularControlWeight => "singular-control-weight",
        LqrRefusal::RiccatiNotConverged => "riccati-not-converged",
        LqrRefusal::NonstabilisingSolution => "nonstabilising-solution",
        LqrRefusal::NonfiniteRiccatiStage => "nonfinite-riccati-stage",
    }
}

/// The reason word of a refused Kalman estimator.
#[must_use]
pub fn kalman_reason(refusal: KalmanRefusal) -> &'static str {
    match refusal {
        KalmanRefusal::Numerical(reason) => numerical_reason(reason),
        KalmanRefusal::SingularInnovation => "singular-innovation",
        KalmanRefusal::RiccatiNotConverged => "riccati-not-converged",
    }
}

/// The reason word of a refused LQG design.
#[must_use]
pub fn lqg_reason(refusal: LqgRefusal) -> &'static str {
    match refusal {
        LqgRefusal::Lqr(refusal) => lqr_reason(refusal),
        LqgRefusal::Kalman(refusal) => kalman_reason(refusal),
        LqgRefusal::SolverFailed => "solver-failed",
    }
}

/// The label of a draft field, as the form names it (`STATE_DESIGN_LABELS`).
#[must_use]
pub fn draft_label(field: DraftField) -> &'static str {
    match field {
        DraftField::DesiredPoles => "Desired poles",
        DraftField::StateWeights => "State weights (Q)",
        DraftField::InputWeights => "Input weights (R)",
        DraftField::ProcessNoise => "Process noise covariance",
        DraftField::MeasurementNoise => "Measurement noise covariance",
        DraftField::SolverMode => "Solver",
        DraftField::HorizonSteps => "Horizon steps",
        DraftField::TerminalWeights => "Terminal weights",
    }
}

/// A draft field whose text is not JSON.
#[must_use]
pub fn not_valid_json(label: &str) -> String {
    format!("{label} must be valid JSON.")
}

/// The horizon steps of a draft that are not an integer from 1 to 512.
pub const HORIZON_STEPS_RANGE: &str = "Horizon steps must be an integer from 1 to 512.";
/// A finite horizon whose terminal weights are blank.
pub const TERMINAL_WEIGHTS_MISSING: &str = "Enter the terminal weights matrix.";
/// A solver choice that is neither finite nor infinite.
pub const SOLVER_MODE_CHOICE: &str = "Choose finite or infinite horizon.";
/// A draft whose decoded preferences are refused by the structural rules.
pub const PREFERENCES_INVALID: &str =
    "Use finite symmetric weight matrices and valid desired poles.";
