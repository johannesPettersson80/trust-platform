//! Control Studio's state-feedback laws: state feedback by pole placement, LQR and LQG (specification 35, C4 and B4;
//! numerical annex N5.13, N5.14 and N7.9). The numbers come from `trust_control_math`; this module holds the rules: which
//! coefficients a declared law maps, how the designed gains become the declared parameters, the stored design
//! preferences and the sentences the engineer reads. Nothing here reads a clock or does I/O. A design is a provisional,
//! unapplied candidate, and nothing writes it to a runtime.

pub mod binding;
pub mod coefficients;
pub mod design;
pub mod families;
pub mod poles;
pub mod preferences;
pub mod reference;
pub mod sampling;
pub mod texts;

#[cfg(test)]
pub(crate) mod fixtures;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::linalg::{DenseMatrix, NumericalTolerance};
use trust_control_math::lqr::HorizonLimits;

pub use binding::{decode_state_controller_binding, StateControllerBinding};
pub use design::{design_state_controller, StateControllerDesign};
pub use poles::state_controller_poles;
pub use preferences::StateDesignPreferences;

/// The family of a state-controller law (C4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum StateControllerFamily {
    /// State feedback whose poles the designer places.
    #[serde(rename = "state-feedback")]
    StateFeedback,
    /// The linear-quadratic regulator.
    #[serde(rename = "LQR")]
    Lqr,
    /// The linear-quadratic-Gaussian controller: an LQR law on the estimates of a Kalman estimator.
    #[serde(rename = "LQG")]
    Lqg,
}

/// The most states a declared law or a design holds (C4, decoded records; the Rust engine's state limit, 32).
pub const MAXIMUM_STATES: usize = 32;
/// The most inputs a declared law or a design holds (C4, decoded records).
pub const MAXIMUM_INPUTS: usize = 16;
/// The most control steps of a finite-horizon design (C4, N5.14; the preference rule of `stateDesignPreferences.ts`).
pub const MAXIMUM_HORIZON_STEPS: usize = 512;

/// The tolerance of the design rules (`stateControllerDesign.ts`): absolute `1e-10`, relative `1e-9`, condition limit
/// `1e10`.
pub const DESIGN_TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-10,
    relative: 1e-9,
    condition_limit: 1e10,
};

/// The product limits of a finite-horizon design, as the sampled designs take them.
pub const HORIZON_LIMITS: HorizonLimits = HorizonLimits {
    maximum_steps: MAXIMUM_HORIZON_STEPS,
    maximum_states: MAXIMUM_STATES,
    maximum_inputs: MAXIMUM_INPUTS,
};

/// The tolerance with which a weight or covariance must be symmetric (`stateDesignPreferences.ts`, `positiveWeight`).
pub const SYMMETRY_TOLERANCE: f64 = 1e-10;

/// Whether `matrix` is `size` rows of `size` entries, symmetric to [`SYMMETRY_TOLERANCE`]. The shape is checked for every
/// row before any entry is compared, so no entry is read outside the matrix.
#[must_use]
pub(crate) fn is_symmetric_square(matrix: &DenseMatrix, size: usize) -> bool {
    matrix.len() == size
        && matrix.iter().all(|row| row.len() == size)
        && matrix.iter().enumerate().all(|(row, values)| {
            values
                .iter()
                .enumerate()
                .all(|(column, value)| (value - matrix[column][row]).abs() <= SYMMETRY_TOLERANCE)
        })
}
