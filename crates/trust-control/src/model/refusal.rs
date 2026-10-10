//! The sentences of Control Studio's model rules. Each variant's text is the sentence that the
//! `TypeScript` throws or reports; the mapping functions choose it from the library's typed refusal.

use trust_control_math::linalg::NumericalRejectionReason;
use trust_control_math::model::{ConstructionError, ConversionError, StructureError};
use trust_control_math::simulate::SimulationError;

/// A refused model operation, with the sentence that Control Studio shows for it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelRefusal {
    /// The linear-model conversion received a model that is not valid.
    #[error("Select a valid linear model before controller analysis.")]
    SelectValidLinearModel,
    /// A sampled model is not a positive one-state model without direct feedthrough.
    #[error("Classical tuning supports a positive one-state sampled model without direct feedthrough; use a compatible sampled/state controller for this model.")]
    ClassicalTuningSampled,
    /// The held-input equivalent of a sampled model is not finite.
    #[error("The sampled model has no finite supported held-input equivalent.")]
    NoFiniteEquivalent,
    /// A transfer conversion needs one input and one output.
    #[error("Transfer analysis requires a SISO model.")]
    TransferAnalysisRequiresSiso,
    /// The state-space order is above the conversion limit.
    #[error("State-space conversion exceeds the supported transfer-polynomial order.")]
    StateSpaceOrderLimit,
    /// The state-space conversion is numerically uncertain.
    #[error("State-space transfer conversion has numerical uncertainty.")]
    StateSpaceUncertainty,
    /// The structural analysis of the model did not qualify.
    #[error("State-space structural analysis did not qualify.")]
    StructureNotQualified,
    /// A structural matrix or rank is not finite or consistent.
    #[error("Structural matrices or ranks are nonfinite or inconsistent.")]
    StructureInconsistent,
    /// The response needs a valid model.
    #[error("Select a valid model before calculating its response.")]
    SelectValidModel,
    /// The response settings do not match the model.
    #[error("Choose initial coordinates and inputs matching the model, its sample period, and at most 4096 output samples.")]
    ChooseResponseSettings,
    /// A feedback response needs a controller, which this stage does not have.
    #[error("Select a compatible scalar feedback controller for this response.")]
    FeedbackControllerMissing,
    /// The controller and the plant have different sample periods.
    #[error("Controller and plant sample periods must match.")]
    SamplePeriodsDiffer,
    /// A response value is not finite.
    #[error("The response exceeded finite model coordinates.")]
    ResponseNotFinite,
    /// The delayed realization's initial coordinates do not match the model.
    #[error("Delayed model initial coordinates must match its states and inputs.")]
    DelayedInitialCoordinates,
    /// The delay or the sample period is not valid.
    #[error("Invalid model delay or sample period.")]
    InvalidDelayOrStep,
    /// A discrete model's delay is not a whole number of its sample periods.
    #[error("A discrete model delay must be an integer number of its sample periods.")]
    DiscreteDelayNotWhole,
    /// The exact delay history needs more states than the budget allows.
    #[error("The exact delay history exceeds the supported 512-state realization budget.")]
    DelayBudget,
    /// The exact conversion at the sample period failed for the given reason word.
    #[error("Exact held-input conversion failed: {reason}.")]
    ConversionFailed {
        /// The reason word of the failed conversion.
        reason: &'static str,
    },
    /// The exact delayed realization has a value that is not finite.
    #[error("Nonfinite exact-delay realization.")]
    NonfiniteRealization,
    /// The recording's sample period differs from the model's.
    #[error("Model and recording sample periods differ.")]
    RecordingPeriodsDiffer,
    /// The recording is too short to separate the two parts of the prediction.
    #[error(
        "The recording is too short to separate initial-state estimation from model validation."
    )]
    RecordingTooShort,
    /// The initial state cannot be estimated from the recording.
    #[error("The model's initial state is not observable in the recording.")]
    NotObservable,
    /// A predicted value is not finite.
    #[error("Frozen model prediction exceeded finite values.")]
    FrozenNotFinite,
    /// The recording has no samples to predict from. (A product rule: the `TypeScript` threw a type error.)
    #[error("The recording has no samples.")]
    EmptyRecording,
    /// The recording and its operating point do not match the model's inputs and outputs.
    /// (A product rule: the `TypeScript` threw a type error.)
    #[error(
        "The recording and its operating point must have one value per model input and output."
    )]
    RecordShape,
    /// The materialized simulation needs a valid general model.
    #[error("Select a valid general model before materializing a simulated plant.")]
    SelectValidGeneralModel,
    /// The actuators do not match the model's inputs.
    #[error("Actuator count must match model inputs.")]
    ActuatorCountMismatch,
    /// The discrete sample time and the simulation step disagree.
    #[error("Discrete sample time and simulation step disagree.")]
    DiscreteStepDisagrees,
    /// The discrete plant's update period differs from its declared sample time.
    #[error("The discrete plant update period must equal its declared sample time.")]
    DiscreteUpdatePeriod,
    /// A continuous state-space model with a delay has no runtime adapter.
    #[error("A continuous state-space delay simulation adapter is not implemented.")]
    ContinuousDelayAdapter,
    /// The exact delayed adapter needs a scalar first-order model with declared input history.
    #[error("An exact delayed simulation adapter is available only for a scalar first-order model with declared input history.")]
    ExactDelayAdapterDomain,
    /// The delayed first-order parameters are outside the runtime adapter's domain.
    #[error("The delayed first-order model is outside the runtime adapter domain.")]
    DelayedFirstOrderDomain,
    /// A static transfer function has no initial state.
    #[error("A static transfer function requires no initial state coordinates.")]
    StaticNoStates,
    /// The actuator feedback addresses must be ordered input addresses.
    #[error("Actuator feedback must map ordered input addresses.")]
    ActuatorFeedbackAddresses,
    /// An actuator's capability or limits are not valid.
    #[error("Invalid actuator capability or limits.")]
    InvalidActuator,
    /// The plant identifier does not match the runtime contract.
    #[error("Plant id must match the runtime plant identifier contract.")]
    PlantId,
    /// The simulation step must be a whole number of milliseconds from 1 to 1000.
    #[error("Simulation step must be a whole millisecond from 1 to 1000.")]
    SimulationStep,
    /// The command bindings must be output or memory addresses, or program variables.
    #[error(
        "Ordered command bindings must use %Q output or %M memory signals or program variables."
    )]
    OrderedCommandBindings,
    /// The feedback bindings must be input addresses, or program variables.
    #[error("Ordered feedback bindings must use %I input signals or program variables.")]
    OrderedFeedbackBindings,
    /// The initial state and input must be finite.
    #[error("Initial state and input values must be finite.")]
    InitialValuesFinite,
    /// The bindings and the initial values must match the model's dimensions.
    #[error("Ordered bindings, initial state, and initial input must match the selected model dimensions.")]
    PlantShape,
    /// A simulation value is not finite.
    #[error("Simulation values must be finite.")]
    SimulationValuesFinite,
    /// The state-space order is zero or the transfer function is not proper.
    #[error("A proper transfer function with a nonzero leading denominator is required.")]
    CompanionProper,
    /// The transfer function's coefficients are not finite after normalisation.
    #[error("Transfer-function coefficients must remain finite after normalization.")]
    CompanionFinite,
}

/// The sentence for a failed simulation calculation. Each library variant has one sentence.
pub(crate) fn simulation_refusal(error: SimulationError) -> ModelRefusal {
    match error {
        SimulationError::InitialCoordinates => ModelRefusal::DelayedInitialCoordinates,
        SimulationError::InvalidDelayOrStep => ModelRefusal::InvalidDelayOrStep,
        SimulationError::NonIntegerDiscreteDelay => ModelRefusal::DiscreteDelayNotWhole,
        SimulationError::StateBudget => ModelRefusal::DelayBudget,
        SimulationError::Conversion(reason) => ModelRefusal::ConversionFailed {
            reason: reason_word(reason),
        },
        SimulationError::NonfiniteRealization => ModelRefusal::NonfiniteRealization,
        SimulationError::SamplePeriodMismatch => ModelRefusal::SamplePeriodsDiffer,
        SimulationError::RecordingPeriodMismatch => ModelRefusal::RecordingPeriodsDiffer,
        SimulationError::NonfiniteResponse => ModelRefusal::ResponseNotFinite,
        SimulationError::EmptyRecording => ModelRefusal::EmptyRecording,
        SimulationError::RecordShape => ModelRefusal::RecordShape,
        SimulationError::RecordingTooShort => ModelRefusal::RecordingTooShort,
        SimulationError::UnobservableInitialState => ModelRefusal::NotObservable,
        SimulationError::NonfinitePrediction => ModelRefusal::FrozenNotFinite,
        SimulationError::NotFirstOrderScalar => ModelRefusal::ExactDelayAdapterDomain,
        SimulationError::OutsideAdapterDomain => ModelRefusal::DelayedFirstOrderDomain,
        // Settings are checked by `model_time_settings` before a response is computed, so this
        // refusal carries the same sentence as that check.
        SimulationError::InvalidSettings => ModelRefusal::ChooseResponseSettings,
    }
}

/// The sentence for a failed model conversion. Each library variant has one sentence.
pub(crate) fn conversion_refusal(error: ConversionError) -> ModelRefusal {
    match error {
        ConversionError::NotSiso => ModelRefusal::TransferAnalysisRequiresSiso,
        ConversionError::OrderLimit => ModelRefusal::StateSpaceOrderLimit,
        ConversionError::NumericalUncertainty => ModelRefusal::StateSpaceUncertainty,
        ConversionError::UnsupportedSampledModel => ModelRefusal::ClassicalTuningSampled,
        ConversionError::NoFiniteEquivalent => ModelRefusal::NoFiniteEquivalent,
    }
}

/// The sentence for a structure that is not available.
pub(crate) fn structure_refusal(error: StructureError) -> ModelRefusal {
    match error {
        StructureError::NotQualified => ModelRefusal::StructureNotQualified,
        StructureError::Inconsistent => ModelRefusal::StructureInconsistent,
    }
}

/// The sentence for a transfer function that cannot be realized. The `TypeScript` reports two
/// messages: a non-finite normalised coefficient, and every other shape or leading-coefficient fault.
pub(crate) fn companion_refusal(error: ConstructionError) -> ModelRefusal {
    match error {
        ConstructionError::Nonfinite => ModelRefusal::CompanionFinite,
        _ => ModelRefusal::CompanionProper,
    }
}

/// The reason word the `TypeScript` prints for a rejected conversion.
pub(crate) fn reason_word(reason: NumericalRejectionReason) -> &'static str {
    match reason {
        NumericalRejectionReason::NonfiniteInput => "nonfinite-input",
        NumericalRejectionReason::InconsistentDimensions => "inconsistent-dimensions",
        NumericalRejectionReason::MatrixNotSquare => "matrix-not-square",
        NumericalRejectionReason::Singular => "singular",
        NumericalRejectionReason::IllConditioned => "ill-conditioned",
        NumericalRejectionReason::SolverFailed => "solver-failed",
    }
}
